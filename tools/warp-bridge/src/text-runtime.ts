import { Agent, type StreamFn } from "@earendil-works/pi-agent-core";
import { streamSimple, type AssistantMessage, type Model } from "@earendil-works/pi-ai/compat";

import {
  createProviderFetch,
  type ProviderTransportFailure,
} from "./provider-transport.js";
import { createProxyTools } from "./proxy-tools.js";
import type { ProxyTerminalOutcome } from "./proxy-tools.js";
import type {
  AssistantMessageCommit,
  CancelledRun,
  CommittedResult,
  CompletedRun,
  FailedRun,
  LimitReachedRun,
  RunConfiguration,
  RunStart,
  RunStatus,
  RuntimeContentBlock,
  TextDelta,
  ToolRequest,
  ToolResult,
  TranscriptItem,
} from "./protocol.js";
import { transcriptMessages } from "./transcript-messages.js";

export type TextRunEvent =
  | RunStatus
  | TextDelta
  | CompletedRun
  | CancelledRun
  | FailedRun
  | LimitReachedRun;

export interface TextRunCallbacks {
  emit(event: TextRunEvent): void | Promise<void>;
  commit(request: AssistantMessageCommit): Promise<CommittedResult>;
  tool?(request: ToolRequest): Promise<ToolResult>;
}

export interface TextRuntime {
  cancel(): void;
  run(
    transcript: TranscriptItem[],
    start: RunStart,
    callbacks: TextRunCallbacks,
  ): Promise<void>;
}

interface PiTextRuntimeOptions {
  stream?: StreamFn;
  createId?: () => string;
  commitTimeoutMs?: number;
}

type AttemptFailure =
  | "provider_http"
  | "provider_protocol"
  | "provider_redirect"
  | "provider_transport"
  | "commit"
  | "protocol"
  | "cancelled";

export class PiTextRuntime implements TextRuntime {
  private readonly providerStream: StreamFn;
  private readonly createId: () => string;
  private readonly commitTimeoutMs: number;
  private activeAgent: Agent | undefined;

  constructor(options: PiTextRuntimeOptions = {}) {
    this.providerStream = options.stream ?? streamSimple;
    this.createId = options.createId ?? (() => crypto.randomUUID());
    this.commitTimeoutMs = options.commitTimeoutMs ?? 30_000;
  }

  cancel(): void {
    this.activeAgent?.abort();
  }

  async run(
    transcript: TranscriptItem[],
    start: RunStart,
    callbacks: TextRunCallbacks,
  ): Promise<void> {
    validateTextRun(start);
    let revision = start.transcript_revision;

    for (
      let attempt = 1;
      attempt <= start.configuration.provider.max_provider_attempts;
      attempt += 1
    ) {
      const result = await this.runAttempt(transcript, start, revision, callbacks);
      revision = result.revision;
      if (result.terminalOutcome === "limit_reached") {
        await callbacks.emit({
          type: "run_finished",
          conversation_id: start.conversation_id,
          run_id: start.run_id,
          outcome: "limit_reached",
          tool_request_limit: start.configuration.tool_request_limit,
        });
        return;
      }
      if (result.terminalOutcome === "tool_outcome_unknown") {
        await callbacks.emit({
          type: "run_finished",
          conversation_id: start.conversation_id,
          run_id: start.run_id,
          outcome: "failed",
          error_code: "runtime_failure",
          diagnostic_id: this.createId(),
        });
        return;
      }
      if (result.failure === undefined) {
        await callbacks.emit({
          type: "run_finished",
          conversation_id: start.conversation_id,
          run_id: start.run_id,
          outcome: "completed",
        });
        return;
      }
      if (
        result.retryableProviderFailure &&
        !result.emittedOutput &&
        attempt < start.configuration.provider.max_provider_attempts
      ) {
        continue;
      }

      if (result.failure === "cancelled") {
        await callbacks.emit({
          type: "run_finished",
          conversation_id: start.conversation_id,
          run_id: start.run_id,
          outcome: "cancelled",
        });
        return;
      }
      await callbacks.emit({
        type: "run_finished",
        conversation_id: start.conversation_id,
        run_id: start.run_id,
        outcome: "failed",
        error_code: failureCode(result.failure),
        diagnostic_id: this.createId(),
      });
      return;
    }
  }

