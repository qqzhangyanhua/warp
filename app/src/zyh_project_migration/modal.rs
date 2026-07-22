use pathfinder_geometry::vector::vec2f;
use warp_core::ui::theme::Fill;
use warpui::elements::{
    Align, ChildAnchor, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox,
    Container, CrossAxisAlignment, Element, Flex, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, ScrollbarWidth, Stack, Text,
};
use warpui::keymap::{FixedBinding, Keystroke};
use warpui::ui_components::components::{UiComponent, UiComponentStyles};
use warpui::{AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle};

use super::{MigrationPreview, MigrationResult, MigrationResultStatus, PreviewStatus};
use crate::appearance::Appearance;
use crate::i18n::{tr, Message};
use crate::ui_components::dialog::{dialog_styles, Dialog};
use crate::view_components::action_button::{
    ActionButton, KeystrokeSource, NakedTheme, PrimaryTheme,
};

const DIALOG_WIDTH: f32 = 680.;
const CONTENT_MAX_HEIGHT: f32 = 360.;

pub(crate) fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_fixed_bindings([
        FixedBinding::new(
            "escape",
            ProjectMigrationDialogAction::Cancel,
            id!(ProjectMigrationDialog::ui_name()),
        ),
        FixedBinding::new(
            "enter",
            ProjectMigrationDialogAction::Primary,
            id!(ProjectMigrationDialog::ui_name()),
        ),
    ]);
}

enum ProjectMigrationDialogState {
    Loading,
    Preview(MigrationPreview),
    Running,
    Result(MigrationResult),
    Error(String),
}

pub(crate) struct ProjectMigrationDialog {
    state: ProjectMigrationDialogState,
    cancel_button: ViewHandle<ActionButton>,
    migrate_button: ViewHandle<ActionButton>,
    close_button: ViewHandle<ActionButton>,
    scroll_state: ClippedScrollStateHandle,
}

