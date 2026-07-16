use warpui::{EntityId, ModelContext};

use super::conversation_loader::convert_persisted_conversation_to_ai_conversation;
use super::BlocklistAIHistoryModel;
use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::blocklist::{BlocklistAIHistoryEvent, ResponseStreamId};
#[cfg(feature = "local_fs")]
use crate::persistence::agent::read_agent_conversation_by_id;
use crate::persistence::model::{AgentConversationData, AgentRuntimeBinding};

impl BlocklistAIHistoryModel {
    pub(crate) fn commit_runtime_text_run_progress(
        &mut self,
        conversation_id: AIConversationId,
        response_stream_id: &ResponseStreamId,
        terminal_surface_id: EntityId,
        mut conversation_data: AgentConversationData,
        tasks: Vec<warp_multi_agent_api::Task>,
        revision: u64,
        ctx: &mut ModelContext<Self>,
    ) {
        conversation_data.runtime_binding = Some(AgentRuntimeBinding::Pi);
        conversation_data.runtime_transcript_revision = Some(revision);
        let Some(conversation) = self.conversations_by_id.get_mut(&conversation_id) else {
            log::warn!("Failed to find Pi runtime conversation {conversation_id:?}");
            return;
        };
        if let Err(error) = conversation.apply_runtime_progress_snapshot(
            response_stream_id,
            tasks,
            conversation_data,
        ) {
            log::warn!("Failed to merge Pi runtime progress for {conversation_id:?}: {error:#}");
            return;
        }
        conversation.update_status(ConversationStatus::InProgress, terminal_surface_id, ctx);
        for exchange in conversation.all_exchanges() {
            ctx.emit(BlocklistAIHistoryEvent::UpdatedStreamingExchange {
                exchange_id: exchange.id,
                terminal_surface_id,
                conversation_id,
                is_hidden: conversation.is_exchange_hidden(exchange.id),
            });
        }
    }

    pub(crate) fn reload_runtime_conversation_from_db(
        &mut self,
        conversation_id: AIConversationId,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        #[cfg(feature = "local_fs")]
        let restored = self.db_connection.clone().and_then(|conn| {
            let mut conn = conn.lock().ok()?;
            read_agent_conversation_by_id(&mut conn, &conversation_id.to_string())
                .ok()
                .flatten()
                .and_then(convert_persisted_conversation_to_ai_conversation)
        });
        #[cfg(not(feature = "local_fs"))]
        let restored: Option<crate::ai::agent::conversation::AIConversation> = None;

        if let Some(conversation) = restored {
            self.conversations_by_id
                .insert(conversation_id, conversation);
            ctx.notify();
            true
        } else {
            log::warn!(
                "Failed to reload Conversation Record {conversation_id} after runtime history mutation failure"
            );
            false
        }
    }
}
