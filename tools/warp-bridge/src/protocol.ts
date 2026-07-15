import { Ajv2020 } from "ajv/dist/2020.js";

import coreSchema from "../protocol/core-v2.schema.json" with { type: "json" };

export interface ProtocolCapability {
  name: string;
  version: number;
  schema_hash: string;
}

export interface BridgeHello {
  type: "bridge_hello";
  protocol_version: 2;
  core_schema_hash: string;
  bridge_version: string;
  capabilities: ProtocolCapability[];
  prompt_version: string;
}

export interface AcceptedHandshakeResult {
  type: "handshake_result";
  status: "accepted";
  max_frame_bytes: number;
  max_transcript_bytes: number;
}

export type HandshakeRejectionCode =
  | "protocol_version_mismatch"
  | "core_schema_mismatch"
  | "missing_required_capability";

export interface RejectedHandshakeResult {
  type: "handshake_result";
  status: "rejected";
  error_code: HandshakeRejectionCode;
  diagnostic_id: string;
}

export type HandshakeResult = AcceptedHandshakeResult | RejectedHandshakeResult;

export interface TextContentBlock {
  type: "text";
  text: string;
}

export interface ImageContentBlock {
  type: "image";
  mime_type: "image/gif" | "image/jpeg" | "image/png" | "image/webp";
  data_base64: string;
}

export type RuntimeContentBlock = TextContentBlock | ImageContentBlock;

export interface TranscriptMessage {
  kind: "message";
  message_id: string;
  role: "user" | "assistant";
  content: RuntimeContentBlock[];
}

export interface TranscriptResourceSnapshot {
  kind: "resource_snapshot";
  resource_id: string;
  name: string;
  content: RuntimeContentBlock[];
}

export interface TranscriptToolRequest {
  kind: "tool_request";
  tool_call_id: string;
  tool_id: string;
  tool_name: string;
  arguments: Record<string, unknown>;
}

export type TranscriptToolResultProjection =
  | {
      status: "success";
      content: RuntimeContentBlock[];
      truncated: boolean;
    }
  | {
      status: "denied";
      denied_by: "policy" | "user";
      content: RuntimeContentBlock[];
      truncated: boolean;
    }
  | {
      status: "error";
      error_code: OrdinaryToolErrorCode;
      may_have_executed: false;
      content: RuntimeContentBlock[];
      truncated: boolean;
    }
  | {
      status: "error";
      error_code: "tool_outcome_unknown";
      may_have_executed: true;
      content: RuntimeContentBlock[];
      truncated: boolean;
    };

export interface TranscriptToolResult {
  kind: "tool_result";
  tool_call_id: string;
  result: TranscriptToolResultProjection;
}

export type TranscriptItem =
  | TranscriptMessage
  | TranscriptResourceSnapshot
  | TranscriptToolRequest
  | TranscriptToolResult;

export interface TranscriptSyncBegin {
  type: "transcript_sync_begin";
  sync_id: string;
  conversation_id: string;
  revision: number;
  item_count: number;
  total_bytes: number;
}

export interface TranscriptSyncItem {
  type: "transcript_sync_item";
  sync_id: string;
  index: number;
  item: TranscriptItem;
}

export interface TranscriptSyncCommit {
  type: "transcript_sync_commit";
  sync_id: string;
}

export interface AcceptedTranscriptSyncResult {
  type: "transcript_sync_result";
  sync_id: string;
  status: "accepted";
  revision: number;
}

export interface ProviderConfiguration {
  protocol: "chat_completions";
  base_url: string;
  provider_origin: string;
  model: string;
  api_key: string;
  max_provider_attempts: number;
  max_redirects: number;
}

export interface ToolCatalogEntry {
  id: string;
  name: string;
  description: string;
  input_schema: Record<string, unknown>;
}

export interface AgentResource {
  id: string;
  name: string;
  content: RuntimeContentBlock[];
}

export interface RunConfiguration {
  provider: ProviderConfiguration;
  working_directory: string;
  context_limit: number;
  reasoning_effort: "none" | "minimal" | "low" | "medium" | "high" | "xhigh";
  tool_request_limit: number;
  tools: ToolCatalogEntry[];
  resources: AgentResource[];
}

export interface RunStart {
  type: "run_start";
  conversation_id: string;
  run_id: string;
  transcript_revision: number;
  configuration: RunConfiguration;
}

