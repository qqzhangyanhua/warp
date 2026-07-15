import type { AgentTool } from "@earendil-works/pi-agent-core";
import { Unsafe, type TSchema, type TUnsafe } from "typebox";

import type {
  RunStart,
  RuntimeContentBlock,
  ToolRequest,
  ToolResult,
} from "./protocol.js";

type ProxyParameters = TUnsafe<Record<string, unknown>>;

export interface ProxyToolSet {
  tools: AgentTool<ProxyParameters, ToolResult>[];
  takeResultError(toolCallId: string): boolean;
  terminalOutcome(): ProxyTerminalOutcome | undefined;
}

export type ProxyTerminalOutcome = "limit_reached" | "tool_outcome_unknown";

export function createProxyTools(
  start: RunStart,
  execute: (request: ToolRequest) => Promise<ToolResult>,
): ProxyToolSet {
  const resultErrors = new Map<string, boolean>();
  let terminalOutcome: ProxyTerminalOutcome | undefined;
  const tools = start.configuration.tools.map(
    (entry): AgentTool<ProxyParameters, ToolResult> => ({
      name: entry.name,
      label: entry.name,
      description: entry.description,
      parameters: Unsafe<Record<string, unknown>>(entry.input_schema as TSchema),
      executionMode: "sequential",
      execute: async (toolCallId, params) => {
        const result = await execute({
          type: "tool_request",
          conversation_id: start.conversation_id,
          run_id: start.run_id,
          tool_call_id: toolCallId,
          tool_id: entry.id,
          tool_name: entry.name,
          arguments: params,
        });
        resultErrors.set(toolCallId, result.status !== "success");
        if (
          result.status === "error" &&
          result.error_code === "tool_request_limit_exceeded"
        ) {
          terminalOutcome = "limit_reached";
        } else if (
          result.status === "error" &&
          result.error_code === "tool_outcome_unknown"
        ) {
          terminalOutcome = "tool_outcome_unknown";
        }
        return {
          content: result.content.map(runtimeContent),
          details: result,
          // Return control to the Bridge after every batch. The Bridge explicitly
          // continues ordinary batches and stops terminal Warp outcomes.
          terminate: true,
        };
      },
    }),
  );
  return {
    tools,
    takeResultError(toolCallId) {
      const isError = resultErrors.get(toolCallId) ?? false;
      resultErrors.delete(toolCallId);
      return isError;
    },
    terminalOutcome() {
      return terminalOutcome;
    },
  };
}

function runtimeContent(block: RuntimeContentBlock) {
  switch (block.type) {
    case "text":
      return { type: "text" as const, text: block.text };
    case "image":
      return {
        type: "image" as const,
        data: block.data_base64,
        mimeType: block.mime_type,
      };
  }
}
