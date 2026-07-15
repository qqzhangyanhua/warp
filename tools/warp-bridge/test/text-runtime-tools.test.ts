import {
  createAssistantMessageEventStream,
  type AssistantMessage,
} from "@earendil-works/pi-ai";
import { describe, expect, test } from "vitest";

import type {
  AssistantMessageCommit,
  CommittedResult,
  ToolRequest,
  ToolResult,
} from "../src/protocol.js";
import { PiTextRuntime, type TextRunEvent } from "../src/text-runtime.js";
import {
  deferred,
  idSequence,
  runStart,
  scriptedStream,
  transcript,
} from "./text-runtime-test-helpers.js";

describe("Pi text runtime proxy tools", () => {
  test("returns a Warp tool result to the same run before the next model turn", async () => {
    let providerTurns = 0;
    const runtime = new PiTextRuntime({
      stream: () => {
        providerTurns += 1;
        return providerTurns === 1
          ? toolCallStream("call-1", "read_files", {
              files: [{ name: "README.md" }],
            })
          : scriptedStream([{ type: "text", text: "Workspace inspected." }]);
      },
      createId: idSequence(),
    });
    const start = runStart();
    start.configuration.tool_request_limit = 32;
    start.configuration.tools = [
      {
        id: "builtin.read_files",
        name: "read_files",
        description: "Read selected files after Warp approval.",
        input_schema: {
          type: "object",
          properties: { files: { type: "array" } },
          required: ["files"],
          additionalProperties: false,
        },
      },
    ];
    const requested = deferred<ToolRequest>();
    const result = deferred<ToolResult>();
    const events: TextRunEvent[] = [];

    const running = runtime.run(transcript(), start, {
      emit: (event) => {
        events.push(event);
      },
      commit: async (request) => ({
        type: "commit_result",
        conversation_id: request.conversation_id,
        run_id: request.run_id,
        commit_id: request.commit_id,
        status: "committed",
        revision: request.expected_revision + 1,
      }),
      tool: (request) => {
        requested.resolve(request);
        return result.promise;
      },
    });

    const toolRequest = await requested.promise;
    expect(toolRequest).toEqual({
      type: "tool_request",
      conversation_id: "conversation-1",
      run_id: "run-1",
      tool_call_id: "call-1",
      tool_id: "builtin.read_files",
      tool_name: "read_files",
      arguments: { files: [{ name: "README.md" }] },
    });
    result.resolve({
      type: "tool_result",
      conversation_id: "conversation-1",
      run_id: "run-1",
      tool_call_id: "call-1",
      status: "success",
      content: [{ type: "text", text: "README contents" }],
      truncated: false,
    });
    await running;

    expect(providerTurns).toBe(2);
    expect(events.at(-1)).toMatchObject({
      type: "run_finished",
      outcome: "completed",
    });
  });

  test.each([
    {
      errorCode: "tool_request_limit_exceeded" as const,
      terminalOutcome: "limit_reached",
    },
    {
      errorCode: "tool_outcome_unknown" as const,
      terminalOutcome: "failed",
    },
  ])(
    "ends the run after a $errorCode tool result",
    async ({ errorCode, terminalOutcome }) => {
      let providerTurns = 0;
      const runtime = new PiTextRuntime({
        stream: () => {
          providerTurns += 1;
          return toolCallStream("call-1", "read_files", {
            files: [{ name: "README.md" }],
          });
        },
        createId: idSequence(),
      });
      const start = runStartWithReadFiles();
      const events: TextRunEvent[] = [];

      await runtime.run(transcript(), start, {
        emit: (event) => {
          events.push(event);
        },
        commit: async (request) => ({
          type: "commit_result",
          conversation_id: request.conversation_id,
          run_id: request.run_id,
          commit_id: request.commit_id,
          status: "committed",
          revision: request.expected_revision + 1,
        }),
        tool: async (request): Promise<ToolResult> =>
          errorToolResult(request, errorCode),
      });

      expect(providerTurns).toBe(1);
      expect(events.at(-1)).toMatchObject({
        type: "run_finished",
        outcome: terminalOutcome,
      });
    },
  );

  test("commits assistant text before requesting its tool effect", async () => {
    let providerTurns = 0;
    const runtime = new PiTextRuntime({
      stream: () => {
        providerTurns += 1;
        return providerTurns === 1
          ? textAndToolCallStream("Inspecting.", "call-1", "read_files", {
              files: [{ name: "README.md" }],
            })
          : scriptedStream([{ type: "text", text: "Done." }]);
      },
      createId: idSequence(),
    });
    const start = runStartWithReadFiles();
    const firstCommit = deferred<AssistantMessageCommit>();
    const firstCommitResult = deferred<CommittedResult>();
    const toolRequested = deferred<ToolRequest>();
    let commits = 0;

    const running = runtime.run(transcript(), start, {
      emit: () => {},
      commit: async (request) => {
        commits += 1;
        if (commits === 1) {
          firstCommit.resolve(request);
          return firstCommitResult.promise;
        }
        return {
          type: "commit_result",
          conversation_id: request.conversation_id,
          run_id: request.run_id,
          commit_id: request.commit_id,
          status: "committed",
          revision: request.expected_revision + 1,
        };
      },
      tool: async (request) => {
        toolRequested.resolve(request);
        return {
          type: "tool_result",
          conversation_id: request.conversation_id,
          run_id: request.run_id,
          tool_call_id: request.tool_call_id,
          status: "success",
          content: [{ type: "text", text: "README contents" }],
          truncated: false,
        };
      },
    });

    const commit = await firstCommit.promise;
    const toolStartedBeforeCommit = await Promise.race([
      toolRequested.promise.then(() => true),
      new Promise<false>((resolve) => setTimeout(() => resolve(false), 20)),
    ]);
    expect(toolStartedBeforeCommit).toBe(false);
    firstCommitResult.resolve({
      type: "commit_result",
      conversation_id: commit.conversation_id,
      run_id: commit.run_id,
      commit_id: commit.commit_id,
      status: "committed",
      revision: commit.expected_revision + 1,
    });
    await toolRequested.promise;
    await running;

    expect(providerTurns).toBe(2);
  });

  test("a mixed tool batch cannot start another provider turn after the limit", async () => {
    let providerTurns = 0;
    const runtime = new PiTextRuntime({
      stream: () => {
        providerTurns += 1;
        return multipleToolCallStream([
          { id: "call-32", name: "read_files", args: { files: [{ name: "a" }] } },
          { id: "call-33", name: "read_files", args: { files: [{ name: "b" }] } },
        ]);
      },
      createId: idSequence(),
    });
    const events: TextRunEvent[] = [];

    await runtime.run(transcript(), runStartWithReadFiles(), {
      emit: (event) => {
        events.push(event);
      },
      commit: async (request) => ({
        type: "commit_result",
        conversation_id: request.conversation_id,
        run_id: request.run_id,
        commit_id: request.commit_id,
        status: "committed",
        revision: request.expected_revision + 1,
      }),
      tool: async (request) =>
        request.tool_call_id === "call-32"
          ? {
              type: "tool_result",
              conversation_id: request.conversation_id,
              run_id: request.run_id,
              tool_call_id: request.tool_call_id,
              status: "success",
              content: [{ type: "text", text: "a" }],
              truncated: false,
            }
          : errorToolResult(request, "tool_request_limit_exceeded"),
    });

    expect(providerTurns).toBe(1);
    expect(events.at(-1)).toMatchObject({
      type: "run_finished",
      outcome: "limit_reached",
    });
  });
});