impl ProjectMigrationDialog {
    pub(crate) fn new(ctx: &mut ViewContext<Self>) -> Self {
        let cancel_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(tr(ctx, Message::WorkspaceCancel), NakedTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(ProjectMigrationDialogAction::Cancel);
            })
        });
        let enter_keystroke = Keystroke::parse("enter").expect("valid enter keystroke");
        let migrate_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(
                tr(ctx, Message::WorkspaceProjectMigrationMigrate),
                PrimaryTheme,
            )
            .with_keybinding(KeystrokeSource::Fixed(enter_keystroke), ctx)
            .on_click(|ctx| {
                ctx.dispatch_typed_action(ProjectMigrationDialogAction::Primary);
            })
        });
        let close_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(tr(ctx, Message::WorkspaceClose), PrimaryTheme).on_click(|ctx| {
                ctx.dispatch_typed_action(ProjectMigrationDialogAction::Cancel);
            })
        });

        Self {
            state: ProjectMigrationDialogState::Loading,
            cancel_button,
            migrate_button,
            close_button,
            scroll_state: ClippedScrollStateHandle::default(),
        }
    }

    pub(crate) fn set_loading(&mut self, ctx: &mut ViewContext<Self>) {
        self.state = ProjectMigrationDialogState::Loading;
        self.reset_scroll(ctx);
    }

    pub(crate) fn set_preview(&mut self, preview: MigrationPreview, ctx: &mut ViewContext<Self>) {
        self.state = ProjectMigrationDialogState::Preview(preview);
        self.reset_scroll(ctx);
    }

    pub(crate) fn set_result(&mut self, result: MigrationResult, ctx: &mut ViewContext<Self>) {
        self.state = ProjectMigrationDialogState::Result(result);
        self.reset_scroll(ctx);
    }

    pub(crate) fn set_error(&mut self, error: String, ctx: &mut ViewContext<Self>) {
        self.state = ProjectMigrationDialogState::Error(error);
        self.reset_scroll(ctx);
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub(crate) fn is_preview_visible(&self) -> bool {
        matches!(self.state, ProjectMigrationDialogState::Preview(_))
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub(crate) fn is_result_visible(&self) -> bool {
        matches!(self.state, ProjectMigrationDialogState::Result(_))
    }

    fn reset_scroll(&mut self, ctx: &mut ViewContext<Self>) {
        self.scroll_state = ClippedScrollStateHandle::default();
        ctx.notify();
    }

    fn body_message(&self) -> Message {
        match self.state {
            ProjectMigrationDialogState::Loading => Message::WorkspaceProjectMigrationLoading,
            ProjectMigrationDialogState::Preview(_) => Message::WorkspaceProjectMigrationPreview,
            ProjectMigrationDialogState::Running => Message::WorkspaceProjectMigrationRunning,
            ProjectMigrationDialogState::Result(_) => Message::WorkspaceProjectMigrationResult,
            ProjectMigrationDialogState::Error(_) => Message::WorkspaceProjectMigrationError,
        }
    }

    fn rows(&self, app: &AppContext) -> Vec<String> {
        match &self.state {
            ProjectMigrationDialogState::Preview(preview) => preview
                .entries
                .iter()
                .map(|entry| {
                    let status = preview_status_text(app, &entry.status);
                    let destination = entry
                        .destination
                        .as_ref()
                        .map(|path| format!(" -> {}", path.display()))
                        .unwrap_or_default();
                    let omissions = omission_text(app, &entry.omissions);
                    format!(
                        "{}{} [{status}]{omissions}",
                        entry.source.display(),
                        destination
                    )
                })
                .collect(),
            ProjectMigrationDialogState::Result(result) => result
                .entries
                .iter()
                .map(|entry| {
                    let status = result_status_text(app, &entry.status);
                    let destination = entry
                        .destination
                        .as_ref()
                        .map(|path| format!(" -> {}", path.display()))
                        .unwrap_or_default();
                    let omissions = omission_text(app, &entry.omissions);
                    format!(
                        "{}{} [{status}]{omissions}",
                        entry.source.display(),
                        destination
                    )
                })
                .collect(),
            ProjectMigrationDialogState::Error(error) => vec![error.clone()],
            ProjectMigrationDialogState::Loading | ProjectMigrationDialogState::Running => {
                Vec::new()
            }
        }
    }

    fn render_rows(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let text_color = theme.main_text_color(theme.surface_1()).into_solid();
        let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        for row in self.rows(app) {
            column.add_child(
                Container::new(
                    Text::new(row, appearance.ui_font_family(), 13.)
                        .with_color(text_color)
                        .finish(),
                )
                .with_margin_bottom(8.)
                .finish(),
            );
        }

        let scrollable = ClippedScrollable::vertical(
            self.scroll_state.clone(),
            column.finish(),
            ScrollbarWidth::Auto,
            theme.nonactive_ui_text_color().into(),
            theme.active_ui_text_color().into(),
            warpui::elements::Fill::None,
        )
        .finish();
        ConstrainedBox::new(scrollable)
            .with_max_height(CONTENT_MAX_HEIGHT)
            .finish()
    }
}

impl Entity for ProjectMigrationDialog {
    type Event = ProjectMigrationDialogEvent;
}

impl View for ProjectMigrationDialog {
    fn ui_name() -> &'static str {
        "ProjectMigrationDialog"
    }

    fn on_focus(&mut self, _focus_ctx: &warpui::FocusContext, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let mut dialog = Dialog::new(
            tr(app, Message::WorkspaceProjectMigrationTitle).to_owned(),
            Some(tr(app, self.body_message()).to_owned()),
            UiComponentStyles {
                width: Some(DIALOG_WIDTH),
                ..dialog_styles(appearance)
            },
        )
        .with_child(self.render_rows(app));

        match self.state {
            ProjectMigrationDialogState::Preview(_) => {
                dialog = dialog
                    .with_bottom_row_child(
                        Container::new(ChildView::new(&self.cancel_button).finish())
                            .with_margin_right(12.)
                            .finish(),
                    )
                    .with_bottom_row_child(ChildView::new(&self.migrate_button).finish());
            }
            ProjectMigrationDialogState::Result(_) | ProjectMigrationDialogState::Error(_) => {
                dialog = dialog.with_bottom_row_child(ChildView::new(&self.close_button).finish());
            }
            ProjectMigrationDialogState::Loading | ProjectMigrationDialogState::Running => {}
        }

        let mut stack = Stack::new();
        stack.add_positioned_child(
            dialog.build().finish(),
            OffsetPositioning::offset_from_parent(
                vec2f(0., 0.),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::Center,
                ChildAnchor::Center,
            ),
        );
        Container::new(Align::new(stack.finish()).finish())
            .with_background_color(Fill::blur().into())
            .with_corner_radius(app.windows().window_corner_radius())
            .finish()
    }
}

