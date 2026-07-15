import { type Context, type SimpleStreamOptions } from "@earendil-works/pi-ai";
import { describe, expect, test } from "vitest";

import type {
  AssistantMessageCommit,
  CommittedResult,
  RunStart,
  TranscriptItem,
} from "../src/protocol.js";
import { PiTextRuntime, type TextRunEvent } from "../src/text-runtime.js";
import {
  deferred,
  idSequence,
  runStart,
  scriptedStream,
  successfulCallbacks,
  transcript,
} from "./text-runtime-test-helpers.js";

describe("Pi text runtime", () => {
  test("streams text and waits for the durable assistant commit", async () => {
    const streamOptions: SimpleStreamOptions[] = [];
    const contexts: Context[] = [];
    const runtime = new PiTextRuntime({
      stream: (_model, context, options) => {
        contexts.push(context);
        streamOptions.push(options ?? {});
        return scriptedStream([{ type: "text", text: "Workspace inspected." }]);
      },
      createId: idSequence(),
    });
    const events: TextRunEvent[] = [];
    const commitAcknowledgement = deferred<CommittedResult>();
    const commitStarted = deferred<AssistantMessageCommit>();

    const running = runtime.run(transcript(), runStart(), {
      emit: (event) => {
        events.push(event);
      },
      commit: (request) => {
        commitStarted.resolve(request);
        return commitAcknowledgement.promise;
      },
    });

    const commit = await commitStarted.promise;
    expect(commit.content).toEqual([{ type: "text", text: "Workspace inspected." }]);
    expect(events.some((event) => event.type === "run_finished")).toBe(false);
    commitAcknowledgement.resolve({
      type: "commit_result",
      conversation_id: commit.conversation_id,
      run_id: commit.run_id,
      commit_id: commit.commit_id,
      status: "committed",
      revision: 8,
    });
    await running;

    expect(streamOptions).toHaveLength(1);
    expect(streamOptions[0]?.maxRetries).toBe(0);
    expect(contexts[0]?.messages).toHaveLength(1);
    expect(events.map((event) => event.type)).toEqual([
      "run_status",
      "text_delta",
      "run_status",
      "run_finished",
    ]);
    expect(events.at(-1)).toMatchObject({ type: "run_finished", outcome: "completed" });
  });

  test("retries one typed HTTP failure before model output", async () => {
    let attempts = 0;
    const runtime = new PiTextRuntime({
      stream: () => {
        attempts += 1;
        return attempts === 1
          ? scriptedStream([
              {
                type: "error",
                errorMessage: "provider failed",
                requestProvider: true,
              },
            ])
          : scriptedStream([{ type: "text", text: "Recovered." }]);
      },
      createId: idSequence(),
    });
    const events: TextRunEvent[] = [];
    const previousFetch = globalThis.fetch;
    globalThis.fetch = async () => new Response(null, { status: 503 });

    try {
      await runtime.run(transcript(), runStart(), {
        emit: (event) => {
          events.push(event);
        },
        commit: async (request) => ({
          type: "commit_result",
          conversation_id: request.conversation_id,
          run_id: request.run_id,
          commit_id: request.commit_id,
          status: "committed",
          revision: 8,
        }),
      });
    } finally {
      globalThis.fetch = previousFetch;
    }

    expect(attempts).toBe(2);
    expect(events.at(-1)).toMatchObject({ type: "run_finished", outcome: "completed" });
  });

  test("does not retry an unapproved provider failure before output", async () => {
    let attempts = 0;
    const runtime = new PiTextRuntime({
      stream: () => {
        attempts += 1;
        return scriptedStream([
          { type: "error", errorMessage: "400 invalid request" },
        ]);
      },
      createId: idSequence(),
    });
    const events: TextRunEvent[] = [];

    await runtime.run(transcript(), runStart(), {
      emit: (event) => {
        events.push(event);
      },
      commit: async () => {
        throw new Error("failed output must not cross the commit barrier");
      },
    });

    expect(attempts).toBe(1);
    expect(events.at(-1)).toMatchObject({
      type: "run_finished",
      outcome: "failed",
    });
  });

  test("reports a forbidden Provider redirect with its stable protocol code", async () => {
    const runtime = new PiTextRuntime({
      stream: () =>
        scriptedStream([
          { type: "error", errorMessage: "provider failed", requestProvider: true },
        ]),
      createId: idSequence(),
    });
    const events: TextRunEvent[] = [];
    const previousFetch = globalThis.fetch;
    globalThis.fetch = async () =>
      new Response(null, {
        status: 307,
        headers: { location: "https://attacker.example/collect" },
      });

    try {
      await runtime.run(transcript(), runStart(), {
        emit: (event) => {
          events.push(event);
        },
        commit: async () => {
          throw new Error("failed output must not cross the commit barrier");
        },
      });
    } finally {
      globalThis.fetch = previousFetch;
    }

    expect(events.at(-1)).toMatchObject({
      type: "run_finished",
      outcome: "failed",
      error_code: "provider_redirect_not_allowed",
    });
  });

  test("does not retry after any model output", async () => {
    let attempts = 0;
    const runtime = new PiTextRuntime({
      stream: () => {
        attempts += 1;
        return scriptedStream([
          { type: "text", text: "Partial" },
          {
            type: "error",
            errorMessage: "Stream ended without finish_reason",
          },
        ]);
      },
      createId: idSequence(),
    });
    const events: TextRunEvent[] = [];

    await runtime.run(transcript(), runStart(), {
      emit: (event) => {
        events.push(event);
      },
      commit: async () => {
        throw new Error("partial output must not cross the commit barrier");
      },
    });

    expect(attempts).toBe(1);
    expect(events.at(-1)).toMatchObject({
      type: "run_finished",
      outcome: "failed",
      error_code: "provider_transport_error",
    });
  });

  test("does not retry after reasoning output", async () => {
    let attempts = 0;
    const runtime = new PiTextRuntime({
      stream: () => {
        attempts += 1;
        return scriptedStream([
          { type: "thinking", text: "Inspecting" },
          {
            type: "error",
            errorMessage: "Stream ended without finish_reason",
          },
        ]);
      },
      createId: idSequence(),
    });
    const events: TextRunEvent[] = [];

    await runtime.run(transcript(), runStart(), {
      emit: (event) => {
        events.push(event);
      },
      commit: async () => {
        throw new Error("reasoning-only output must not cross the commit barrier");
      },
    });

    expect(attempts).toBe(1);
    expect(events.at(-1)).toMatchObject({
      type: "run_finished",
      outcome: "failed",
      error_code: "provider_transport_error",
    });
  });

  test("fails the run when the assistant commit is not acknowledged in time", async () => {
    const runtime = new PiTextRuntime({
      stream: () => scriptedStream([{ type: "text", text: "Done." }]),
      createId: idSequence(),
      commitTimeoutMs: 10,
    });
    const events: TextRunEvent[] = [];

    await runtime.run(transcript(), runStart(), {
      emit: (event) => {
        events.push(event);
      },
      commit: () => new Promise<never>(() => {}),
    });

    expect(events.at(-1)).toMatchObject({
      type: "run_finished",
      outcome: "failed",
      error_code: "commit_timeout",
    });
  });

  test("cancels while waiting for an assistant commit acknowledgement", async () => {
    const runtime = new PiTextRuntime({
      stream: () => scriptedStream([{ type: "text", text: "Done." }]),
      createId: idSequence(),
    });
    const events: TextRunEvent[] = [];
    const commitStarted = deferred<void>();
    const acknowledgement = deferred<CommittedResult>();

    const running = runtime.run(transcript(), runStart(), {
      emit: (event) => {
        events.push(event);
      },
      commit: () => {
        commitStarted.resolve();
        return acknowledgement.promise;
      },
    });
    await commitStarted.promise;

    runtime.cancel();
    acknowledgement.reject(new Error("cancelled"));
    await running;

    expect(events.at(-1)).toMatchObject({
      type: "run_finished",
      outcome: "cancelled",
    });
  });

  test("renders current resources once while replaying historical snapshots", async () => {
    const contexts: Context[] = [];
    const runtime = new PiTextRuntime({
      stream: (_model, context) => {
        contexts.push(context);
        return scriptedStream([{ type: "text", text: "Done." }]);
      },
      createId: idSequence(),
    });
    const start = runStart();
    start.configuration.resources = [
      {
        id: "current-rule",
        name: "AGENTS.md",
        content: [{ type: "text", text: "Current instructions" }],
      },
    ];
    const items: TranscriptItem[] = [
      ...transcript(),
      {
        kind: "resource_snapshot",
        resource_id: "historical-rule",
        name: "OLD_AGENTS.md",
        content: [{ type: "text", text: "Historical instructions" }],
      },
      {
        kind: "resource_snapshot",
        resource_id: "current-rule",
        name: "AGENTS.md",
        content: [{ type: "text", text: "Current instructions" }],
      },
    ];

    await runtime.run(items, start, {
      emit: () => {},
      commit: async (request) => ({
        type: "commit_result",
        conversation_id: request.conversation_id,
        run_id: request.run_id,
        commit_id: request.commit_id,
        status: "committed",
        revision: 8,
      }),
    });

    expect(contexts[0]?.systemPrompt).toContain("Current instructions");
    const replayed = JSON.stringify(contexts[0]?.messages);
    expect(replayed).toContain("Historical instructions");
    expect(replayed).not.toContain("Current instructions");
  });

  test("replays current resource images without duplicating their text", async () => {
    const contexts: Context[] = [];
    const runtime = new PiTextRuntime({
      stream: (_model, context) => {
        contexts.push(context);
        return scriptedStream([{ type: "text", text: "Done." }]);
      },
      createId: idSequence(),
    });
    const start = runStart();
    start.configuration.resources = [
      {
        id: "current-image",
        name: "diagram.png",
        content: [
          { type: "text", text: "Current image description" },
          { type: "image", mime_type: "image/png", data_base64: "aW1hZ2U=" },
        ],
      },
    ];
    const items: TranscriptItem[] = [
      ...transcript(),
      {
        kind: "resource_snapshot",
        resource_id: "current-image",
        name: "diagram.png",
        content: start.configuration.resources[0]!.content,
      },
    ];

    await runtime.run(items, start, successfulCallbacks());

    expect(JSON.stringify(contexts[0]?.messages)).toContain("aW1hZ2U=");
    expect(JSON.stringify(contexts[0]?.messages)).not.toContain(
      "Current image description",
    );
  });

  test("replays paired historical tool activity into Pi messages", async () => {
    const contexts: Context[] = [];
    const runtime = new PiTextRuntime({
      stream: (_model, context) => {
        contexts.push(context);
        return scriptedStream([{ type: "text", text: "Done." }]);
      },
      createId: idSequence(),
    });
    const items: TranscriptItem[] = [
      ...transcript(),
      {
        kind: "tool_request",
        tool_call_id: "call-1",
        tool_id: "builtin.run_shell_command",
        tool_name: "run_shell_command",
        arguments: { command: "pwd" },
      },
      {
        kind: "tool_result",
        tool_call_id: "call-1",
        result: {
          status: "success",
          content: [{ type: "text", text: "/workspace" }],
          truncated: false,
        },
      },
    ];

    await runtime.run(items, runStart(), successfulCallbacks());

    expect(contexts[0]?.messages.slice(-2)).toMatchObject([
      {
        role: "assistant",
        content: [
          {
            type: "toolCall",
            id: "call-1",
            name: "run_shell_command",
            arguments: { command: "pwd" },
          },
        ],
      },
      {
        role: "toolResult",
        toolCallId: "call-1",
        toolName: "run_shell_command",
        content: [{ type: "text", text: "/workspace" }],
        isError: false,
      },
    ]);
  });

  test("pairs reused tool call IDs in transcript order", async () => {
    const contexts: Context[] = [];
    const runtime = new PiTextRuntime({
      stream: (_model, context) => {
        contexts.push(context);
        return scriptedStream([{ type: "text", text: "Done." }]);
      },
      createId: idSequence(),
    });
    const items: TranscriptItem[] = [
      ...transcript(),
      {
        kind: "tool_request",
        tool_call_id: "call-1",
        tool_id: "builtin.run_shell_command",
        tool_name: "run_shell_command",
        arguments: { command: "pwd" },
      },
      {
        kind: "tool_result",
        tool_call_id: "call-1",
        result: {
          status: "success",
          content: [{ type: "text", text: "/first" }],
          truncated: false,
        },
      },
      {
        kind: "tool_request",
        tool_call_id: "call-1",
        tool_id: "builtin.read_files",
        tool_name: "read_files",
        arguments: { files: [{ name: "README.md", line_ranges: [] }] },
      },
      {
        kind: "tool_result",
        tool_call_id: "call-1",
        result: {
          status: "success",
          content: [{ type: "text", text: "/second" }],
          truncated: false,
        },
      },
    ];

    await runtime.run(items, runStart(), successfulCallbacks());

    const results = contexts[0]?.messages.filter((message) => message.role === "toolResult");
    expect(results).toMatchObject([
      { toolCallId: "call-1", toolName: "run_shell_command" },
      { toolCallId: "call-1", toolName: "read_files" },
    ]);
  });
});
