use ai::agent::convert::ConvertToAPITypeError;
use async_channel::{Receiver, Sender};
use futures::channel::oneshot;
use futures::future::BoxFuture;
use prost::Message as _;
use warp_multi_agent_api as api;
use warpui::{AppContext, Entity, ModelContext, ModelHandle};

use super::{RuntimeToolActionAdapter, ToolEffectOutcome, ToolPermissionDecision};
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::agent::runtime::transcript::{
    RuntimeContentBlock, ToolErrorCode, ToolResultProjection,
};
use crate::ai::agent::{AIAgentAction, AIAgentActionResult, MarkdownActionResult};
use crate::ai::blocklist::{BlocklistAIActionModel, RuntimeToolExecutionError};

const MAX_PROJECTION_TEXT_BYTES: usize = 64 * 1024;

pub(crate) struct BlocklistRuntimeToolActionAdapter {
    commands: Sender<RuntimeToolCommand>,
    _dispatcher: ModelHandle<BlocklistRuntimeToolDispatcher>,
}

impl BlocklistRuntimeToolActionAdapter {
    pub(crate) fn new(
        action_model: ModelHandle<BlocklistAIActionModel>,
        conversation_id: AIConversationId,
        ctx: &mut AppContext,
    ) -> Self {
        let (commands, receiver) = async_channel::bounded(8);
        let dispatcher = ctx.add_model(|ctx| {
            BlocklistRuntimeToolDispatcher::new(action_model, conversation_id, receiver, ctx)
        });
        Self {
            commands,
            _dispatcher: dispatcher,
        }
    }

    #[cfg(test)]
    pub(super) fn new_for_test(
        action_model: ModelHandle<BlocklistAIActionModel>,
        conversation_id: AIConversationId,
        app: &mut warpui::App,
    ) -> Self {
        let (commands, receiver) = async_channel::bounded(8);
        let dispatcher = app.add_model(|ctx| {
            BlocklistRuntimeToolDispatcher::new(action_model, conversation_id, receiver, ctx)
        });
        Self {
            commands,
            _dispatcher: dispatcher,
        }
    }
}

impl RuntimeToolActionAdapter for BlocklistRuntimeToolActionAdapter {
    fn cancel_run(&self, run_id: String) -> BoxFuture<'static, ()> {
        let commands = self.commands.clone();
        Box::pin(async move {
            let (acknowledgement, acknowledged) = oneshot::channel();
            if commands
                .send(RuntimeToolCommand::CancelRun {
                    run_id,
                    acknowledgement,
                })
                .await
                .is_ok()
            {
                let _ = acknowledged.await;
            }
        })
    }

    fn request_permission(
        &self,
        run_id: String,
        action: AIAgentAction,
    ) -> BoxFuture<'static, ToolPermissionDecision> {
        let commands = self.commands.clone();
        Box::pin(async move {
            let (response, result) = oneshot::channel();
            if commands
                .send(RuntimeToolCommand::RequestPermission {
                    run_id,
                    action,
                    response,
                })
                .await
                .is_err()
            {
                return ToolPermissionDecision::DeniedByPolicy;
            }
            result
                .await
                .unwrap_or(ToolPermissionDecision::DeniedByPolicy)
        })
    }

    fn execute(
        &self,
        run_id: String,
        action: AIAgentAction,
    ) -> BoxFuture<'static, ToolEffectOutcome> {
        let commands = self.commands.clone();
        Box::pin(async move {
            let (response, result) = oneshot::channel();
            if commands
                .send(RuntimeToolCommand::Execute {
                    run_id,
                    action,
                    response,
                })
                .await
                .is_err()
            {
                return failed_outcome("The ZYH tool executor is unavailable.", false);
            }
            match result.await {
                Ok(Ok(result)) => effect_outcome(result),
                Ok(Err(RuntimeToolExecutionError::ExecutorUnavailable)) => {
                    failed_outcome("The ZYH tool executor could not start the action.", false)
                }
                Err(_) => failed_outcome("The ZYH tool executor stopped unexpectedly.", true),
            }
        })
    }
}

enum RuntimeToolCommand {
    CancelRun {
        run_id: String,
        acknowledgement: oneshot::Sender<()>,
    },
    RequestPermission {
        run_id: String,
        action: AIAgentAction,
        response: oneshot::Sender<ToolPermissionDecision>,
    },
    Execute {
        run_id: String,
        action: AIAgentAction,
        response: oneshot::Sender<Result<AIAgentActionResult, RuntimeToolExecutionError>>,
    },
}

struct BlocklistRuntimeToolDispatcher {
    action_model: ModelHandle<BlocklistAIActionModel>,
    conversation_id: AIConversationId,
}

impl BlocklistRuntimeToolDispatcher {
    fn new(
        action_model: ModelHandle<BlocklistAIActionModel>,
        conversation_id: AIConversationId,
        commands: Receiver<RuntimeToolCommand>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        ctx.spawn_stream_local(
            commands,
            |me, command, ctx| me.handle(command, ctx),
            |_, _| {},
        );
        Self {
            action_model,
            conversation_id,
        }
    }

