import type {
  AssistantMessageCommit,
  CommittedResult,
  ProtocolMessage,
} from "./protocol.js";
import { BridgeProtocolSession, BridgeSessionError } from "./session.js";
import { PiTextRuntime, type TextRuntime } from "./text-runtime.js";

type EmitProtocolMessage = (message: ProtocolMessage) => void | Promise<void>;

interface PendingCommit {
  request: AssistantMessageCommit;
  resolve(result: CommittedResult): void;
  reject(error: Error): void;
}

export class BridgeTextRuntimeSession {
  private readonly protocol = new BridgeProtocolSession();
  private readonly runtime: TextRuntime;
  private readonly emit: EmitProtocolMessage;
  private activeRun: Promise<void> | undefined;
  private pendingCommit: PendingCommit | undefined;

  constructor(emit: EmitProtocolMessage, runtime: TextRuntime = new PiTextRuntime()) {
    this.emit = emit;
    this.runtime = runtime;
  }

  hello(): ProtocolMessage {
    return this.protocol.hello();
  }

  receive(line: string): void {
    const message = this.protocol.receiveInboundLine(line);
    switch (message.type) {
      case "transcript_sync_commit": {
        const transcript = this.protocol.acceptedTranscriptIdentity();
        if (transcript === undefined || transcript.syncId !== message.sync_id) {
          throw new BridgeSessionError();
        }
        void this.emit({
          type: "transcript_sync_result",
          sync_id: transcript.syncId,
          status: "accepted",
          revision: transcript.revision,
        });
        return;
      }
      case "run_start": {
        if (this.activeRun !== undefined) {
          throw new BridgeSessionError();
        }
        const transcript = this.protocol.transcriptForRun(message);
        const running = this.runtime.run(transcript, message, {
          emit: (event) => this.emit(event),
          commit: (request) => this.requestCommit(request),
        });
        this.activeRun = running.finally(() => {
          this.activeRun = undefined;
          this.rejectPendingCommit();
        });
        return;
      }
      case "commit_result":
        this.acceptCommit(message);
        return;
      case "run_cancel":
        this.runtime.cancel();
        this.rejectPendingCommit();
        return;
      case "handshake_result":
      case "transcript_sync_begin":
      case "transcript_sync_item":
        return;
      case "bridge_hello":
      case "transcript_sync_result":
      case "run_status":
      case "text_delta":
      case "assistant_message_commit":
      case "run_finished":
      case "tool_request":
      case "tool_result":
        throw new BridgeSessionError();
    }
  }

  async waitForIdle(): Promise<void> {
    await this.activeRun;
  }

  close(): void {
    this.runtime.cancel();
    this.rejectPendingCommit();
  }

  private async requestCommit(request: AssistantMessageCommit): Promise<CommittedResult> {
    if (this.pendingCommit !== undefined) {
      throw new BridgeSessionError();
    }
    const result = new Promise<CommittedResult>((resolve, reject) => {
      this.pendingCommit = { request, resolve, reject };
    });
    await this.emit(request);
    return result;
  }

  private acceptCommit(result: CommittedResult): void {
    const pending = this.pendingCommit;
    if (
      pending === undefined ||
      pending.request.conversation_id !== result.conversation_id ||
      pending.request.run_id !== result.run_id ||
      pending.request.commit_id !== result.commit_id
    ) {
      throw new BridgeSessionError();
    }
    this.pendingCommit = undefined;
    pending.resolve(result);
  }

  private rejectPendingCommit(): void {
    const pending = this.pendingCommit;
    this.pendingCommit = undefined;
    pending?.reject(new BridgeSessionError());
  }
}