export interface RunStatus {
  type: "run_status";
  conversation_id: string;
  run_id: string;
  status: "running" | "waiting_for_commit" | "waiting_for_tool_result";
}

export interface TextDelta {
  type: "text_delta";
  conversation_id: string;
  run_id: string;
  event_id: string;
  delta: string;
}

export interface AssistantMessageCommit {
  type: "assistant_message_commit";
  conversation_id: string;
  run_id: string;
  event_id: string;
  commit_id: string;
  message_id: string;
  expected_revision: number;
  content: RuntimeContentBlock[];
}

export interface CommittedResult {
  type: "commit_result";
  conversation_id: string;
  run_id: string;
  commit_id: string;
  status: "committed";
  revision: number;
}

export interface RunCancel {
  type: "run_cancel";
  conversation_id: string;
  run_id: string;
}

export interface CompletedRun {
  type: "run_finished";
  conversation_id: string;
  run_id: string;
  outcome: "completed";
}

export interface CancelledRun {
  type: "run_finished";
  conversation_id: string;
  run_id: string;
  outcome: "cancelled";
}

export type RunFailureCode =
  | "bridge_protocol_error"
  | "commit_timeout"
  | "provider_http_error"
  | "provider_protocol_error"
  | "provider_redirect_not_allowed"
  | "provider_transport_error"
  | "revision_conflict"
  | "runtime_failure"
  | "transcript_sync_error";

export interface FailedRun {
  type: "run_finished";
  conversation_id: string;
  run_id: string;
  outcome: "failed";
  error_code: RunFailureCode;
  diagnostic_id: string;
}

export interface LimitReachedRun {
  type: "run_finished";
  conversation_id: string;
  run_id: string;
  outcome: "limit_reached";
  tool_request_limit: number;
}

export interface ToolRequest {
  type: "tool_request";
  conversation_id: string;
  run_id: string;
  tool_call_id: string;
  tool_id: string;
  tool_name: string;
  arguments: Record<string, unknown>;
}

interface ToolResultBase {
  type: "tool_result";
  conversation_id: string;
  run_id: string;
  tool_call_id: string;
  content: RuntimeContentBlock[];
  truncated: boolean;
}

export interface SuccessfulToolResult extends ToolResultBase {
  status: "success";
}

export interface DeniedToolResult extends ToolResultBase {
  status: "denied";
  denied_by: "policy" | "user";
}

export type OrdinaryToolErrorCode =
  | "invalid_tool_request"
  | "tool_execution_failed"
  | "tool_request_limit_exceeded";

export interface OrdinaryToolErrorResult extends ToolResultBase {
  status: "error";
  error_code: OrdinaryToolErrorCode;
  may_have_executed: false;
}

export interface IndeterminateToolResult extends ToolResultBase {
  status: "error";
  error_code: "tool_outcome_unknown";
  may_have_executed: true;
}

export type ToolResult =
  | SuccessfulToolResult
  | DeniedToolResult
  | OrdinaryToolErrorResult
  | IndeterminateToolResult;

export type ProtocolMessage =
  | BridgeHello
  | HandshakeResult
  | TranscriptSyncBegin
  | TranscriptSyncItem
  | TranscriptSyncCommit
  | AcceptedTranscriptSyncResult
  | RunStart
  | RunStatus
  | TextDelta
  | AssistantMessageCommit
  | CommittedResult
  | RunCancel
  | CompletedRun
  | CancelledRun
  | FailedRun
  | LimitReachedRun
  | ToolRequest
  | ToolResult;

export class ProtocolError extends Error {
  constructor() {
    super("Invalid Bridge Protocol message");
    this.name = "ProtocolError";
  }
}

const ajv = new Ajv2020({ allErrors: false, strict: true });
const validateProtocolMessage = ajv.compile<ProtocolMessage>(coreSchema);

export function parseProtocolLine(
  line: string,
  maxFrameBytes = Number.MAX_SAFE_INTEGER,
): ProtocolMessage {
  if (Buffer.byteLength(line, "utf8") > maxFrameBytes) {
    throw new ProtocolError();
  }

  let value: unknown;
  try {
    value = JSON.parse(line);
  } catch {
    throw new ProtocolError();
  }

  if (!validateProtocolMessage(value)) {
    throw new ProtocolError();
  }
  return value;
}
