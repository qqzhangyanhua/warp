import {
  BRIDGE_VERSION,
  CORE_PROTOCOL_VERSION,
  CORE_SCHEMA_HASH,
  PROMPT_VERSION,
} from "./protocol-identity.js";
import { parseProtocolLine } from "./protocol.js";
import type { BridgeHello, ProtocolMessage, RunStart, TranscriptItem } from "./protocol.js";

const MAX_HANDSHAKE_FRAME_BYTES = 64 * 1024;

type SessionState = "awaiting_handshake" | "ready" | "rejected";

interface TranscriptCandidate {
  syncId: string;
  conversationId: string;
  revision: number;
  expectedItems: number;
  expectedBytes: number;
  nextIndex: number;
  receivedBytes: number;
  items: TranscriptItem[];
}

export interface AcceptedTranscript {
  syncId: string;
  conversationId: string;
  revision: number;
  items: TranscriptItem[];
}

export class BridgeSessionError extends Error {
  constructor() {
    super("Invalid Bridge Protocol session state");
    this.name = "BridgeSessionError";
  }
}

export class BridgeProtocolSession {
  private state: SessionState = "awaiting_handshake";
  private maxFrameBytes = MAX_HANDSHAKE_FRAME_BYTES;
  private maxTranscriptBytes = 0;
  private transcriptCandidate: TranscriptCandidate | undefined;
  private acceptedTranscript: AcceptedTranscript | undefined;

  hello(): BridgeHello {
    return {
      type: "bridge_hello",
      protocol_version: CORE_PROTOCOL_VERSION,
      core_schema_hash: CORE_SCHEMA_HASH,
      bridge_version: BRIDGE_VERSION,
      capabilities: [],
      prompt_version: PROMPT_VERSION,
    };
  }

  isReady(): boolean {
    return this.state === "ready";
  }

  transcriptForRun(start: RunStart): TranscriptItem[] {
    const transcript = this.acceptedTranscript;
    if (
      transcript === undefined ||
      transcript.conversationId !== start.conversation_id ||
      transcript.revision !== start.transcript_revision
    ) {
      throw new BridgeSessionError();
    }
    return [...transcript.items];
  }

  acceptedTranscriptIdentity(): AcceptedTranscript | undefined {
    const transcript = this.acceptedTranscript;
    return transcript === undefined
      ? undefined
      : { ...transcript, items: [...transcript.items] };
  }

  receiveInboundLine(line: string): ProtocolMessage {
    if (this.state === "rejected") {
      throw new BridgeSessionError();
    }

    let message: ProtocolMessage;
    try {
      message = parseProtocolLine(line, this.maxFrameBytes);
    } catch (error) {
      if (this.state === "ready") {
        this.transcriptCandidate = undefined;
      }
      throw error;
    }
    if (this.state === "awaiting_handshake") {
      if (message.type !== "handshake_result") {
        throw new BridgeSessionError();
      }
      if (message.status === "rejected") {
        this.state = "rejected";
        throw new BridgeSessionError();
      }
      this.maxFrameBytes = message.max_frame_bytes;
      this.maxTranscriptBytes = message.max_transcript_bytes;
      this.state = "ready";
      return message;
    }

    if (message.type === "bridge_hello" || message.type === "handshake_result") {
      throw new BridgeSessionError();
    }
    try {
      this.validateTranscriptSync(message, line);
    } catch (error) {
      this.transcriptCandidate = undefined;
      throw error;
    }
    return message;
  }

  private validateTranscriptSync(message: ProtocolMessage, line: string): void {
    if (message.type === "transcript_sync_begin") {
      if (this.transcriptCandidate !== undefined || message.total_bytes > this.maxTranscriptBytes) {
        throw new BridgeSessionError();
      }
      this.transcriptCandidate = {
        syncId: message.sync_id,
        conversationId: message.conversation_id,
        revision: message.revision,
        expectedItems: message.item_count,
        expectedBytes: message.total_bytes,
        nextIndex: 0,
        receivedBytes: 0,
        items: [],
      };
      return;
    }

    const candidate = this.transcriptCandidate;
    if (message.type === "transcript_sync_item") {
      if (
        candidate === undefined ||
        message.sync_id !== candidate.syncId ||
        message.index !== candidate.nextIndex ||
        message.index >= candidate.expectedItems
      ) {
        throw new BridgeSessionError();
      }
      candidate.nextIndex += 1;
      candidate.receivedBytes += Buffer.byteLength(line, "utf8");
      candidate.items.push(message.item);
      if (
        candidate.receivedBytes > candidate.expectedBytes ||
        candidate.receivedBytes > this.maxTranscriptBytes
      ) {
        throw new BridgeSessionError();
      }
      return;
    }

    if (message.type === "transcript_sync_commit") {
      if (
        candidate === undefined ||
        message.sync_id !== candidate.syncId ||
        candidate.nextIndex !== candidate.expectedItems ||
        candidate.receivedBytes !== candidate.expectedBytes
      ) {
        throw new BridgeSessionError();
      }
      this.acceptedTranscript = {
        syncId: candidate.syncId,
        conversationId: candidate.conversationId,
        revision: candidate.revision,
        items: candidate.items,
      };
      this.transcriptCandidate = undefined;
      return;
    }

    if (candidate !== undefined) {
      throw new BridgeSessionError();
    }
  }
}
