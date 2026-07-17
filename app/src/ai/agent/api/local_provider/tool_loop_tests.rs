use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use futures::channel::oneshot;
use futures::stream;
use futures_lite::StreamExt as _;
use reqwest::header::HeaderMap;
use warp_multi_agent_api as api;
use warpui::r#async::BoxFuture;

use super::super::transport::{LocalProviderResponse, LocalProviderTransport, ProviderByteStream};
use super::super::{
    build_chat_completion_request, generate_local_provider_output_with_transport,
    ChatCompletionRequest, LocalProviderModel,
};
use super::{ChatFunctionCallDelta, ChatToolCallDelta, ToolCallAssembler, ToolCatalog};
use crate::ai::agent::api::RequestParams;
use crate::ai::agent::task::TaskId;
use crate::ai::agent::{
    AIAgentActionId, AIAgentActionResult, AIAgentActionResultType, AIAgentInput, CallMCPToolResult,
    MCPContext, MCPServer, RequestCommandOutputResult, UserQueryMode,
};
use crate::ai::llms::LLMId;
use crate::server::server_api::AIApiError;

struct ToolCallProviderTransport;

struct MCPToolCallProviderTransport;

struct InterruptedToolCallProviderTransport;

impl LocalProviderTransport for ToolCallProviderTransport {
    fn send(
        &self,
        _provider_model: LocalProviderModel,
        _request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        Box::pin(async {
            Ok(LocalProviderResponse {
                status: http::StatusCode::OK,
                headers: HeaderMap::new(),
                body: Box::pin(stream::iter([
                    Ok(Bytes::from_static(
                        br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-shell","type":"function","function":{"name":"run_shell_","arguments":"{\"command\":\"pwd"}},{"index":1,"id":"call-read","type":"function","function":{"name":"read_files","arguments":"{\"files\":[{\"name\":\"Cargo.toml\""}}]}}]}

"#,
                    )),
                    Ok(Bytes::from_static(
                        br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"command","arguments":"\"}"}},{"index":1,"function":{"arguments":"}]}"}}]}}]}

data: [DONE]

"#,
                    )),
                ])) as ProviderByteStream,
            })
        })
    }
}

impl LocalProviderTransport for MCPToolCallProviderTransport {
    fn send(
        &self,
        _provider_model: LocalProviderModel,
        _request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        Box::pin(async {
            Ok(LocalProviderResponse {
                status: http::StatusCode::OK,
                headers: HeaderMap::new(),
                body: Box::pin(stream::iter([Ok(Bytes::from_static(
                    br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-search","type":"function","function":{"name":"mcp_123e4567e89b12d3a456426614174000_web_search_75898d89","arguments":"{\"query\":\"rust async streams\"}"}}]}}]}

data: [DONE]

"#,
                ))])) as ProviderByteStream,
            })
        })
    }
}

impl LocalProviderTransport for InterruptedToolCallProviderTransport {
    fn send(
        &self,
        _provider_model: LocalProviderModel,
        _request: ChatCompletionRequest,
    ) -> BoxFuture<'static, Result<LocalProviderResponse, AIApiError>> {
        Box::pin(async {
            Ok(LocalProviderResponse {
                status: http::StatusCode::OK,
                headers: HeaderMap::new(),
                body: Box::pin(
                    stream::iter([Ok(Bytes::from_static(
                        br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-shell","function":{"name":"run_shell_command","arguments":"{\"command\":\"pwd\"}"}}]}}]}

"#,
                    ))])
                    .chain(stream::pending()),
                ) as ProviderByteStream,
            })
        })
    }
}