function runStartWithReadFiles() {
  const start = runStart();
  start.configuration.tool_request_limit = 32;
  start.configuration.tools = [
    {
      id: "builtin.read_files",
      name: "read_files",
      description: "Read selected files after Warp approval.",
      input_schema: {
        type: "object",
        properties: { files: { type: "array" } },
        required: ["files"],
        additionalProperties: false,
      },
    },
  ];
  return start;
}

function errorToolResult(
  request: ToolRequest,
  errorCode: "tool_request_limit_exceeded" | "tool_outcome_unknown",
): ToolResult {
  const common = {
    type: "tool_result" as const,
    conversation_id: request.conversation_id,
    run_id: request.run_id,
    tool_call_id: request.tool_call_id,
    status: "error" as const,
    content: [{ type: "text" as const, text: "Tool run stopped." }],
    truncated: false,
  };
  return errorCode === "tool_outcome_unknown"
    ? {
        ...common,
        error_code: errorCode,
        may_have_executed: true,
      }
    : {
        ...common,
        error_code: errorCode,
        may_have_executed: false,
      };
}

function toolCallStream(
  id: string,
  name: string,
  args: Record<string, unknown>,
) {
  const stream = createAssistantMessageEventStream();
  queueMicrotask(() => {
    const empty = assistantMessage([]);
    const toolCall = { type: "toolCall" as const, id, name, arguments: args };
    const complete = assistantMessage([toolCall], "toolUse");
    stream.push({ type: "start", partial: empty });
    stream.push({ type: "toolcall_start", contentIndex: 0, partial: empty });
    stream.push({
      type: "toolcall_end",
      contentIndex: 0,
      toolCall,
      partial: complete,
    });
    stream.push({ type: "done", reason: "toolUse", message: complete });
  });
  return stream;
}

