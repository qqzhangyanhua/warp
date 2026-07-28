use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use anyhow::{Context, Result};
use futures::channel::oneshot::{self, Receiver};
use warp_errors::{report_error, report_if_error};
use warpui::{Entity, ModelContext, RequestState, SingletonEntity};

use super::user_workspaces::{
    UserWorkspaces, WorkspacesMetadataResponse, WorkspacesMetadataWithPricing,
};
use super::workspace::WorkspaceUid;
use crate::ai::llms::LLMPreferences;
use crate::auth::AuthStateProvider;
use crate::persistence::ModelEvent;
use crate::server::retry_strategies::OUT_OF_BAND_REQUEST_RETRY_STRATEGY;
use crate::server::server_api::team::TeamClient;
use crate::server::server_api::ServerApiProvider;

/// Performs explicit workspace-metadata refreshes and persists the selected workspace.
pub struct TeamUpdateManager {
    team_client: Arc<dyn TeamClient>,
    model_event_sender: Option<SyncSender<ModelEvent>>,
}

impl TeamUpdateManager {
    #[cfg(test)]
    pub fn new(
        team_client: Arc<dyn TeamClient>,
        model_event_sender: Option<SyncSender<ModelEvent>>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self {
            team_client,
            model_event_sender,
        }
    }

    #[cfg(test)]
    pub fn mock(ctx: &mut ModelContext<Self>) -> Self {
        use crate::server::server_api::team::MockTeamClient;

        // Stub explicit metadata refreshes in tests that do not care which teams the user is on.
        let mut team_client = MockTeamClient::new();
        team_client.expect_workspaces_metadata().returning(|| {
            Ok(WorkspacesMetadataWithPricing {
                metadata: WorkspacesMetadataResponse {
                    workspaces: vec![],
                    joinable_teams: vec![],
                    experiments: None,
                    feature_model_choices: None,
                },
                pricing_info: None,
            })
        });

        Self::new(Arc::new(team_client), Default::default(), ctx)
    }

    /// Out-of-band (from the regular poll) refresh of workspace metadata.
    /// Returns a oneshot Receiver that resolves when the refresh completes (success or final failure).
    pub fn refresh_workspace_metadata(&mut self, ctx: &mut ModelContext<Self>) -> Receiver<()> {
        // Skip the refresh when logged out to avoid noisy auth errors.
        if !AuthStateProvider::as_ref(ctx).get().is_logged_in() {
            let (tx, rx) = oneshot::channel::<()>();
            let _ = tx.send(());
            return rx;
        }

        let team_client = self.team_client.clone();
        let (tx, rx) = oneshot::channel::<()>();
        let mut tx = Some(tx);
        ctx.spawn_with_retry_on_error(
            move || {
                let team_client = team_client.clone();
                async move { team_client.workspaces_metadata().await }
            },
            OUT_OF_BAND_REQUEST_RETRY_STRATEGY,
            move |update_manager, request_state, ctx| {
                // Only signal once there are no more retries left.
                let is_final = !request_state.has_pending_retries();
                update_manager.handle_workspace_metadata_with_request_state(request_state, ctx);
                if is_final {
                    if let Some(sender) = tx.take() {
                        let _ = sender.send(());
                    }
                }
            },
        );
        rx
    }

    fn save_to_db(&self, events: impl IntoIterator<Item = ModelEvent>) {
        let model_event_sender = self.model_event_sender.clone();
        if let Some(model_event_sender) = &model_event_sender {
            for event in events {
                report_if_error!(model_event_sender
                    .send(event)
                    .context("Unable to save teams metadata to sqlite"));
            }
        }
    }

    fn handle_workspace_metadata_with_request_state(
        &mut self,
        request_state: RequestState<WorkspacesMetadataWithPricing>,
        ctx: &mut ModelContext<Self>,
    ) {
        match request_state {
            RequestState::RequestSucceeded(response) => {
                // Server pricing catalogs are unused in ZYH (no buy-credits / overage UI).
                let _ = response.pricing_info;

                // Right now, this function is coupled with how we handle leaving a team.
                // TODO(zheng) refactor so we can separate these two cases and have clearer logic.
                self.on_workspaces_updated(Ok(response.metadata), ctx);
            }
            RequestState::RequestFailedRetryPending(err) => {
                log::info!(
                    "get_workspaces_metadata_for_user: request failed with error {err:#}. Trying again."
                );
            }
            RequestState::RequestFailed(err) => {
                log::info!("get_workspaces_metadata_for_user: request failed with error {err:#}. Retries exhausted.");
            }
        }
    }

    fn on_workspaces_updated(
        &mut self,
        result: Result<WorkspacesMetadataResponse>,
        ctx: &mut ModelContext<Self>,
    ) {
        match result {
            Ok(user_workspaces_access) => {
                let workspaces = user_workspaces_access.workspaces;
                let joinable_teams = user_workspaces_access.joinable_teams;
                let experiments = user_workspaces_access.experiments;

                UserWorkspaces::handle(ctx).update(ctx, |user_workspaces, ctx| {
                    user_workspaces.update_workspaces(workspaces.clone(), ctx);
                    user_workspaces.update_joinable_teams(joinable_teams.clone(), ctx);
                });

                // Check if the current workspace is still in the list of workspaces.
                // If it's not, then set the current workspace to the first workspace in the list.
                if let Some(current_workspace) = UserWorkspaces::as_ref(ctx).current_workspace() {
                    if !workspaces.iter().any(|w| w.uid == current_workspace.uid) {
                        if let Some(workspace_uid) = workspaces.first().map(|w| w.uid) {
                            self.set_current_workspace_uid(workspace_uid, ctx);
                        };
                    }
                } else if let Some(workspace_uid) = workspaces.first().map(|w| w.uid) {
                    self.set_current_workspace_uid(workspace_uid, ctx);
                }

                if let Some(experiments) = experiments {
                    ServerApiProvider::handle(ctx).update(ctx, |provider, ctx| {
                        provider.handle_experiments_fetched(experiments, ctx);
                    });
                }

                if let Some(feature_model_choices) = user_workspaces_access.feature_model_choices {
                    LLMPreferences::handle(ctx).update(ctx, |llm_preferences, ctx| {
                        llm_preferences
                            .update_feature_model_choices(feature_model_choices.try_into(), ctx);
                    });
                }

                // Update sqlite
                self.save_to_db([ModelEvent::UpsertWorkspaces { workspaces }]);
            }
            Err(e) => {
                report_error!(e);
            }
        }
    }

    pub fn set_current_workspace_uid(
        &mut self,
        workspace_uid: WorkspaceUid,
        ctx: &mut ModelContext<Self>,
    ) {
        UserWorkspaces::handle(ctx).update(ctx, |user_workspaces, ctx| {
            user_workspaces.set_current_workspace_uid(workspace_uid, ctx);
        });

        // Update sqlite
        self.save_to_db([ModelEvent::SetCurrentWorkspace { workspace_uid }]);
    }
}

impl Entity for TeamUpdateManager {
    type Event = ();
}

impl SingletonEntity for TeamUpdateManager {}