  private async runAttempt(
    transcript: TranscriptItem[],
    start: RunStart,
    revision: number,
    callbacks: TextRunCallbacks,
  ): Promise<{
    revision: number;
    emittedOutput: boolean;
    retryableProviderFailure: boolean;
    terminalOutcome?: ProxyTerminalOutcome;
    failure?: AttemptFailure;
  }> {
    let eventId = this.createId();
    let emittedOutput = false;
    let failure: AttemptFailure | undefined;
    let retryableProviderFailure = false;
    let providerTransportFailure: ProviderTransportFailure | undefined;
    let committedRevision = revision;
    const configuration = start.configuration;
    const proxyTools = createProxyTools(start, (request) => {
      if (callbacks.tool === undefined) {
        throw new Error("Warp tool callback is unavailable");
      }
      return callbacks.tool(request);
    });
    const agent = new Agent({
      initialState: {
        systemPrompt: buildAgentPolicyPrompt(configuration),
        model: providerModel(configuration),
        thinkingLevel:
          configuration.reasoning_effort === "none"
            ? "off"
            : configuration.reasoning_effort,
        tools: proxyTools.tools,
        messages: transcriptMessages(
          transcript,
          new Set(configuration.resources.map((resource) => resource.id)),
        ),
      },
      getApiKey: () => configuration.provider.api_key,
      streamFn: (model, context, options) =>
        this.providerStream(model, context, { ...options, maxRetries: 0 }),
      toolExecution: "sequential",
      afterToolCall: async ({ toolCall }) => ({
        isError: proxyTools.takeResultError(toolCall.id),
      }),
    });
    this.activeAgent = agent;

    agent.subscribe(async (event, signal) => {
      if (
        event.type === "message_update" &&
        event.assistantMessageEvent.type === "text_delta"
      ) {
        emittedOutput = true;
        await callbacks.emit({
          type: "text_delta",
          conversation_id: start.conversation_id,
          run_id: start.run_id,
          event_id: eventId,
          delta: event.assistantMessageEvent.delta,
        });
        return;
      }
      if (
        event.type === "message_update" &&
        (event.assistantMessageEvent.type === "thinking_delta" ||
          event.assistantMessageEvent.type === "toolcall_delta")
      ) {
        emittedOutput = true;
        return;
      }
      if (event.type !== "message_end" || event.message.role !== "assistant") {
        return;
      }
      if (event.message.stopReason === "aborted" || signal.aborted) {
        failure = "cancelled";
        return;
      }
      if (event.message.stopReason === "error") {
        const providerFailure = classifyProviderFailure(
          providerTransportFailure,
          event.message.errorMessage,
        );
        failure = providerFailure.failure;
        retryableProviderFailure = providerFailure.retryable;
        return;
      }
      const hasToolCall = event.message.content.some(
        (content) => content.type === "toolCall",
      );
      const content = completedTextContent(event.message);
      if (content.length === 0) {
        if (hasToolCall) {
          eventId = this.createId();
          return;
        }
        failure = "protocol";
        return;
      }

      await callbacks.emit({
        type: "run_status",
        conversation_id: start.conversation_id,
        run_id: start.run_id,
        status: "waiting_for_commit",
      });
      const commitId = this.createId();
      const request: AssistantMessageCommit = {
        type: "assistant_message_commit",
        conversation_id: start.conversation_id,
        run_id: start.run_id,
        event_id: eventId,
        commit_id: commitId,
        message_id: this.createId(),
        expected_revision: committedRevision,
        content,
      };
      try {
        const acknowledgement = await withTimeout(
          callbacks.commit(request),
          this.commitTimeoutMs,
        );
        if (
          acknowledgement.conversation_id !== start.conversation_id ||
          acknowledgement.run_id !== start.run_id ||
          acknowledgement.commit_id !== commitId ||
          acknowledgement.revision !== committedRevision + 1
        ) {
          failure = "protocol";
          agent.abort();
          return;
        }
        committedRevision = acknowledgement.revision;
        eventId = this.createId();
      } catch {
        failure = agent.signal?.aborted ? "cancelled" : "commit";
        agent.abort();
      }
    });

    await callbacks.emit({
      type: "run_status",
      conversation_id: start.conversation_id,
      run_id: start.run_id,
      status: "running",
    });
    const previousFetch = globalThis.fetch;
    globalThis.fetch = createProviderFetch(
      configuration.provider.provider_origin,
      configuration.provider.max_redirects,
      previousFetch,
      (providerFailure) => {
        providerTransportFailure = providerFailure;
      },
    );
    try {
      do {
        await agent.continue();
      } while (
        proxyTools.terminalOutcome() === undefined &&
        agent.state.messages.at(-1)?.role === "toolResult"
      );
    } finally {
      globalThis.fetch = previousFetch;
      this.activeAgent = undefined;
    }
    if (failure === undefined && agent.state.errorMessage !== undefined) {
      if (agent.signal?.aborted) {
        failure = "cancelled";
      } else {
        const providerFailure = classifyProviderFailure(
          providerTransportFailure,
          agent.state.errorMessage,
        );
        failure = providerFailure.failure;
        retryableProviderFailure = providerFailure.retryable;
      }
    }
    const terminalOutcome = proxyTools.terminalOutcome();
    return failure === undefined
      ? {
          revision: committedRevision,
          emittedOutput,
          retryableProviderFailure,
          ...(terminalOutcome === undefined ? {} : { terminalOutcome }),
        }
      : {
          revision: committedRevision,
          emittedOutput,
          retryableProviderFailure,
          failure,
        };
  }
}