function textAndToolCallStream(
  text: string,
  id: string,
  name: string,
  args: Record<string, unknown>,
) {
  const stream = createAssistantMessageEventStream();
  queueMicrotask(() => {
    const empty = assistantMessage([]);
    const textContent = { type: "text" as const, text };
    const partial = assistantMessage([textContent]);
    const toolCall = { type: "toolCall" as const, id, name, arguments: args };
    const complete = assistantMessage([textContent, toolCall], "toolUse");
    stream.push({ type: "start", partial: empty });
    stream.push({
      type: "text_delta",
      contentIndex: 0,
      delta: text,
      partial,
    });
    stream.push({ type: "toolcall_start", contentIndex: 1, partial });
    stream.push({
      type: "toolcall_end",
      contentIndex: 1,
      toolCall,
      partial: complete,
    });
    stream.push({ type: "done", reason: "toolUse", message: complete });
  });
  return stream;
}

function multipleToolCallStream(
  calls: { id: string; name: string; args: Record<string, unknown> }[],
) {
  const stream = createAssistantMessageEventStream();
  queueMicrotask(() => {
    const empty = assistantMessage([]);
    const toolCalls = calls.map(({ id, name, args }) => ({
      type: "toolCall" as const,
      id,
      name,
      arguments: args,
    }));
    let partial = empty;
    stream.push({ type: "start", partial });
    toolCalls.forEach((toolCall, contentIndex) => {
      stream.push({ type: "toolcall_start", contentIndex, partial });
      partial = assistantMessage(toolCalls.slice(0, contentIndex + 1), "toolUse");
      stream.push({
        type: "toolcall_end",
        contentIndex,
        toolCall,
        partial,
      });
    });
    stream.push({ type: "done", reason: "toolUse", message: partial });
  });
  return stream;
}

function assistantMessage(
  content: AssistantMessage["content"],
  stopReason: AssistantMessage["stopReason"] = "stop",
): AssistantMessage {
  return {
    role: "assistant",
    content,
    api: "openai-completions",
    provider: "openai",
    model: "local-model",
    usage: {
      input: 0,
      output: 0,
      cacheRead: 0,
      cacheWrite: 0,
      totalTokens: 0,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
    },
    stopReason,
    timestamp: 1,
  };
}