fn params_with_custom_model() -> RequestParams {
    let mut params = RequestParams::new_for_test();
    let model = LLMId::from("custom-model");
    params.model = model.clone();
    params.coding_model = model.clone();
    params.cli_agent_model = model.clone();
    params.computer_use_model = model;
    params.custom_model_providers = Some(api::request::settings::CustomModelProviders {
        providers: vec![
            api::request::settings::custom_model_providers::CustomModelProvider {
                base_url: "http://localhost:8080/v1".to_string(),
                api_key: "provider-key".to_string(),
                models: vec![
                    api::request::settings::custom_model_providers::CustomModel {
                        slug: "provider-model".to_string(),
                        config_key: "custom-model".to_string(),
                    },
                ],
            },
        ],
    });
    params.tasks = vec![api::Task {
        id: "task-1".to_string(),
        ..Default::default()
    }];
    params.input = vec![AIAgentInput::UserQuery {
        query: "hello".to_string(),
        context: Arc::from([]),
        static_query_type: None,
        referenced_attachments: HashMap::new(),
        user_query_mode: UserQueryMode::Normal,
        running_command: None,
        intended_agent: None,
    }];
    params
}

fn add_search_mcp_tool(params: &mut RequestParams) {
    let tool: rmcp::model::Tool = serde_json::from_value(serde_json::json!({
        "name": "web.search",
        "description": "Search the public web",
        "inputSchema": {
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"],
            "additionalProperties": false
        }
    }))
    .expect("MCP tool should deserialize");
    #[allow(deprecated)]
    {
        params.mcp_context = Some(MCPContext {
            resources: vec![],
            tools: vec![],
            servers: vec![MCPServer {
                id: "123e4567-e89b-12d3-a456-426614174000".to_string(),
                name: "Search server".to_string(),
                description: String::new(),
                resources: vec![],
                tools: vec![tool],
            }],
        });
    }
}

#[test]
fn chat_request_advertises_local_terminal_and_file_tools() {
    let params = params_with_custom_model();

    let request = build_chat_completion_request(&params, "provider-model".to_string())
        .expect("chat request should build");
    let request = serde_json::to_value(request).expect("chat request should serialize");
    let tool_names = request["tools"]
        .as_array()
        .expect("local tools should be present")
        .iter()
        .map(|tool| {
            tool["function"]["name"]
                .as_str()
                .expect("tool name should be a string")
        })
        .collect::<Vec<_>>();

    assert_eq!(
        tool_names,
        ["run_shell_command", "read_files", "apply_file_diffs"]
    );
    assert!(request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| tool["function"]["parameters"].is_object()));
}

#[test]
fn configured_mcp_tool_keeps_its_typed_schema_and_gets_a_stable_name() {
    let mut params = params_with_custom_model();
    add_search_mcp_tool(&mut params);

    let request = build_chat_completion_request(&params, "provider-model".to_string())
        .expect("MCP request should build");
    let request = serde_json::to_value(request).expect("chat request should serialize");
    let mcp_tool = request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["function"]["description"] == "Search the public web")
        .expect("configured MCP tool should be advertised");

    assert_eq!(
        mcp_tool["function"]["name"],
        "mcp_123e4567e89b12d3a456426614174000_web_search_75898d89"
    );
    assert_eq!(
        mcp_tool["function"]["parameters"]["required"],
        serde_json::json!(["query"])
    );
}

#[test]
fn malformed_mcp_arguments_cannot_become_an_executable_call() {
    let mut params = params_with_custom_model();
    add_search_mcp_tool(&mut params);
    let catalog = ToolCatalog::new(params.mcp_context.as_ref());
    let mut assembler = ToolCallAssembler::default();
    assembler
        .push(vec![ChatToolCallDelta {
            index: 0,
            id: Some("call-search".to_string()),
            function: Some(ChatFunctionCallDelta {
                name: Some("mcp_123e4567e89b12d3a456426614174000_web_search_75898d89".to_string()),
                arguments: Some("[]".to_string()),
            }),
        }])
        .unwrap();

    assert!(assembler.finish("task-1", "request-1", &catalog).is_err());
}