pub(crate) enum ProjectMigrationDialogEvent {
    Confirm(MigrationPreview),
    Close,
}

#[derive(Debug)]
pub(crate) enum ProjectMigrationDialogAction {
    Primary,
    Cancel,
}

impl TypedActionView for ProjectMigrationDialog {
    type Action = ProjectMigrationDialogAction;

    fn handle_action(
        &mut self,
        action: &ProjectMigrationDialogAction,
        ctx: &mut ViewContext<Self>,
    ) {
        match action {
            ProjectMigrationDialogAction::Primary => {
                if let ProjectMigrationDialogState::Preview(preview) = &self.state {
                    let preview = preview.clone();
                    self.state = ProjectMigrationDialogState::Running;
                    ctx.emit(ProjectMigrationDialogEvent::Confirm(preview));
                    ctx.notify();
                }
            }
            ProjectMigrationDialogAction::Cancel => {
                if !matches!(self.state, ProjectMigrationDialogState::Running) {
                    ctx.emit(ProjectMigrationDialogEvent::Close);
                }
            }
        }
    }
}

fn omission_text(app: &AppContext, omissions: &[String]) -> String {
    if omissions.is_empty() {
        String::new()
    } else {
        format!(
            "\n  {}: {}",
            tr(app, Message::WorkspaceProjectMigrationOmitted),
            omissions.join(", ")
        )
    }
}

fn preview_status_text(app: &AppContext, status: &PreviewStatus) -> &'static str {
    let message = match status {
        PreviewStatus::Ready => Message::WorkspaceProjectMigrationReady,
        PreviewStatus::AlreadyPresent => Message::WorkspaceProjectMigrationAlreadyPresent,
        PreviewStatus::Conflict => Message::WorkspaceProjectMigrationConflict,
        PreviewStatus::SkippedSymlink => Message::WorkspaceProjectMigrationSkippedSymlink,
        PreviewStatus::Unsupported => Message::WorkspaceProjectMigrationUnsupported,
    };
    tr(app, message)
}

fn result_status_text(app: &AppContext, status: &MigrationResultStatus) -> String {
    let (message, detail) = match status {
        MigrationResultStatus::Copied => (Message::WorkspaceProjectMigrationCopied, None),
        MigrationResultStatus::AlreadyPresent => {
            (Message::WorkspaceProjectMigrationAlreadyPresent, None)
        }
        MigrationResultStatus::Conflict => (Message::WorkspaceProjectMigrationConflict, None),
        MigrationResultStatus::SkippedSymlink => {
            (Message::WorkspaceProjectMigrationSkippedSymlink, None)
        }
        MigrationResultStatus::Unsupported => (Message::WorkspaceProjectMigrationUnsupported, None),
        MigrationResultStatus::Stale => (Message::WorkspaceProjectMigrationStale, None),
        MigrationResultStatus::Failed(error) => (
            Message::WorkspaceProjectMigrationFailed,
            Some(error.as_str()),
        ),
    };
    match detail {
        Some(detail) => format!("{}: {detail}", tr(app, message)),
        None => tr(app, message).to_owned(),
    }
}
