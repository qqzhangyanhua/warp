use std::collections::{BTreeMap, HashMap, HashSet};

use warp_multi_agent_api as api;

use super::transcript::{
    RunScopedToolCallId, RuntimeContentBlock, RuntimeTranscript, ToolResultProjection,
    TranscriptItem, TranscriptRole,
};
use crate::ai::agent::conversation::{AIConversation, AIConversationId};

#[test]
fn projects_completed_text_messages_in_conversation_order() {
    let conversation = restored_conversation(vec![
        user_message("user-1", "run-1", "Inspect the workspace"),
        assistant_message("assistant-1", "run-1", "The workspace is clean."),
        reasoning_message("reasoning-1", "run-1", "private reasoning"),
        tool_message("tool-1", "run-1"),
        user_message("user-2", "run-2", "Try again"),
        assistant_message("assistant-2", "run-2", "A completed step."),
        assistant_message("interrupted:run-2", "run-2", "partial output"),
    ]);
    let interrupted_message_ids = HashSet::from(["interrupted:run-2".to_string()]);

    let transcript =
        RuntimeTranscript::project(&conversation, 7, &interrupted_message_ids, &HashMap::new())
            .unwrap();

    assert_eq!(transcript.revision(), 7);
    assert_eq!(
        transcript.items(),
        [
            text_item("user-1", TranscriptRole::User, "Inspect the workspace"),
            text_item(
                "assistant-1",
                TranscriptRole::Assistant,
                "The workspace is clean.",
            ),
            text_item("user-2", TranscriptRole::User, "Try again"),
            text_item(
                "assistant-2",
                TranscriptRole::Assistant,
                "A completed step."
            ),
        ]
    );
}

#[test]
fn projects_retained_resources_after_their_initiating_user_message() {
    let mut user = user_message("user-1", "run-1", "Review the rules");
    let Some(api::message::Message::UserQuery(query)) = user.message.as_mut() else {
        unreachable!();
    };
    query.context = Some(api::InputContext {
        project_rules: vec![api::input_context::ProjectRules {
            root_path: "/workspace".to_string(),
            active_rule_files: vec![api::FileContent {
                file_path: "/workspace/AGENTS.md".to_string(),
                content: "Retained rules".to_string(),
                line_range: None,
            }],
            additional_rule_file_paths: vec![],
        }],
        ..Default::default()
    });
    let conversation = restored_conversation(vec![user]);

    let transcript =
        RuntimeTranscript::project(&conversation, 3, &HashSet::new(), &HashMap::new()).unwrap();

    assert_eq!(
        transcript.items(),
        [
            text_item("user-1", TranscriptRole::User, "Review the rules"),
            TranscriptItem::ResourceSnapshot {
                resource_id: "user-1:rule:0:0".to_string(),
                name: "/workspace/AGENTS.md".to_string(),
                content: vec![RuntimeContentBlock::Text {
                    text: "Retained rules".to_string(),
                }],
            },
        ]
    );
}

#[test]
fn projects_paired_tool_activity_with_the_fixed_result_projection() {
    let conversation = restored_conversation(vec![
        user_message("user-1", "run-1", "Show the working directory"),
        shell_tool_message("tool-1", "run-1", "call-1", "pwd"),
        tool_result_message("result-1", "run-1", "call-1"),
    ]);
    let result_projections = HashMap::from([(
        RunScopedToolCallId::new("run-1", "call-1"),
        ToolResultProjection::Success {
            content: vec![RuntimeContentBlock::Text {
                text: "/workspace".to_string(),
            }],
            truncated: false,
        },
    )]);

    let transcript =
        RuntimeTranscript::project(&conversation, 5, &HashSet::new(), &result_projections).unwrap();

    assert_eq!(
        transcript.items(),
        [
            text_item("user-1", TranscriptRole::User, "Show the working directory"),
            TranscriptItem::ToolRequest {
                tool_call_id: "call-1".to_string(),
                tool_id: "builtin.run_shell_command".to_string(),
                tool_name: "run_shell_command".to_string(),
                arguments: serde_json::Map::from_iter([(
                    "command".to_string(),
                    serde_json::Value::String("pwd".to_string()),
                )]),
            },
            TranscriptItem::ToolResult {
                tool_call_id: "call-1".to_string(),
                result: ToolResultProjection::Success {
                    content: vec![RuntimeContentBlock::Text {
                        text: "/workspace".to_string(),
                    }],
                    truncated: false,
                },
            },
        ]
    );
}