#[test]
fn configured_mcp_search_call_maps_back_to_the_exact_server_and_tool() {
    let mut params = params_with_custom_model();
    add_search_mcp_tool(&mut params);
    let (_tx, rx) = oneshot::channel();

    let mut output = futures::executor::block_on(generate_local_provider_output_with_transport(
        params,
        rx,
        Arc::new(MCPToolCallProviderTransport),
    ))
    .expect("stream should be created");
    let events = futures::executor::block_on(async {
        let mut events = Vec::new();
        while let Some(event) = output.next().await {
            events.push(event.expect("provider event should succeed"));
        }
        events
    });

    let call = events
        .iter()
        .filter_map(|event| match &event.r#type {
            Some(api::response_event::Type::ClientActions(actions)) => Some(actions),
            _ => None,
        })
        .flat_map(|actions| actions.actions.iter())
        .filter_map(|action| match &action.action {
            Some(api::client_action::Action::AddMessagesToTask(add)) => add.messages.first(),
            _ => None,
        })
        .find_map(|message| match &message.message {
            Some(api::message::Message::ToolCall(call)) => call.tool.as_ref(),
            _ => None,
        })
        .expect("MCP tool call should be emitted");

    assert!(matches!(
        call,
        api::message::tool_call::Tool::CallMcpTool(call)
            if call.server_id == "123e4567-e89b-12d3-a456-426614174000"
                && call.name == "web.search"
                && call.args.as_ref().and_then(|args| args.fields.get("query")).is_some_and(
                    |value| matches!(value.kind, Some(prost_types::value::Kind::StringValue(ref query)) if query == "rust async streams")
                )
    ));
}

#[test]
fn successful_mcp_search_result_continues_the_same_provider_conversation() {
    let mut params = params_with_custom_model();
    add_search_mcp_tool(&mut params);
    params.tasks[0].messages = vec![api::Message {
        id: "message-search".to_string(),
        task_id: "task-1".to_string(),
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: "call-search".to_string(),
            tool: Some(api::message::tool_call::Tool::CallMcpTool(
                api::message::tool_call::CallMcpTool {
                    name: "web.search".to_string(),
                    args: Some(prost_types::Struct {
                        fields: [(
                            "query".to_string(),
                            prost_types::Value {
                                kind: Some(prost_types::value::Kind::StringValue(
                                    "rust async streams".to_string(),
                                )),
                            },
                        )]
                        .into_iter()
                        .collect(),
                    }),
                    server_id: "123e4567-e89b-12d3-a456-426614174000".to_string(),
                },
            )),
        })),
        ..Default::default()
    }];
    params.input = vec![AIAgentInput::ActionResult {
        result: AIAgentActionResult {
            id: AIAgentActionId::from("call-search".to_string()),
            task_id: TaskId::new("task-1".to_string()),
            result: AIAgentActionResultType::CallMCPTool(CallMCPToolResult::Success {
                result: rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(
                    "Rust async streams documentation",
                )]),
            }),
        },
        context: Arc::from([]),
    }];

    let request = build_chat_completion_request(&params, "provider-model".to_string())
        .expect("MCP result request should build");
    let request = serde_json::to_value(request).expect("chat request should serialize");
    let messages = request["messages"].as_array().unwrap();

    assert_eq!(messages[0]["role"], "assistant");
    assert_eq!(
        messages[0]["tool_calls"][0]["function"]["name"],
        "mcp_123e4567e89b12d3a456426614174000_web_search_75898d89"
    );
    assert_eq!(messages[1]["role"], "tool");
    assert_eq!(messages[1]["tool_call_id"], "call-search");
    assert!(messages[1]["content"]
        .as_str()
        .is_some_and(|content| content.contains("Rust async streams documentation")));
}

