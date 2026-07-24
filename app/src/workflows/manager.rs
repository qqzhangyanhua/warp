use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;

use warpui::{Entity, EntityId, ModelContext, SingletonEntity};

use super::workflow::Workflow;
use super::WorkflowSource;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::Owner;
use crate::drive::OpenWarpDriveObjectSettings;
use crate::pane_group::pane::PaneContent;
use crate::pane_group::WorkflowPane;
use crate::server::ids::SyncId;
use crate::workflows::workflow_view::WorkflowView;
use crate::workflows::WorkflowViewMode;
use crate::{safe_warn, PaneViewLocator, WindowId};

pub struct WorkflowManager {
    panes_by_hashed_id: HashMap<String, WorkflowPaneData>,
}

#[derive(Debug, Clone)]
pub enum WorkflowOpenSource {
    Existing(SyncId),
    /// Open an existing local YAML Workflow file for view/edit.
    LocalFile {
        path: PathBuf,
        source: WorkflowSource,
    },
    /// Create a new local YAML Workflow under the given directory.
    NewLocal {
        title: Option<String>,
        content: Option<String>,
        directory: PathBuf,
        source: WorkflowSource,
        is_for_agent_mode: bool,
    },
    New {
        title: Option<String>,

        /// The "content" of the workflow.
        /// For `Command` workflows, this is the command.
        /// For `AgentMode` workflows, this is the AI query.
        content: Option<String>,

        owner: Owner,
        initial_folder_id: Option<SyncId>,
        is_for_agent_mode: bool,
    },
    NewFromWorkflow {
        workflow: Box<Workflow>,
        owner: Owner,
        initial_folder_id: Option<SyncId>,
    },
}

impl WorkflowManager {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        WorkflowManager {
            panes_by_hashed_id: HashMap::new(),
        }
    }

    pub fn find_pane(&self, source: &WorkflowOpenSource) -> Option<(WindowId, PaneViewLocator)> {
        match source {
            WorkflowOpenSource::Existing(workflow_id) => {
                let pane_data = self.panes_by_hashed_id.get(&workflow_id.uid())?;
                Some((pane_data.window_id, pane_data.locator))
            }
            WorkflowOpenSource::LocalFile { path, .. } => {
                let key = local_file_pane_key(path);
                let pane_data = self.panes_by_hashed_id.get(&key)?;
                Some((pane_data.window_id, pane_data.locator))
            }
            WorkflowOpenSource::New { .. }
            | WorkflowOpenSource::NewLocal { .. }
            | WorkflowOpenSource::NewFromWorkflow { .. } => None,
        }
    }

    pub fn create_pane(
        &mut self,
        source: &WorkflowOpenSource,
        settings: &OpenWarpDriveObjectSettings,
        mode: WorkflowViewMode,
        window_id: WindowId,
        ctx: &mut ModelContext<Self>,
    ) -> WorkflowPane {
        let view = ctx.add_typed_action_view(window_id, WorkflowView::new_in_pane);

        match source {
            WorkflowOpenSource::Existing(workflow_id) => {
                let workflow = CloudModel::as_ref(ctx).get_workflow(workflow_id).cloned();
                if let Some(workflow) = workflow {
                    view.update(ctx, |view, ctx| view.load(workflow, settings, mode, ctx));
                } else {
                    // If the workflow doesn't exist, try waiting for initial load and trying again
                    view.update(ctx, |view, ctx| {
                        view.wait_for_initial_load_then_load(
                            *workflow_id,
                            settings,
                            mode,
                            window_id,
                            ctx,
                        )
                    });
                }
            }
            WorkflowOpenSource::LocalFile { path, source } => {
                view.update(ctx, |view, ctx| {
                    view.open_local_workflow(path.clone(), *source, mode, ctx);
                });
            }
            WorkflowOpenSource::NewLocal {
                title,
                content,
                directory,
                source,
                is_for_agent_mode,
            } => view.update(ctx, |view, ctx| {
                view.open_new_local_workflow(
                    title.clone(),
                    content.clone(),
                    directory.clone(),
                    *source,
                    *is_for_agent_mode,
                    ctx,
                )
            }),
            WorkflowOpenSource::New {
                title,
                content,
                owner,
                initial_folder_id,
                is_for_agent_mode,
            } => view.update(ctx, |view, ctx| {
                // Retained product path: create as local user YAML instead of cloud ownership.
                view.open_new_local_workflow(
                    title.clone(),
                    content.clone(),
                    crate::user_config::workflows_dir(),
                    WorkflowSource::Local,
                    *is_for_agent_mode,
                    ctx,
                );
                let _ = owner;
                let _ = initial_folder_id;
            }),
            WorkflowOpenSource::NewFromWorkflow {
                workflow,
                owner,
                initial_folder_id,
            } => {
                // Prefer local YAML creation from a template workflow.
                view.update(ctx, |view, ctx| {
                    view.open_new_local_workflow(
                        Some(workflow.name().to_string()),
                        Some(workflow.content().to_string()),
                        crate::user_config::workflows_dir(),
                        WorkflowSource::Local,
                        workflow.is_agent_mode_workflow(),
                        ctx,
                    );
                    let _ = owner;
                    let _ = initial_folder_id;
                });
            }
        }

        WorkflowPane::new(view, ctx)
    }

    pub fn register_pane(
        &mut self,
        pane: &WorkflowPane,
        pane_group_id: EntityId,
        window_id: WindowId,
        ctx: &mut ModelContext<Self>,
    ) {
        let view = pane.get_view(ctx);
        let key = view
            .as_ref(ctx)
            .local_file_path()
            .map(local_file_pane_key)
            .unwrap_or_else(|| view.as_ref(ctx).workflow_id().uid());
        let workflow_id = view.as_ref(ctx).workflow_id();
        let entry = self.panes_by_hashed_id.entry(key);
        if let Entry::Vacant(entry) = entry {
            entry.insert(WorkflowPaneData {
                workflow_id,
                window_id,
                locator: PaneViewLocator {
                    pane_group_id,
                    pane_id: pane.id(),
                },
            });
        } else {
            safe_warn!(
                safe: ("Ignoring duplicate Workflow pane registration"),
                full: ("Ignoring duplicate Workflow pane registration for {workflow_id}")
            );
        }
    }

    pub fn deregister_pane(&mut self, pane: &WorkflowPane, ctx: &mut ModelContext<Self>) {
        let view = pane.get_view(ctx);
        let key = view
            .as_ref(ctx)
            .local_file_path()
            .map(local_file_pane_key)
            .unwrap_or_else(|| view.as_ref(ctx).workflow_id().uid());
        let workflow_id = view.as_ref(ctx).workflow_id();

        // If a workflow pane is restored, the workflow may have been reopened in the meantime. In
        // that case, don't let closing the original pane clear out the new pane.
        if let Entry::Occupied(entry) = self.panes_by_hashed_id.entry(key) {
            if entry.get().locator.pane_id == pane.id() {
                entry.remove();
            } else {
                log::warn!(
                    "Ignoring duplicate registration of panes for {}",
                    workflow_id.uid()
                );
            }
        }
    }

    pub fn reset(&mut self) {
        self.panes_by_hashed_id.clear();
    }
}

struct WorkflowPaneData {
    workflow_id: SyncId,
    window_id: WindowId,
    locator: PaneViewLocator,
}

fn local_file_pane_key(path: &std::path::Path) -> String {
    format!("local-yaml:{}", path.display())
}

impl Entity for WorkflowManager {
    type Event = ();
}

impl SingletonEntity for WorkflowManager {}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
