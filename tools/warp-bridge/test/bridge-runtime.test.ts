import { describe, expect, test } from "vitest";

import { BridgeTextRuntimeSession } from "../src/bridge-runtime.js";
import type {
  AssistantMessageCommit,
  CommittedResult,
  ProtocolMessage,
  RunStart,
  TranscriptItem,
} from "../src/protocol.js";
import type { TextRuntime, TextRunCallbacks } from "../src/text-runtime.js";

describe("Bridge text runtime session", () => {
  test("acknowledges a complete transcript before running and forwards commit results", async () => {
    const runtime = new FakeTextRuntime();
    const outbound: ProtocolMessage[] = [];
    const bridge = new BridgeTextRuntimeSession((message) => {
      outbound.push(message);
    }, runtime);

    bridge.receive(JSON.stringify(acceptedHandshake()));
    for (const frame of transcriptSync()) {
      bridge.receive(JSON.stringify(frame));
    }
    bridge.receive(JSON.stringify(runStart()));

    await runtime.started.promise;
    expect(outbound[0]).toEqual({
      type: "transcript_sync_result",
      sync_id: "sync-1",
      status: "accepted",
      revision: 7,
    });
    expect(runtime.transcript).toEqual([userMessage()]);

    const commit = await runtime.commitRequested.promise;
    expect(outbound[1]).toEqual(commit);
    bridge.receive(
      JSON.stringify({
        type: "commit_result",
        conversation_id: commit.conversation_id,
        run_id: commit.run_id,
        commit_id: commit.commit_id,
        status: "committed",
        revision: 8,
      } satisfies CommittedResult),
    );
    await bridge.waitForIdle();

    expect(runtime.committedRevision).toBe(8);
    expect(outbound.at(-1)).toEqual({
      type: "run_finished",
      conversation_id: "conversation-1",
      run_id: "run-1",
      outcome: "completed",
    });
  });

  test("does not replace an accepted transcript with an invalid candidate", async () => {
    const runtime = new FakeTextRuntime();
    const bridge = new BridgeTextRuntimeSession(() => {}, runtime);
    bridge.receive(JSON.stringify(acceptedHandshake()));
    for (const frame of transcriptSync()) {
      bridge.receive(JSON.stringify(frame));
    }

    const invalid = transcriptSync("sync-2", 8);
    bridge.receive(JSON.stringify(invalid[0]));
    expect(() => bridge.receive(JSON.stringify(invalid[2]))).toThrow();
    bridge.receive(JSON.stringify(runStart()));

    await runtime.started.promise;
    expect(runtime.transcript).toEqual([userMessage()]);
    expect(runtime.start?.transcript_revision).toBe(7);
  });

  test("rejects a pending assistant commit when the run is cancelled", async () => {
    const runtime = new FakeTextRuntime();
    const bridge = new BridgeTextRuntimeSession(() => {}, runtime);
    bridge.receive(JSON.stringify(acceptedHandshake()));
    for (const frame of transcriptSync()) {
      bridge.receive(JSON.stringify(frame));
    }
    bridge.receive(JSON.stringify(runStart()));
    await runtime.commitRequested.promise;

    bridge.receive(
      JSON.stringify({
        type: "run_cancel",
        conversation_id: "conversation-1",
        run_id: "run-1",
      } satisfies ProtocolMessage),
    );

    await runtime.commitRejected.promise;
    await bridge.waitForIdle();
    expect(runtime.cancelled).toBe(true);
  });

  test("rejects Bridge-originated messages received from Warp", () => {
    const bridge = new BridgeTextRuntimeSession(() => {}, new FakeTextRuntime());
    bridge.receive(JSON.stringify(acceptedHandshake()));

    expect(() =>
      bridge.receive(
        JSON.stringify({
          type: "text_delta",
          conversation_id: "conversation-1",
          run_id: "run-1",
          event_id: "event-1",
          delta: "invalid direction",
        } satisfies ProtocolMessage),
      ),
    ).toThrow();
  });
});

class FakeTextRuntime implements TextRuntime {
  readonly started = deferred<void>();
  readonly commitRequested = deferred<AssistantMessageCommit>();
  readonly commitRejected = deferred<void>();
  transcript: TranscriptItem[] = [];
  start: RunStart | undefined;
  committedRevision: number | undefined;
  cancelled = false;

  cancel(): void {
    this.cancelled = true;
  }

  async run(
    transcript: TranscriptItem[],
    start: RunStart,
    callbacks: TextRunCallbacks,
  ): Promise<void> {
    this.transcript = transcript;
    this.start = start;
    this.started.resolve();
    const commit: AssistantMessageCommit = {
      type: "assistant_message_commit",
      conversation_id: start.conversation_id,
      run_id: start.run_id,
      event_id: "event-1",
      commit_id: "commit-1",
      message_id: "assistant-1",
      expected_revision: start.transcript_revision,
      content: [{ type: "text", text: "Done" }],
    };
    this.commitRequested.resolve(commit);
    let result: CommittedResult;
    try {
      result = await callbacks.commit(commit);
    } catch {
      this.commitRejected.resolve();
      return;
    }
    this.committedRevision = result.revision;
    await callbacks.emit({
      type: "run_finished",
      conversation_id: start.conversation_id,
      run_id: start.run_id,
      outcome: "completed",
    });
  }
}

function acceptedHandshake(): ProtocolMessage {
  return {
    type: "handshake_result",
    status: "accepted",
    max_frame_bytes: 1_048_576,
    max_transcript_bytes: 16_777_216,
  };
}

function transcriptSync(syncId = "sync-1", revision = 7): ProtocolMessage[] {
  const item = {
    type: "transcript_sync_item",
    sync_id: syncId,
    index: 0,
    item: userMessage(),
  } satisfies ProtocolMessage;
  const itemLine = JSON.stringify(item);
  return [
    {
      type: "transcript_sync_begin",
      sync_id: syncId,
      conversation_id: "conversation-1",
      revision,
      item_count: 1,
      total_bytes: Buffer.byteLength(itemLine, "utf8"),
    },
    item,
    { type: "transcript_sync_commit", sync_id: syncId },
  ];
}

function userMessage(): TranscriptItem {
  return {
    kind: "message",
    message_id: "user-1",
    role: "user",
    content: [{ type: "text", text: "Inspect the workspace" }],
  };
}

function runStart(): RunStart {
  return {
    type: "run_start",
    conversation_id: "conversation-1",
    run_id: "run-1",
    transcript_revision: 7,
    configuration: {
      provider: {
        protocol: "chat_completions",
        base_url: "https://provider.example/v1/chat/completions",
        provider_origin: "https://provider.example",
        model: "local-model",
        api_key: "secret-key",
        max_provider_attempts: 2,
        max_redirects: 3,
      },
      working_directory: "/workspace",
      context_limit: 32_768,
      reasoning_effort: "medium",
      tool_request_limit: 0,
      tools: [],
      resources: [],
    },
  };
}

function deferred<T>(): {
  promise: Promise<T>;
  resolve: (value: T) => void;
} {
  let resolve = (_value: T): void => {
    throw new Error("deferred promise was not initialized");
  };
  const promise = new Promise<T>((resolver) => {
    resolve = resolver;
  });
  return { promise, resolve };
}