#[test]
fn scopes_tool_result_projections_to_their_agent_run() {
    let conversation = restored_conversation(vec![
        shell_tool_message("tool-1", "run-1", "call-1", "pwd"),
        tool_result_message("result-1", "run-1", "call-1"),
        shell_tool_message("tool-2", "run-2", "call-1", "pwd"),
        tool_result_message("result-2", "run-2", "call-1"),
    ]);
    let result_projections = HashMap::from([
        (
            RunScopedToolCallId::new("run-1", "call-1"),
            success_projection("/first"),
        ),
        (
            RunScopedToolCallId::new("run-2", "call-1"),
            success_projection("/second"),
        ),
    ]);

    let transcript =
        RuntimeTranscript::project(&conversation, 5, &HashSet::new(), &result_projections).unwrap();

    let results = transcript
        .items()
        .iter()
        .filter_map(|item| match item {
            TranscriptItem::ToolResult { result, .. } => Some(result),
            TranscriptItem::Message { .. }
            | TranscriptItem::ResourceSnapshot { .. }
            | TranscriptItem::ToolRequest { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        [
            &success_projection("/first"),
            &success_projection("/second")
        ]
    );
}

#[test]
fn projects_all_initial_tool_request_families() {
    let conversation = restored_conversation(vec![
        read_files_tool_message("tool-1", "run-1", "read-1"),
        tool_result_message("result-1", "run-1", "read-1"),
        apply_diffs_tool_message("tool-2", "run-1", "apply-1"),
        tool_result_message("result-2", "run-1", "apply-1"),
        mcp_tool_message("tool-3", "run-1", "mcp-1"),
        tool_result_message("result-3", "run-1", "mcp-1"),
    ]);
    let result_projections = ["read-1", "apply-1", "mcp-1"]
        .into_iter()
        .map(|tool_call_id| {
            (
                RunScopedToolCallId::new("run-1", tool_call_id),
                success_projection("done"),
            )
        })
        .collect();

    let transcript =
        RuntimeTranscript::project(&conversation, 5, &HashSet::new(), &result_projections).unwrap();

    let requests = transcript
        .items()
        .iter()
        .filter(|item| matches!(item, TranscriptItem::ToolRequest { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        requests,
        [
            &TranscriptItem::ToolRequest {
                tool_call_id: "read-1".to_string(),
                tool_id: "builtin.read_files".to_string(),
                tool_name: "read_files".to_string(),
                arguments: serde_json::json!({
                    "files": [{"name": "README.md", "line_ranges": [{"start": 1, "end": 4}]}]
                })
                .as_object()
                .unwrap()
                .clone(),
            },
            &TranscriptItem::ToolRequest {
                tool_call_id: "apply-1".to_string(),
                tool_id: "builtin.apply_file_diffs".to_string(),
                tool_name: "apply_file_diffs".to_string(),
                arguments: serde_json::json!({
                    "summary": "Update docs",
                    "diffs": [{"file_path": "README.md", "search": "old", "replace": "new"}],
                    "new_files": [],
                    "deleted_files": [],
                    "v4a_updates": []
                })
                .as_object()
                .unwrap()
                .clone(),
            },
            &TranscriptItem::ToolRequest {
                tool_call_id: "mcp-1".to_string(),
                tool_id: "mcp:123e4567-e89b-12d3-a456-426614174000:web.search".to_string(),
                tool_name: "mcp_123e4567e89b12d3a456426614174000_web_search_75898d89".to_string(),
                arguments: serde_json::json!({"query": "rust"})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ]
    );
}

fn restored_conversation(messages: Vec<api::Message>) -> AIConversation {
    AIConversation::new_restored(
        AIConversationId::new(),
        vec![api::Task {
            id: "root-task".to_string(),
            messages,
            dependencies: None,
            description: String::new(),
            summary: String::new(),
            server_data: String::new(),
        }],
        None,
    )
    .unwrap()
}

fn user_message(id: &str, run_id: &str, text: &str) -> api::Message {
    message(
        id,
        run_id,
        api::message::Message::UserQuery(api::message::UserQuery {
            query: text.to_string(),
            context: None,
            referenced_attachments: HashMap::new(),
            mode: None,
            intended_agent: Default::default(),
        }),
    )
}

fn assistant_message(id: &str, run_id: &str, text: &str) -> api::Message {
    message(
        id,
        run_id,
        api::message::Message::AgentOutput(api::message::AgentOutput {
            text: text.to_string(),
        }),
    )
}

fn reasoning_message(id: &str, run_id: &str, text: &str) -> api::Message {
    message(
        id,
        run_id,
        api::message::Message::AgentReasoning(api::message::AgentReasoning {
            reasoning: text.to_string(),
            finished_duration: None,
        }),
    )
}

fn tool_message(id: &str, run_id: &str) -> api::Message {
    message(
        id,
        run_id,
        api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: "call-1".to_string(),
            tool: None,
        }),
    )
}

fn shell_tool_message(id: &str, run_id: &str, tool_call_id: &str, command: &str) -> api::Message {
    message(
        id,
        run_id,
        api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: tool_call_id.to_string(),
            tool: Some(api::message::tool_call::Tool::RunShellCommand(
                api::message::tool_call::RunShellCommand {
                    command: command.to_string(),
                    ..Default::default()
                },
            )),
        }),
    )
}

fn tool_result_message(id: &str, run_id: &str, tool_call_id: &str) -> api::Message {
    message(
        id,
        run_id,
        api::message::Message::ToolCallResult(api::message::ToolCallResult {
            tool_call_id: tool_call_id.to_string(),
            context: None,
            result: None,
        }),
    )
}

fn read_files_tool_message(id: &str, run_id: &str, tool_call_id: &str) -> api::Message {
    message(
        id,
        run_id,
        api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: tool_call_id.to_string(),
            tool: Some(api::message::tool_call::Tool::ReadFiles(
                api::message::tool_call::ReadFiles {
                    files: vec![api::message::tool_call::read_files::File {
                        name: "README.md".to_string(),
                        line_ranges: vec![api::FileContentLineRange { start: 1, end: 4 }],
                    }],
                },
            )),
        }),
    )
}

