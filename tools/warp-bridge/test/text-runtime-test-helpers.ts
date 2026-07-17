import { createAssistantMessageEventStream, type AssistantMessage } from "@earendil-works/pi-ai";

import type {
  AssistantMessageCommit,
  CommittedResult,
  RunStart,
  TranscriptItem,
} from "../src/protocol.js";

export function successfulCallbacks() {
  return {
    emit: () => {},
    commit: async (request: AssistantMessageCommit): Promise<CommittedResult> => ({
      type: "commit_result",
      conversation_id: request.conversation_id,
      run_id: request.run_id,
      commit_id: request.commit_id,
      status: "committed",
      revision: request.expected_revision + 1,
    }),
  };
}

type ScriptEvent =
  | { type: "text"; text: string }
  | { type: "thinking"; text: string }
  | { type: "request"; url: string }
  | {
      type: "error";
      errorMessage?: string;
      requestProvider?: boolean;
      consumeProviderBody?: boolean;
    };

export function scriptedStream(events: ScriptEvent[]) {
  const stream = createAssistantMessageEventStream();
  queueMicrotask(async () => {
    let text = "";
    stream.push({ type: "start", partial: assistantMessage(text) });
    for (const event of events) {
      if (event.type === "text") {
        text += event.text;
        stream.push({
          type: "text_delta",
          contentIndex: 0,
          delta: event.text,
          partial: assistantMessage(text),
        });
      } else if (event.type === "thinking") {
        stream.push({
          type: "thinking_delta",
          contentIndex: 0,
          delta: event.text,
          partial: assistantMessage(text),
        });
      } else if (event.type === "request") {
        try {
          await globalThis.fetch(event.url);
        } catch {
          // Network-policy rejection is an expected scripted event outcome.
        }
      } else {
        if (event.requestProvider === true) {
          try {
            const response = await globalThis.fetch(
              "https://provider.example/v1/chat/completions",
            );
            if (event.consumeProviderBody === true) {
              await response.text();
            }
          } catch {
            // The scripted stream still emits Pi's terminal error after transport rejection.
          }
        }
        const error = assistantMessage(text, "error", event.errorMessage);
        stream.push({ type: "error", reason: "error", error });
        return;
      }
    }
    const message = assistantMessage(text);
    stream.push({ type: "done", reason: "stop", message });
  });
  return stream;
}

function assistantMessage(
  text: string,
  stopReason: "stop" | "error" = "stop",
  errorMessage = "recognizable provider detail",
): AssistantMessage {
  const message: AssistantMessage = {
    role: "assistant",
    content: text.length === 0 ? [] : [{ type: "text", text }],
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
  if (stopReason === "error") {
    message.errorMessage = errorMessage;
  }
  return message;
}

export function transcript(): TranscriptItem[] {
  return [
    {
      kind: "message",
      message_id: "user-1",
      role: "user",
      content: [{ type: "text", text: "Inspect the workspace" }],
    },
  ];
}

export function runStart(): RunStart {
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

export function idSequence(): () => string {
  let next = 0;
  return () => `id-${++next}`;
}

export function deferred<T>(): {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (error: Error) => void;
} {
  let resolve = (_value: T): void => {
    throw new Error("deferred promise was not initialized");
  };
  let reject = (_error: Error): void => {
    throw new Error("deferred promise was not initialized");
  };
  const promise = new Promise<T>((resolver, rejecter) => {
    resolve = resolver;
    reject = rejecter;
  });
  return { promise, resolve, reject };
}
