use ai::project_context::model::{ProjectContextModel, ProjectContextModelEvent};
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::color::internal_colors;
use warp_editor::editor::NavigationKey;
use warpui::elements::{
    Border, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, Flex, MainAxisAlignment, MainAxisSize, MouseStateHandle,
    ParentElement, Radius, ScrollbarWidth,
};
use warpui::platform::Cursor;
use warpui::ui_components::components::UiComponent;
use warpui::{
    AppContext, Element, Entity, FocusContext, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};
use warpui_extras::owner_only_file::{ContentHash, ExpectedContent};

use super::style;
use crate::ai::facts::{GlobalAgentRulesDocument, GlobalAgentRulesError, GlobalAgentRulesState};
use crate::editor::{
    EditorOptions, EditorView, EnterAction, EnterSettings, Event as EditorEvent,
    PropagateAndNoOpNavigationKeys, TextOptions,
};
use crate::i18n::{tr, Message};
use crate::ui_components::buttons::icon_button;
use crate::ui_components::icons::Icon;
use crate::view_components::action_button::{ActionButton, DangerSecondaryTheme, PrimaryTheme};
use crate::view_components::DismissibleToast;
use crate::workspace::ToastStack;

const RULE_CONTENT_PLACEHOLDER_TEXT: &str =
    "e.g. Prefer 4-space indentation and always run tests before committing.";
const CONFLICT_BANNER_TEXT: &str =
    "This file changed on disk. Reload the latest content, or save again after reloading.";
const DELETE_CONFIRM_PROMPT: &str = "Delete ~/.agents/AGENTS.md? This cannot be undone.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveStatus {
    Clean,
    Dirty,
    Conflict,
    Error,
}

#[derive(Debug, Clone)]
pub enum RuleEditorViewEvent {
    Back,
    Saved,
    Deleted,
}

#[derive(Debug, Clone)]
pub enum RuleEditorViewAction {
    Back,
    Save,
    Delete,
    ConfirmDelete,
    CancelDelete,
    Reload,
}

pub struct RuleEditorView {
    document: GlobalAgentRulesDocument,
    /// Hash of the content last loaded from disk (or written by us).
    /// `None` means the file was missing when the editor opened.
    loaded_hash: Option<ContentHash>,
    file_exists: bool,
    save_status: SaveStatus,
    pending_delete_confirmation: bool,
    content_editor: ViewHandle<EditorView>,
    save_button: ViewHandle<ActionButton>,
    delete_button: ViewHandle<ActionButton>,
    confirm_delete_button: ViewHandle<ActionButton>,
    cancel_delete_button: ViewHandle<ActionButton>,
    reload_button: ViewHandle<ActionButton>,
    back_button: MouseStateHandle,
    clipped_scroll_state: ClippedScrollStateHandle,
}