    fn handle(&mut self, command: RuntimeToolCommand, ctx: &mut ModelContext<Self>) {
        match command {
            RuntimeToolCommand::CancelRun {
                run_id,
                acknowledgement,
            } => {
                self.action_model.update(ctx, |model, ctx| {
                    model.cancel_runtime_tool_run(&self.conversation_id, &run_id, ctx);
                });
                let _ = acknowledgement.send(());
            }
            RuntimeToolCommand::RequestPermission {
                run_id,
                action,
                response,
            } => {
                self.action_model.update(ctx, |model, ctx| {
                    model.request_runtime_tool_permission(
                        action,
                        self.conversation_id,
                        run_id,
                        response,
                        ctx,
                    );
                });
            }
            RuntimeToolCommand::Execute {
                run_id,
                action,
                response,
            } => {
                self.action_model.update(ctx, |model, ctx| {
                    model.execute_runtime_tool_action(
                        action,
                        self.conversation_id,
                        run_id,
                        response,
                        ctx,
                    );
                });
            }
        }
    }
}

impl Entity for BlocklistRuntimeToolDispatcher {
    type Event = ();
}

pub(super) fn effect_outcome(result: AIAgentActionResult) -> ToolEffectOutcome {
    use api::request::input::user_inputs::user_input::Input as UserInput;

    let tool_call_id = result.id.clone().into();
    let text = MarkdownActionResult(&result.result).to_string();
    let succeeded = result.result.is_successful();
    let result = match result.try_into() {
        Ok(UserInput::ToolCallResult(result)) => result.result.and_then(message_result),
        Ok(
            UserInput::UserQuery(_)
            | UserInput::CliAgentUserQuery(_)
            | UserInput::MessagesReceivedFromAgents(_)
            | UserInput::EventsFromAgents(_)
            | UserInput::PassiveSuggestionResult(_)
            | UserInput::OrchestrationConfigUpdate(_)
            | UserInput::ConversationHandoff(_),
        )
        | Err(
            ConvertToAPITypeError::Ignore
            | ConvertToAPITypeError::Unimplemented(_)
            | ConvertToAPITypeError::Other(_),
        ) => None,
    };
    let complete_outcome = api::message::ToolCallResult {
        tool_call_id,
        context: None,
        result: result.clone(),
    }
    .encode_to_vec();
    let (text, truncated) = truncate_text(text);
    let content = vec![RuntimeContentBlock::Text { text }];
    let projection = if succeeded {
        ToolResultProjection::Success { content, truncated }
    } else {
        ToolResultProjection::Error {
            error_code: ToolErrorCode::ToolExecutionFailed,
            may_have_executed: true,
            content,
            truncated,
        }
    };
    ToolEffectOutcome {
        complete_outcome,
        result,
        projection,
    }
}

#[allow(deprecated)]
fn message_result(
    result: api::request::input::tool_call_result::Result,
) -> Option<api::message::tool_call_result::Result> {
    use api::message::tool_call_result::Result as MessageResult;
    use api::request::input::tool_call_result::Result as InputResult;

    match result {
        InputResult::RunShellCommand(result) => Some(MessageResult::RunShellCommand(result)),
        InputResult::ReadFiles(result) => Some(MessageResult::ReadFiles(result)),
        InputResult::ApplyFileDiffs(result) => Some(MessageResult::ApplyFileDiffs(result)),
        InputResult::CallMcpTool(result) => Some(MessageResult::CallMcpTool(result)),
        InputResult::SearchCodebase(_)
        | InputResult::SuggestPlan(_)
        | InputResult::SuggestCreatePlan(_)
        | InputResult::Grep(_)
        | InputResult::FileGlob(_)
        | InputResult::ReadMcpResource(_)
        | InputResult::WriteToLongRunningShellCommand(_)
        | InputResult::SuggestNewConversation(_)
        | InputResult::FileGlobV2(_)
        | InputResult::SuggestPrompt(_)
        | InputResult::OpenCodeReview(_)
        | InputResult::InitProject(_)
        | InputResult::ReadDocuments(_)
        | InputResult::EditDocuments(_)
        | InputResult::CreateDocuments(_)
        | InputResult::ReadShellCommandOutput(_)
        | InputResult::UseComputer(_)
        | InputResult::InsertReviewComments(_)
        | InputResult::ReadSkill(_)
        | InputResult::RequestComputerUse(_)
        | InputResult::FetchConversation(_)
        | InputResult::StartAgent(_)
        | InputResult::SendMessageToAgent(_)
        | InputResult::TransferShellCommandControlToUser(_)
        | InputResult::AskUserQuestion(_)
        | InputResult::StartAgentV2(_)
        | InputResult::UploadFileArtifact(_)
        | InputResult::RunAgentsResult(_)
        | InputResult::WaitForEvents(_)
        | InputResult::StartRecording(_)
        | InputResult::StopRecording(_) => None,
    }
}

fn failed_outcome(message: &str, may_have_executed: bool) -> ToolEffectOutcome {
    ToolEffectOutcome {
        complete_outcome: Vec::new(),
        result: None,
        projection: ToolResultProjection::Error {
            error_code: ToolErrorCode::ToolExecutionFailed,
            may_have_executed,
            content: vec![RuntimeContentBlock::Text {
                text: message.to_string(),
            }],
            truncated: false,
        },
    }
}

fn truncate_text(mut text: String) -> (String, bool) {
    if text.len() <= MAX_PROJECTION_TEXT_BYTES {
        return (text, false);
    }
    let mut end = MAX_PROJECTION_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    (text, true)
}
