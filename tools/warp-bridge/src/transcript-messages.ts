import type { AgentMessage } from "@earendil-works/pi-agent-core";
import type {
  AssistantMessage,
  ImageContent,
  TextContent,
} from "@earendil-works/pi-ai/compat";

import type { RuntimeContentBlock, TranscriptItem } from "./protocol.js";

export function transcriptMessages(
  items: TranscriptItem[],
  currentResourceIds: ReadonlySet<string>,
): AgentMessage[] {
  const messages: AgentMessage[] = [];
  const pendingToolNames = new Map<string, string>();
  for (const item of items) {
    if (item.kind === "message") {
      if (item.role === "user") {
        messages.push({ role: "user", content: toPiContent(item.content), timestamp: Date.now() });
        continue;
      }
      if (item.content.some((content) => content.type === "image")) {
        throw new Error("Text-only runtime cannot replay assistant images");
      }
      messages.push(replayedAssistant(item.content));
      continue;
    }
    if (item.kind === "resource_snapshot") {
      if (currentResourceIds.has(item.resource_id)) {
        const images = item.content.filter((content) => content.type === "image");
        if (images.length > 0) {
          messages.push({
            role: "user",
            content: [
              { type: "text", text: `Resource: ${item.name}` },
              ...toPiContent(images),
            ],
            timestamp: Date.now(),
          });
        }
        continue;
      }
      const content: RuntimeContentBlock[] = [
        { type: "text", text: `Resource: ${item.name}` },
        ...item.content,
      ];
      messages.push({ role: "user", content: toPiContent(content), timestamp: Date.now() });
      continue;
    }
    if (item.kind === "tool_request") {
      if (pendingToolNames.has(item.tool_call_id)) {
        throw new Error("Tool request reused an active tool call ID");
      }
      pendingToolNames.set(item.tool_call_id, item.tool_name);
      messages.push(replayedToolRequest(item));
      continue;
    }
    const toolName = pendingToolNames.get(item.tool_call_id);
    if (toolName === undefined) {
      throw new Error("Tool result is missing its paired request");
    }
    pendingToolNames.delete(item.tool_call_id);
    messages.push({
      role: "toolResult",
      toolCallId: item.tool_call_id,
      toolName,
      content: toPiContent(item.result.content),
      details: item.result,
      isError: item.result.status !== "success",
      timestamp: Date.now(),
    });
  }
  if (pendingToolNames.size > 0) {
    throw new Error("Tool request is missing its paired result");
  }
  return messages;
}

function toPiContent(content: RuntimeContentBlock[]): (TextContent | ImageContent)[] {
  return content.map((block) =>
    block.type === "text"
      ? { type: "text", text: block.text }
      : { type: "image", data: block.data_base64, mimeType: block.mime_type },
  );
}

function replayedAssistant(content: RuntimeContentBlock[]): AssistantMessage {
  return {
    role: "assistant",
    content: content.map((block) => ({ type: "text", text: block.type === "text" ? block.text : "" })),
    api: "openai-completions",
    provider: "openai",
    model: "replayed",
    usage: emptyUsage(),
    stopReason: "stop",
    timestamp: Date.now(),
  };
}

function replayedToolRequest(
  request: Extract<TranscriptItem, { kind: "tool_request" }>,
): AssistantMessage {
  return {
    role: "assistant",
    content: [
      {
        type: "toolCall",
        id: request.tool_call_id,
        name: request.tool_name,
        arguments: request.arguments,
      },
    ],
    api: "openai-completions",
    provider: "openai",
    model: "replayed",
    usage: emptyUsage(),
    stopReason: "toolUse",
    timestamp: Date.now(),
  };
}

function emptyUsage(): AssistantMessage["usage"] {
  return {
    input: 0,
    output: 0,
    cacheRead: 0,
    cacheWrite: 0,
    totalTokens: 0,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
  };
}