impl RuleEditorView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        // External edits to ~/.agents/AGENTS.md refresh a clean editor, or mark
        // a dirty editor as conflicted so a stale save cannot overwrite disk.
        ctx.subscribe_to_model(
            &ProjectContextModel::handle(ctx),
            |me, _model, event, ctx| {
                if matches!(event, ProjectContextModelEvent::GlobalRulesChanged(_)) {
                    me.on_external_document_change(ctx);
                }
            },
        );

        let appearance = Appearance::as_ref(ctx);
        let font_family = appearance.ui_font_family();
        let text = TextOptions {
            font_size_override: Some(style::TEXT_FONT_SIZE),
            font_family_override: Some(font_family),
            ..Default::default()
        };

        let content_editor = ctx.add_typed_action_view(|ctx| {
            let mut editor = EditorView::new(
                EditorOptions {
                    text,
                    soft_wrap: true,
                    autogrow: true,
                    propagate_and_no_op_vertical_navigation_keys:
                        PropagateAndNoOpNavigationKeys::Always,
                    supports_vim_mode: false,
                    single_line: false,
                    enter_settings: EnterSettings {
                        shift_enter: EnterAction::InsertNewLineIfMultiLine,
                        enter: EnterAction::InsertNewLineIfMultiLine,
                        alt_enter: EnterAction::InsertNewLineIfMultiLine,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ctx,
            );
            editor.set_placeholder_text(RULE_CONTENT_PLACEHOLDER_TEXT, ctx);
            editor
        });
        ctx.subscribe_to_view(&content_editor, |me, _editor, event, ctx| {
            me.handle_editor_event(event, ctx);
        });

        let save_button = ctx.add_typed_action_view(|ctx| {
            let mut button = ActionButton::new(tr(ctx, Message::SettingsSave), PrimaryTheme)
                .with_icon(Icon::Check)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(RuleEditorViewAction::Save);
                });
            button.set_disabled(true, ctx);
            button
        });

        let delete_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(tr(ctx, Message::AiDeleteRule), DangerSecondaryTheme)
                .with_icon(Icon::Trash)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(RuleEditorViewAction::Delete);
                })
        });

        let confirm_delete_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Confirm delete", DangerSecondaryTheme)
                .with_icon(Icon::Trash)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(RuleEditorViewAction::ConfirmDelete);
                })
        });

        let cancel_delete_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(tr(ctx, Message::CommonCancel), DangerSecondaryTheme).on_click(
                |ctx| {
                    ctx.dispatch_typed_action(RuleEditorViewAction::CancelDelete);
                },
            )
        });

        let reload_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("Reload", PrimaryTheme)
                .with_icon(Icon::Refresh)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(RuleEditorViewAction::Reload);
                })
        });

        let document = GlobalAgentRulesDocument::standard().unwrap_or_else(|_| {
            // Degenerate environments without a home dir still construct a view;
            // load/save will surface HomeNotFound.
            GlobalAgentRulesDocument::with_path(GlobalAgentRulesDocument::standard_path_for_home(
                "/",
            ))
        });

        let mut view = Self {
            document,
            loaded_hash: None,
            file_exists: false,
            save_status: SaveStatus::Clean,
            pending_delete_confirmation: false,
            content_editor,
            save_button,
            delete_button,
            confirm_delete_button,
            cancel_delete_button,
            reload_button,
            back_button: Default::default(),
            clipped_scroll_state: Default::default(),
        };
        view.reload_from_disk(ctx);
        view
    }

    /// Open the editor against the standard global rules file.
    pub fn open_global_rule(&mut self, ctx: &mut ViewContext<Self>) {
        self.pending_delete_confirmation = false;
        self.reload_from_disk(ctx);
        ctx.notify();
    }

    fn on_external_document_change(&mut self, ctx: &mut ViewContext<Self>) {
        match self.save_status {
            SaveStatus::Dirty | SaveStatus::Conflict | SaveStatus::Error => {
                // Keep the user's buffer; block save until they reload.
                if self.save_status != SaveStatus::Conflict {
                    self.save_status = SaveStatus::Conflict;
                    self.update_save_button(ctx);
                    ctx.notify();
                }
            }
            SaveStatus::Clean => {
                self.reload_from_disk(ctx);
                ctx.notify();
            }
        }
    }

    fn reload_from_disk(&mut self, ctx: &mut ViewContext<Self>) {
        match self.document.load() {
            Ok(GlobalAgentRulesState::Missing) => {
                self.loaded_hash = None;
                self.file_exists = false;
                self.save_status = SaveStatus::Clean;
                self.content_editor.update(ctx, |editor, ctx| {
                    editor.clear_buffer_and_reset_undo_stack(ctx);
                });
            }
            Ok(GlobalAgentRulesState::Present {
                content,
                content_hash,
            }) => {
                self.loaded_hash = Some(content_hash);
                self.file_exists = true;
                self.save_status = SaveStatus::Clean;
                self.content_editor.update(ctx, |editor, ctx| {
                    editor.set_buffer_text(content.as_str(), ctx);
                });
            }
            Err(error) => {
                self.save_status = SaveStatus::Error;
                self.show_error_toast(&error, ctx);
            }
        }
        self.update_save_button(ctx);
    }

    fn handle_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        match event {
            EditorEvent::Navigate(NavigationKey::Up) => {
                self.content_editor.update(ctx, |editor, ctx| {
                    editor.move_up(ctx);
                });
            }
            EditorEvent::Navigate(NavigationKey::Down) => {
                self.content_editor.update(ctx, |editor, ctx| {
                    editor.move_down(ctx);
                });
            }
            EditorEvent::Edited(_) => {
                if self.save_status != SaveStatus::Conflict {
                    self.save_status = SaveStatus::Dirty;
                }
                self.update_save_button(ctx);
                ctx.notify();
            }
            _ => {}
        }
    }

    fn update_save_button(&mut self, ctx: &mut ViewContext<Self>) {
        let content_empty = self.content_editor.as_ref(ctx).buffer_text(ctx).is_empty();
        // Enable when dirty. Creating a brand-new file requires non-empty content.
        // Conflict blocks save until the user reloads.
        let is_disabled = match self.save_status {
            SaveStatus::Conflict | SaveStatus::Clean => true,
            SaveStatus::Error | SaveStatus::Dirty => {
                if self.file_exists {
                    false
                } else {
                    content_empty
                }
            }
        };
        self.save_button.update(ctx, |button, ctx| {
            button.set_disabled(is_disabled, ctx);
        });
    }

    fn expected_content(&self) -> ExpectedContent {
        match self.loaded_hash {
            Some(hash) => ExpectedContent::Hash(hash),
            None => ExpectedContent::Missing,
        }
    }

    fn save(&mut self, ctx: &mut ViewContext<Self>) {
        let content = self.content_editor.as_ref(ctx).buffer_text(ctx);
        let result = if self.file_exists {
            self.document.save(&content, self.expected_content())
        } else {
            self.document.create(&content)
        };

        match result {
            Ok(hash) => {
                self.loaded_hash = Some(hash);
                self.file_exists = true;
                self.save_status = SaveStatus::Clean;
                self.pending_delete_confirmation = false;
                self.update_save_button(ctx);
                ctx.emit(RuleEditorViewEvent::Saved);
                ctx.notify();
            }
            Err(GlobalAgentRulesError::Conflict { .. }) => {
                self.save_status = SaveStatus::Conflict;
                self.update_save_button(ctx);
                self.show_toast(
                    "Could not save: the file changed on disk. Reload and try again.",
                    true,
                    ctx,
                );
                ctx.notify();
            }
            Err(error) => {
                self.save_status = SaveStatus::Error;
                self.show_error_toast(&error, ctx);
                ctx.notify();
            }
        }
    }

    fn confirm_delete(&mut self, ctx: &mut ViewContext<Self>) {
        match self.document.delete(self.expected_content()) {
            Ok(()) => {
                self.loaded_hash = None;
                self.file_exists = false;
                self.save_status = SaveStatus::Clean;
                self.pending_delete_confirmation = false;
                self.content_editor.update(ctx, |editor, ctx| {
                    editor.clear_buffer_and_reset_undo_stack(ctx);
                });
                self.update_save_button(ctx);
                ctx.emit(RuleEditorViewEvent::Deleted);
                ctx.notify();
            }
            Err(GlobalAgentRulesError::Conflict { .. }) => {
                self.save_status = SaveStatus::Conflict;
                self.pending_delete_confirmation = false;
                self.show_toast(
                    "Could not delete: the file changed on disk. Reload and try again.",
                    true,
                    ctx,
                );
                ctx.notify();
            }
            Err(error) => {
                self.pending_delete_confirmation = false;
                self.show_error_toast(&error, ctx);
                ctx.notify();
            }
        }
    }

    fn show_error_toast(&self, error: &GlobalAgentRulesError, ctx: &mut ViewContext<Self>) {
        self.show_toast(&error.to_string(), true, ctx);
    }

    fn show_toast(&self, message: &str, is_error: bool, ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            let toast = if is_error {
                DismissibleToast::error(message.to_string())
            } else {
                DismissibleToast::success(message.to_string())
            };
            toast_stack.add_ephemeral_toast(toast, window_id, ctx);
        });
    }

    fn render_back_button(&self, appearance: &Appearance) -> Box<dyn Element> {
        let button = icon_button(appearance, Icon::ArrowLeft, false, self.back_button.clone());
        Container::new(
            button
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(RuleEditorViewAction::Back);
                })
                .with_cursor(Cursor::PointingHand)
                .finish(),
        )
        .with_margin_right(style::ICON_MARGIN)
        .finish()
    }

    fn render_save_button(&self, _appearance: &Appearance) -> Box<dyn Element> {
        Container::new(ChildView::new(&self.save_button).finish())
            .with_margin_left(style::SECTION_MARGIN)
            .finish()
    }

    fn render_header(&self, appearance: &Appearance) -> Box<dyn Element> {
        let title = if self.file_exists {
            "Edit Global Rule"
        } else {
            "Create Global Rule"
        };
        Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    Flex::row()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_child(self.render_back_button(appearance))
                        .with_child(
                            appearance
                                .ui_builder()
                                .wrappable_text(title, true)
                                .with_style(style::header_text())
                                .build()
                                .finish(),
                        )
                        .finish(),
                )
                .with_child(self.render_save_button(appearance))
                .finish(),
        )
        .with_margin_bottom(style::ITEM_BOTTOM_MARGIN)
        .finish()
    }

    fn render_path_label(&self, appearance: &Appearance) -> Box<dyn Element> {
        Container::new(
            appearance
                .ui_builder()
                .wrappable_text(self.document.display_path(), true)
                .with_style(style::description_text(appearance))
                .build()
                .finish(),
        )
        .with_margin_bottom(style::ITEM_BOTTOM_MARGIN)
        .finish()
    }

    fn render_conflict_banner(&self, appearance: &Appearance) -> Box<dyn Element> {
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_child(
                    appearance
                        .ui_builder()
                        .wrappable_text(CONFLICT_BANNER_TEXT, true)
                        .with_style(style::description_text(appearance))
                        .build()
                        .finish(),
                )
                .with_child(ChildView::new(&self.reload_button).finish())
                .finish(),
        )
        .with_background(appearance.theme().accent_overlay())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .with_uniform_padding(style::BANNER_PADDING)
        .with_margin_bottom(style::ITEM_BOTTOM_MARGIN)
        .finish()
    }

    fn render_content_editor(&self, appearance: &Appearance) -> Box<dyn Element> {
        ConstrainedBox::new(
            Container::new(
                ClippedScrollable::vertical(
                    self.clipped_scroll_state.clone(),
                    ConstrainedBox::new(ChildView::new(&self.content_editor).finish())
                        .with_min_height(style::EDITOR_MIN_HEIGHT)
                        .finish(),
                    ScrollbarWidth::Auto,
                    appearance.theme().nonactive_ui_detail().into(),
                    appearance.theme().active_ui_detail().into(),
                    warpui::elements::Fill::None,
                )
                .finish(),
            )
            .with_background(appearance.theme().surface_2())
            .with_border(
                Border::all(1.).with_border_color(internal_colors::neutral_4(appearance.theme())),
            )
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
            .with_margin_bottom(style::ITEM_BOTTOM_MARGIN)
            .with_padding_left(style::EDITOR_HORIZONTAL_PADDING)
            .with_vertical_padding(style::EDITOR_VERTICAL_PADDING)
            .finish(),
        )
        .with_max_height(style::EDITOR_MAX_HEIGHT)
        .finish()
    }

    fn render_form(&self, appearance: &Appearance) -> Box<dyn Element> {
        Flex::column()
            .with_child(self.render_path_label(appearance))
            .with_child(
                Container::new(appearance.ui_builder().span("Rule").build().finish())
                    .with_margin_bottom(style::ITEM_BOTTOM_MARGIN)
                    .finish(),
            )
            .with_child(self.render_content_editor(appearance))
            .finish()
    }

    fn render_delete_section(&self, appearance: &Appearance) -> Box<dyn Element> {
        if self.pending_delete_confirmation {
            Flex::column()
                .with_child(
                    Container::new(
                        appearance
                            .ui_builder()
                            .wrappable_text(DELETE_CONFIRM_PROMPT, true)
                            .with_style(style::description_text(appearance))
                            .build()
                            .finish(),
                    )
                    .with_margin_bottom(style::ITEM_BOTTOM_MARGIN)
                    .finish(),
                )
                .with_child(
                    Flex::row()
                        .with_child(ChildView::new(&self.confirm_delete_button).finish())
                        .with_child(
                            Container::new(ChildView::new(&self.cancel_delete_button).finish())
                                .with_margin_left(style::SECTION_MARGIN)
                                .finish(),
                        )
                        .finish(),
                )
                .finish()
        } else {
            ChildView::new(&self.delete_button).finish()
        }
    }
}