fn apply_diffs_tool_message(id: &str, run_id: &str, tool_call_id: &str) -> api::Message {
    message(
        id,
        run_id,
        api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: tool_call_id.to_string(),
            tool: Some(api::message::tool_call::Tool::ApplyFileDiffs(
                api::message::tool_call::ApplyFileDiffs {
                    summary: "Update docs".to_string(),
                    diffs: vec![api::message::tool_call::apply_file_diffs::FileDiff {
                        file_path: "README.md".to_string(),
                        search: "old".to_string(),
                        replace: "new".to_string(),
                    }],
                    ..Default::default()
                },
            )),
        }),
    )
}

fn mcp_tool_message(id: &str, run_id: &str, tool_call_id: &str) -> api::Message {
    message(
        id,
        run_id,
        api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: tool_call_id.to_string(),
            tool: Some(api::message::tool_call::Tool::CallMcpTool(
                api::message::tool_call::CallMcpTool {
                    name: "web.search".to_string(),
                    args: Some(prost_types::Struct {
                        fields: BTreeMap::from([(
                            "query".to_string(),
                            prost_types::Value {
                                kind: Some(prost_types::value::Kind::StringValue(
                                    "rust".to_string(),
                                )),
                            },
                        )]),
                    }),
                    server_id: "123e4567-e89b-12d3-a456-426614174000".to_string(),
                },
            )),
        }),
    )
}

fn message(id: &str, run_id: &str, content: api::message::Message) -> api::Message {
    api::Message {
        id: id.to_string(),
        task_id: "root-task".to_string(),
        request_id: run_id.to_string(),
        message: Some(content),
        ..Default::default()
    }
}

fn text_item(id: &str, role: TranscriptRole, text: &str) -> TranscriptItem {
    TranscriptItem::Message {
        message_id: id.to_string(),
        role,
        content: vec![RuntimeContentBlock::Text {
            text: text.to_string(),
        }],
    }
}

fn success_projection(text: &str) -> ToolResultProjection {
    ToolResultProjection::Success {
        content: vec![RuntimeContentBlock::Text {
            text: text.to_string(),
        }],
        truncated: false,
    }
}