function validateTextRun(start: RunStart): void {
  const { configuration } = start;
  const providerUrl = new URL(configuration.provider.base_url);
  if (
    configuration.provider.protocol !== "chat_completions" ||
    providerUrl.origin !== configuration.provider.provider_origin ||
    !providerUrl.pathname.endsWith("/chat/completions") ||
    configuration.provider.max_provider_attempts < 1 ||
    configuration.provider.max_provider_attempts > 2 ||
    !validToolConfiguration(configuration.tool_request_limit, configuration.tools)
  ) {
    throw new Error("Invalid text-only Agent Run Configuration");
  }
}

function validToolConfiguration(
  toolRequestLimit: number,
  tools: RunConfiguration["tools"],
): boolean {
  if (tools.length === 0) {
    return toolRequestLimit === 0;
  }
  if (toolRequestLimit < 1 || toolRequestLimit > 32) {
    return false;
  }
  const ids = new Set(tools.map((tool) => tool.id));
  const names = new Set(tools.map((tool) => tool.name));
  return (
    ids.size === tools.length &&
    names.size === tools.length &&
    tools.every(
      (tool) =>
        tool.id.length > 0 &&
        tool.name.length > 0 &&
        tool.name.length <= 64 &&
        Object.keys(tool.input_schema).length > 0,
    )
  );
}

function providerModel(configuration: RunConfiguration): Model<"openai-completions"> {
  const baseUrl = new URL(configuration.provider.base_url);
  baseUrl.pathname = baseUrl.pathname.slice(0, -"/chat/completions".length) || "/";
  return {
    id: configuration.provider.model,
    name: configuration.provider.model,
    api: "openai-completions",
    provider: "openai",
    baseUrl: baseUrl.toString().replace(/\/$/, ""),
    reasoning: configuration.reasoning_effort !== "none",
    input: ["text", "image"],
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
    contextWindow: configuration.context_limit,
    maxTokens: Math.max(1, Math.min(8_192, Math.floor(configuration.context_limit / 4))),
  };
}

function completedTextContent(message: AssistantMessage): RuntimeContentBlock[] {
  return message.content.flatMap((content): RuntimeContentBlock[] =>
    content.type === "text" && content.text.length > 0
      ? [{ type: "text", text: content.text }]
      : [],
  );
}

function buildAgentPolicyPrompt(configuration: RunConfiguration): string {
  const resources = configuration.resources
    .map((resource) => `\nResource ${resource.name}:\n${resource.content
      .filter((content) => content.type === "text")
      .map((content) => content.text)
      .join("\n")}`)
    .join("\n");
  const tools = configuration.tools.length === 0
    ? "Tools are disabled for this run."
    : `Available tools: ${configuration.tools.map((tool) => tool.name).join(", ")}. Tool execution requires Warp approval.`;
  return `You are Warp's local Agent Runtime. ${tools} Working directory: ${configuration.working_directory}.${resources}`;
}

function failureCode(failure: AttemptFailure): FailedRun["error_code"] {
  switch (failure) {
    case "provider_http":
      return "provider_http_error";
    case "provider_protocol":
      return "provider_protocol_error";
    case "provider_redirect":
      return "provider_redirect_not_allowed";
    case "provider_transport":
      return "provider_transport_error";
    case "commit":
      return "commit_timeout";
    case "protocol":
      return "bridge_protocol_error";
    case "cancelled":
      return "runtime_failure";
  }
}

function classifyProviderFailure(
  transportFailure: ProviderTransportFailure | undefined,
  errorMessage: string | undefined,
): { failure: AttemptFailure; retryable: boolean } {
  if (transportFailure?.type === "http") {
    return {
      failure: "provider_http",
      retryable:
        transportFailure.status === 408 ||
        transportFailure.status === 429 ||
        (transportFailure.status >= 500 && transportFailure.status <= 599),
    };
  }
  if (transportFailure?.type === "transport") {
    return { failure: "provider_transport", retryable: true };
  }
  if (transportFailure?.type === "redirect_not_allowed") {
    return { failure: "provider_redirect", retryable: false };
  }
  if (errorMessage === PI_UNEXPECTED_EOF_ERROR_MESSAGE) {
    return { failure: "provider_transport", retryable: true };
  }
  return { failure: "provider_protocol", retryable: false };
}

const PI_UNEXPECTED_EOF_ERROR_MESSAGE = "Stream ended without finish_reason";

function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      reject(new Error("Commit acknowledgement timed out"));
    }, timeoutMs);
    promise.then(
      (value) => {
        clearTimeout(timeout);
        resolve(value);
      },
      (error: unknown) => {
        clearTimeout(timeout);
        reject(error);
      },
    );
  });
}