#[test]
fn tool_result_continues_the_same_provider_conversation() {
    let mut params = params_with_custom_model();
    params.tasks[0].messages = vec![api::Message {
        id: "message-call".to_string(),
        task_id: "task-1".to_string(),
        message: Some(api::message::Message::ToolCall(api::message::ToolCall {
            tool_call_id: "call-shell".to_string(),
            tool: Some(api::message::tool_call::Tool::RunShellCommand(
                api::message::tool_call::RunShellCommand {
                    command: "pwd".to_string(),
                    is_read_only: false,
                    uses_pager: false,
                    citations: vec![],
                    is_risky: true,
                    wait_until_complete_value: None,
                    risk_category: 0,
                },
            )),
        })),
        ..Default::default()
    }];
    params.input = vec![AIAgentInput::ActionResult {
        result: AIAgentActionResult {
            id: AIAgentActionId::from("call-shell".to_string()),
            task_id: TaskId::new("task-1".to_string()),
            result: AIAgentActionResultType::RequestCommandOutput(
                RequestCommandOutputResult::CancelledBeforeExecution,
            ),
        },
        context: Arc::from([]),
    }];

    let request = build_chat_completion_request(&params, "provider-model".to_string())
        .expect("tool result request should build");
    let request = serde_json::to_value(request).expect("chat request should serialize");
    let messages = request["messages"].as_array().unwrap();

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "assistant");
    assert_eq!(messages[0]["tool_calls"][0]["id"], "call-shell");
    assert_eq!(
        messages[0]["tool_calls"][0]["function"]["name"],
        "run_shell_command"
    );
    assert_eq!(messages[1]["role"], "tool");
    assert_eq!(messages[1]["tool_call_id"], "call-shell");
    assert_eq!(messages[1]["content"], "Command output cancelled");
}

#[test]
fn fragmented_parallel_tool_calls_are_emitted_only_after_full_assembly() {
    let params = params_with_custom_model();
    let (_tx, rx) = oneshot::channel();

    let mut output = futures::executor::block_on(generate_local_provider_output_with_transport(
        params,
        rx,
        Arc::new(ToolCallProviderTransport),
    ))
    .expect("stream should be created");
    let events = futures::executor::block_on(async {
        let mut events = Vec::new();
        while let Some(event) = output.next().await {
            events.push(event.expect("provider event should succeed"));
        }
        events
    });

    let tool_calls = events
        .iter()
        .filter_map(|event| match &event.r#type {
            Some(api::response_event::Type::ClientActions(actions)) => Some(actions),
            _ => None,
        })
        .flat_map(|actions| actions.actions.iter())
        .filter_map(|action| match &action.action {
            Some(api::client_action::Action::AddMessagesToTask(add)) => Some(&add.messages),
            _ => None,
        })
        .flatten()
        .filter_map(|message| match &message.message {
            Some(api::message::Message::ToolCall(tool_call)) => Some(tool_call),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(tool_calls.len(), 2);
    assert_eq!(tool_calls[0].tool_call_id, "call-shell");
    assert!(matches!(
        tool_calls[0].tool,
        Some(api::message::tool_call::Tool::RunShellCommand(ref command))
            if command.command == "pwd"
    ));
    assert_eq!(tool_calls[1].tool_call_id, "call-read");
    assert!(matches!(
        tool_calls[1].tool,
        Some(api::message::tool_call::Tool::ReadFiles(ref read))
            if read.files.first().is_some_and(|file| file.name == "Cargo.toml")
    ));
}

#[test]
fn stopping_before_done_discards_not_yet_started_tool_calls() {
    let params = params_with_custom_model();
    let (tx, rx) = oneshot::channel();
    let mut output = futures::executor::block_on(generate_local_provider_output_with_transport(
        params,
        rx,
        Arc::new(InterruptedToolCallProviderTransport),
    ))
    .expect("stream should be created");

    let events = futures::executor::block_on(async {
        let init = output
            .next()
            .await
            .expect("init event should exist")
            .unwrap();
        tx.send(()).expect("cancellation should be delivered");
        let rest = output.collect::<Vec<_>>().await;
        (init, rest)
    });

    assert!(matches!(
        events.0.r#type,
        Some(api::response_event::Type::Init(_))
    ));
    assert!(events.1.is_empty());
}