impl Entity for RuleEditorView {
    type Event = RuleEditorViewEvent;
}

impl View for RuleEditorView {
    fn ui_name() -> &'static str {
        "RuleEditorView"
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            ctx.focus(&self.content_editor);
        }
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let mut col = Flex::column()
            .with_child(self.render_header(appearance))
            .with_child(self.render_form(appearance));

        if self.save_status == SaveStatus::Conflict {
            col.add_child(self.render_conflict_banner(appearance));
        }

        if self.file_exists {
            col.add_child(self.render_delete_section(appearance));
        }
        col.finish()
    }
}

impl TypedActionView for RuleEditorView {
    type Action = RuleEditorViewAction;

    fn handle_action(&mut self, action: &RuleEditorViewAction, ctx: &mut ViewContext<Self>) {
        match action {
            RuleEditorViewAction::Back => {
                ctx.emit(RuleEditorViewEvent::Back);
            }
            RuleEditorViewAction::Save => {
                self.save(ctx);
            }
            RuleEditorViewAction::Delete => {
                self.pending_delete_confirmation = true;
                ctx.notify();
            }
            RuleEditorViewAction::ConfirmDelete => {
                self.confirm_delete(ctx);
            }
            RuleEditorViewAction::CancelDelete => {
                self.pending_delete_confirmation = false;
                ctx.notify();
            }
            RuleEditorViewAction::Reload => {
                self.pending_delete_confirmation = false;
                self.reload_from_disk(ctx);
                ctx.notify();
            }
        }
    }
}
