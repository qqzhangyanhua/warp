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
    return message;
  }
}
