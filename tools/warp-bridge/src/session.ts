import {
  BRIDGE_VERSION,
  CORE_PROTOCOL_VERSION,
  CORE_SCHEMA_HASH,
  PROMPT_VERSION,
} from "./protocol-identity.js";
import { parseProtocolLine } from "./protocol.js";
import type { BridgeHello, ProtocolMessage } from "./protocol.js";

const MAX_HANDSHAKE_FRAME_BYTES = 64 * 1024;

type SessionState = "awaiting_handshake" | "ready" | "rejected";

interface TranscriptCandidate {
  syncId: string;
  expectedItems: number;
  nextIndex: number;
  receivedBytes: number;
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

  receiveInboundLine(line: string): ProtocolMessage {
    if (this.state === "rejected") {
      throw new BridgeSessionError();
    }

    const message = parseProtocolLine(line, this.maxFrameBytes);
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
    this.validateTranscriptSync(message, line);
    return message;
  }

  private validateTranscriptSync(message: ProtocolMessage, line: string): void {
    if (message.type === "transcript_sync_begin") {
      if (this.transcriptCandidate !== undefined || message.total_bytes > this.maxTranscriptBytes) {
        throw new BridgeSessionError();
      }
      this.transcriptCandidate = {
        syncId: message.sync_id,
        expectedItems: message.item_count,
        nextIndex: 0,
        receivedBytes: 0,
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
      if (candidate.receivedBytes > this.maxTranscriptBytes) {
        throw new BridgeSessionError();
      }
      return;
    }

    if (message.type === "transcript_sync_commit") {
      if (
        candidate === undefined ||
        message.sync_id !== candidate.syncId ||
        candidate.nextIndex !== candidate.expectedItems
      ) {
        throw new BridgeSessionError();
      }
      this.transcriptCandidate = undefined;
      return;
    }

    if (candidate !== undefined) {
      throw new BridgeSessionError();
    }
  }
}
