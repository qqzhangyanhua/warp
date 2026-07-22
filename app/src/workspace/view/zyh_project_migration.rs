use std::path::PathBuf;

use warpui::ViewContext;

use super::Workspace;
use crate::i18n::{tr, Message};
use crate::zyh_project_migration::modal::ProjectMigrationDialogEvent;
use crate::zyh_project_migration::{execute_project_migration, preview_project_migration};

impl Workspace {
    pub(super) fn handle_zyh_project_migration_dialog_event(
        &mut self,
        event: &ProjectMigrationDialogEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ProjectMigrationDialogEvent::Confirm(preview) => {
                let preview = preview.clone();
                let dialog = self.zyh_project_migration_dialog.clone();
                ctx.spawn(
                    async move {
                        tokio::task::spawn_blocking(move || execute_project_migration(preview))
                            .await
                    },
                    move |_, result, ctx| {
                        dialog.update(ctx, |dialog, ctx| match result {
                            Ok(result) => dialog.set_result(result, ctx),
                            Err(error) => dialog.set_error(error.to_string(), ctx),
                        });
                    },
                );
            }
            ProjectMigrationDialogEvent::Close => {
                self.current_workspace_state
                    .is_zyh_project_migration_dialog_open = false;
                self.focus_active_tab(ctx);
                ctx.notify();
            }
        }
    }

    pub(super) fn show_zyh_project_migration_dialog(&mut self, ctx: &mut ViewContext<Self>) {
        self.zyh_project_migration_request_id =
            self.zyh_project_migration_request_id.wrapping_add(1);
        let request_id = self.zyh_project_migration_request_id;
        self.current_workspace_state
            .is_zyh_project_migration_dialog_open = true;
        self.zyh_project_migration_dialog
            .update(ctx, |dialog, ctx| dialog.set_loading(ctx));
        ctx.focus(&self.zyh_project_migration_dialog);
        ctx.notify();

        let path = self
            .active_tab_pane_group()
            .as_ref(ctx)
            .focused_session_view(ctx)
            .and_then(|terminal| terminal.as_ref(ctx).pwd_if_local(ctx))
            .map(PathBuf::from);
        let Some(path) = path else {
            self.zyh_project_migration_dialog
                .update(ctx, |dialog, ctx| {
                    dialog.set_error(
                        tr(
                            ctx,
                            Message::WorkspaceProjectMigrationLocalRepositoryRequired,
                        )
                        .to_owned(),
                        ctx,
                    );
                });
            return;
        };

        let dialog = self.zyh_project_migration_dialog.clone();
        ctx.spawn(
            async move {
                tokio::task::spawn_blocking(move || preview_project_migration(&path)).await
            },
            move |workspace, result, ctx| {
                if workspace.zyh_project_migration_request_id != request_id {
                    return;
                }
                dialog.update(ctx, |dialog, ctx| match result {
                    Ok(Ok(preview)) => dialog.set_preview(preview, ctx),
                    Ok(Err(error)) => dialog.set_error(error.to_string(), ctx),
                    Err(error) => dialog.set_error(error.to_string(), ctx),
                });
            },
        );
    }
}
