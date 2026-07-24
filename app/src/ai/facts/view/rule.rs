use std::path::PathBuf;

use ai::project_context::model::{ProjectContextModel, ProjectContextModelEvent};
use markdown_parser::weight::CustomWeight;
use markdown_parser::{FormattedText, FormattedTextFragment, FormattedTextLine};
use warp_core::ui::appearance::{Appearance, AppearanceEvent};
use warp_core::ui::theme::color::internal_colors;
use warp_util::local_or_remote_path::LocalOrRemotePath;
use warpui::elements::{
    Align, Border, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    Expanded, Flex, FormattedTextElement, HighlightedHyperlink, Hoverable, MainAxisAlignment,
    MainAxisSize, MouseStateHandle, ParentElement, Shrinkable,
};
use warpui::platform::{Cursor, FilePickerConfiguration};
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{UiComponent, UiComponentStyles};
use warpui::{
    AppContext, Element, Entity, FocusContext, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use super::style;
use crate::ai::facts::{GlobalAgentRulesDocument, GlobalAgentRulesState};
use crate::editor::{
    EditorView, Event as EditorEvent, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions,
    TextOptions,
};
use crate::i18n::{Message, tr, tr_cached};
use crate::search_bar::SearchBar;
use crate::settings::{AISettings, AISettingsChangedEvent};
use crate::ui_components::icons::Icon;
use crate::util::path::display_path_with_host;
use crate::view_components::DismissibleToast;
use crate::view_components::action_button::{ActionButton, NakedTheme};
use crate::workspace::ToastStack;

pub const HEADER_TEXT: &str = "Rules";
fn description_text() -> &'static str {
    tr_cached(Message::RulesEnhanceAgent)
}

fn search_placeholder_text() -> &'static str {
    tr_cached(Message::SearchRules)
}

const ZERO_STATE_TEXT_GLOBAL: &str = "Create a Global Rule or drop a Markdown file at ~/.agents/AGENTS.md to apply it across every project.";
fn zero_state_text_project() -> &'static str {
    tr_cached(Message::RulesEmptyGenerateWarpMd)
}

fn disabled_banner_text() -> &'static str {
    tr_cached(Message::YourRulesAreDisabled)
}
fn disabled_banner_link_text() -> &'static str {
    tr_cached(Message::TurnItBackOn)
}
const DISABLED_BANNER_TEXT_2: &str = " anytime.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleScope {
    Global,
    ProjectBased,
}

#[derive(Debug, Clone)]
pub enum RuleViewEvent {
    /// Open the file-backed global rules editor (create or edit).
    EditGlobalRule,
    OpenSettings,
    OpenFile(LocalOrRemotePath),
    InitializeProject(PathBuf),
}

#[derive(Debug, Clone)]
pub enum RuleViewAction {
    EditGlobalRule,
    InitializeProject,
    OpenSettings,
    SelectScope(RuleScope),
    OpenFile(LocalOrRemotePath),
}

/// A rule row backed by a file on disk — project-scoped rules (e.g.
/// `<repo>/WARP.md`) and the single global `~/.agents/AGENTS.md`.
#[derive(Debug, Clone)]
struct FileBackedRow {
    file_path: LocalOrRemotePath,
    /// Optional content preview (global document only).
    preview: Option<String>,
    mouse_state: MouseStateHandle,
    /// When true, clicking the row opens the in-app editor rather than an external file open.
    opens_editor: bool,
}

impl FileBackedRow {
    fn matches_search_term(&self, search_term: &str) -> bool {
        let search_term = search_term.to_lowercase();
        self.file_path
            .display_path()
            .to_lowercase()
            .contains(&search_term)
            || self
                .preview
                .as_ref()
                .is_some_and(|preview| preview.to_lowercase().contains(&search_term))
    }
}

pub struct RuleView {
    /// Sole Global Rules source: the standard `~/.agents/AGENTS.md` document.
    global_rule: Option<FileBackedRow>,
    project_rules: Vec<FileBackedRow>,
    search_editor: ViewHandle<EditorView>,
    search_bar: ViewHandle<SearchBar>,
    add_button: ViewHandle<ActionButton>,
    initialize_button: ViewHandle<ActionButton>,
    disabled_banner_highlight_index: HighlightedHyperlink,
    current_scope: RuleScope,
    global_tab_mouse_state: MouseStateHandle,
    project_tab_mouse_state: MouseStateHandle,
}

impl RuleView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        ctx.subscribe_to_model(&AISettings::handle(ctx), |_, _, event, ctx| {
            if matches!(
                event,
                AISettingsChangedEvent::MemoryEnabled { .. }
                    | AISettingsChangedEvent::IsAnyAIEnabled { .. }
            ) {
                ctx.notify();
            }
        });

        let project_context = ProjectContextModel::handle(ctx);
        let project_rules = project_context
            .as_ref(ctx)
            .indexed_rules()
            .map(|p| FileBackedRow {
                file_path: p,
                preview: None,
                mouse_state: Default::default(),
                opens_editor: false,
            })
            .collect();

        ctx.subscribe_to_model(
            &project_context,
            |me, context_model, event, ctx| match event {
                ProjectContextModelEvent::PathIndexed => {
                    me.project_rules = context_model
                        .as_ref(ctx)
                        .indexed_rules()
                        .map(|p| FileBackedRow {
                            file_path: p,
                            preview: None,
                            mouse_state: Default::default(),
                            opens_editor: false,
                        })
                        .collect();
                    ctx.notify();
                }
                ProjectContextModelEvent::GlobalRulesChanged(_) => {
                    me.refresh_global_rule();
                    ctx.notify();
                }
                ProjectContextModelEvent::KnownRulesChanged(_) => {}
            },
        );

        let appearance = Appearance::handle(ctx);
        ctx.subscribe_to_model(&appearance, move |me, _, event, ctx| {
            if let AppearanceEvent::ThemeChanged = event {
                let appearance = Appearance::as_ref(ctx);
                let search_bar_styles = style::search_bar(appearance);
                me.search_bar.update(ctx, |search_bar, _| {
                    search_bar.with_style(search_bar_styles)
                });
            }
        });

        let search_editor_text = TextOptions::ui_text(None, appearance.as_ref(ctx));
        let search_editor = {
            let options = SingleLineEditorOptions {
                text: search_editor_text,
                propagate_and_no_op_vertical_navigation_keys:
                    PropagateAndNoOpNavigationKeys::Always,
                ..Default::default()
            };
            ctx.add_typed_action_view(|ctx| EditorView::single_line(options, ctx))
        };
        ctx.subscribe_to_view(&search_editor, move |me, _, event, ctx| {
            me.handle_search_editor_event(event, ctx);
        });

        search_editor.update(ctx, |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
            editor.set_placeholder_text(search_placeholder_text(), ctx);
        });
        let search_bar = ctx.add_typed_action_view(|_| SearchBar::new(search_editor.clone()));

        let add_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(tr(ctx, Message::McpAdd), NakedTheme)
                .with_icon(Icon::Plus)
                .on_click(|ctx| ctx.dispatch_typed_action(RuleViewAction::EditGlobalRule))
        });

        let initialize_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(tr(ctx, Message::AiInitializeProject), NakedTheme)
                .with_icon(Icon::Plus)
                .on_click(|ctx| ctx.dispatch_typed_action(RuleViewAction::InitializeProject))
        });

        let mut view = Self {
            global_rule: None,
            project_rules,
            search_editor,
            search_bar,
            add_button,
            initialize_button,
            disabled_banner_highlight_index: Default::default(),
            current_scope: RuleScope::Global,
            global_tab_mouse_state: Default::default(),
            project_tab_mouse_state: Default::default(),
        };
        view.refresh_global_rule();
        view
    }

    /// Refresh the Global tab from the on-disk document. Never reads cloud Rules.
    pub fn refresh_global_rule(&mut self) {
        let Ok(document) = GlobalAgentRulesDocument::standard() else {
            self.global_rule = None;
            return;
        };

        match document.load() {
            Ok(GlobalAgentRulesState::Present { content, .. }) => {
                self.global_rule = Some(FileBackedRow {
                    file_path: LocalOrRemotePath::Local(document.path().to_path_buf()),
                    preview: Some(truncate_preview(&content)),
                    mouse_state: Default::default(),
                    opens_editor: true,
                });
            }
            Ok(GlobalAgentRulesState::Missing) | Err(_) => {
                self.global_rule = None;
            }
        }
    }

    fn handle_search_editor_event(&mut self, _event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        ctx.notify();
    }

    fn select_scope(&mut self, scope: RuleScope, ctx: &mut ViewContext<Self>) {
        self.current_scope = scope;
        if scope == RuleScope::Global {
            self.refresh_global_rule();
        }
        ctx.notify();
    }

    fn filtered_global_rows(&self) -> Vec<FileBackedRow> {
        self.global_rule.iter().cloned().collect()
    }

    fn filtered_project_rows(&self) -> Vec<FileBackedRow> {
        self.project_rules.clone()
    }

    fn render_header(&self, appearance: &Appearance) -> Box<dyn Element> {
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(
                    ConstrainedBox::new(
                        warpui::elements::Icon::new(
                            Icon::BookOpen.into(),
                            appearance
                                .theme()
                                .main_text_color(appearance.theme().background()),
                        )
                        .finish(),
                    )
                    .with_width(style::ICON_SIZE)
                    .with_height(style::ICON_SIZE)
                    .finish(),
                )
                .with_margin_right(style::ICON_MARGIN)
                .finish(),
            )
            .with_child(
                appearance
                    .ui_builder()
                    .wrappable_text(HEADER_TEXT, true)
                    .with_style(style::header_text())
                    .build()
                    .finish(),
            )
            .finish()
    }

    fn render_description(&self, appearance: &Appearance) -> Box<dyn Element> {
        Container::new(
            appearance
                .ui_builder()
                .wrappable_text(description_text(), true)
                .with_style(style::description_text(appearance))
                .build()
                .finish(),
        )
        .with_vertical_margin(style::ITEM_BOTTOM_MARGIN)
        .finish()
    }

    fn render_scope_tabs(&self, appearance: &Appearance) -> Box<dyn Element> {
        let global_tab = Container::new(self.render_scope_tab(
            "Global",
            RuleScope::Global,
            appearance,
            self.global_tab_mouse_state.clone(),
        ))
        .with_padding_right(4.)
        .finish();
        let project_tab = self.render_scope_tab(
            "Project based",
            RuleScope::ProjectBased,
            appearance,
            self.project_tab_mouse_state.clone(),
        );

        Container::new(
            Flex::row()
                .with_child(global_tab)
                .with_child(project_tab)
                .finish(),
        )
        .with_margin_bottom(style::SECTION_MARGIN)
        .finish()
    }

    fn render_scope_tab(
        &self,
        title: &str,
        scope: RuleScope,
        appearance: &Appearance,
        mouse_state: MouseStateHandle,
    ) -> Box<dyn Element> {
        let is_selected = self.current_scope == scope;
        let text_color = if is_selected {
            appearance
                .theme()
                .main_text_color(appearance.theme().background())
        } else {
            appearance
                .theme()
                .sub_text_color(appearance.theme().background())
        };
        let title_owned = title.to_string();

        Hoverable::new(mouse_state, move |state| {
            let mut container = Container::new(
                appearance
                    .ui_builder()
                    .wrappable_text(title_owned.clone(), true)
                    .with_style(UiComponentStyles {
                        font_size: Some(style::TEXT_FONT_SIZE),
                        font_color: Some(text_color.into()),
                        ..Default::default()
                    })
                    .build()
                    .finish(),
            )
            .with_horizontal_padding(style::ROW_HORIZONTAL_PADDING)
            .with_vertical_padding(8.);

            if is_selected {
                container = container
                    .with_background(appearance.theme().surface_2())
                    .with_corner_radius(CornerRadius::with_all(warpui::elements::Radius::Pixels(
                        4.,
                    )));
            } else if state.is_hovered() {
                container = container
                    .with_background(appearance.theme().surface_1())
                    .with_corner_radius(CornerRadius::with_all(warpui::elements::Radius::Pixels(
                        4.,
                    )));
            }

            container.finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(RuleViewAction::SelectScope(scope));
        })
        .finish()
    }

    fn render_initialize_button(&self) -> Box<dyn Element> {
        Container::new(ChildView::new(&self.initialize_button).finish())
            .with_margin_left(style::SECTION_MARGIN)
            .finish()
    }

    fn render_add_global_button(&self) -> Box<dyn Element> {
        Container::new(ChildView::new(&self.add_button).finish())
            .with_margin_left(style::SECTION_MARGIN)
            .finish()
    }

    fn render_disabled_banner(&self, appearance: &Appearance) -> Box<dyn Element> {
        let mut link =
            FormattedTextFragment::hyperlink(disabled_banner_link_text(), "Settings > AI");
        link.styles.weight = Some(CustomWeight::Bold);

        let formatted_text = FormattedTextElement::new(
            FormattedText::new([FormattedTextLine::Line(vec![
                FormattedTextFragment::bold(disabled_banner_text()),
                link,
                FormattedTextFragment::bold(DISABLED_BANNER_TEXT_2),
            ])]),
            style::SUBTEXT_FONT_SIZE,
            appearance.ui_font_family(),
            appearance.ui_font_family(),
            appearance
                .theme()
                .sub_text_color(appearance.theme().background())
                .into(),
            self.disabled_banner_highlight_index.clone(),
        )
        .with_hyperlink_font_color(internal_colors::accent_fg_strong(appearance.theme()).into())
        .register_default_click_handlers(|_, ctx, _| {
            ctx.dispatch_typed_action(RuleViewAction::OpenSettings);
        });

        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    Container::new(
                        ConstrainedBox::new(
                            Icon::Info
                                .to_warpui_icon(
                                    appearance
                                        .theme()
                                        .sub_text_color(appearance.theme().background()),
                                )
                                .finish(),
                        )
                        .with_width(style::BANNER_ICON_SIZE)
                        .with_height(style::BANNER_ICON_SIZE)
                        .finish(),
                    )
                    .with_margin_right(style::ROW_ICON_MARGIN)
                    .finish(),
                )
                .with_child(Expanded::new(1., formatted_text.finish()).finish())
                .finish(),
        )
        .with_background(appearance.theme().accent_overlay())
        .with_corner_radius(CornerRadius::with_all(warpui::elements::Radius::Pixels(4.)))
        .with_uniform_padding(style::BANNER_PADDING)
        .with_margin_bottom(style::ITEM_BOTTOM_MARGIN)
        .finish()
    }

    fn render_search_bar_row(&self, show_add: bool) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Expanded::new(1., ChildView::new(&self.search_bar).finish()).finish());

        if show_add {
            match self.current_scope {
                RuleScope::Global => row.add_child(self.render_add_global_button()),
                RuleScope::ProjectBased => row.add_child(self.render_initialize_button()),
            }
        }
        Container::new(row.finish())
            .with_margin_bottom(style::SECTION_MARGIN)
            .finish()
    }

    fn render_file_backed_row(
        &self,
        row: FileBackedRow,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let row_name = display_path_with_host(&row.file_path, false, app);
        let mut content = Flex::column().with_child(
            Shrinkable::new(
                1.,
                appearance
                    .ui_builder()
                    .wrappable_text(row_name, true)
                    .with_style(style::fact_project_based_row_text(appearance))
                    .build()
                    .finish(),
            )
            .finish(),
        );

        if let Some(preview) = &row.preview {
            content.add_child(
                appearance
                    .ui_builder()
                    .wrappable_text(preview.clone(), true)
                    .with_style(style::fact_row_subtext(appearance))
                    .build()
                    .finish(),
            );
        }

        let mut row_flex = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Expanded::new(1., content.finish()).finish());

        if row.opens_editor {
            row_flex.add_child(
                appearance
                    .ui_builder()
                    .button(ButtonVariant::Outlined, row.mouse_state.clone())
                    .with_text_label("Edit".to_string())
                    .build()
                    .on_click(move |ctx, _, _| {
                        ctx.dispatch_typed_action(RuleViewAction::EditGlobalRule);
                    })
                    .finish(),
            );
        } else {
            let file_path = row.file_path.clone();
            row_flex.add_child(
                appearance
                    .ui_builder()
                    .button(ButtonVariant::Outlined, row.mouse_state.clone())
                    .with_text_label(tr_cached(Message::CodeOpenFile).to_string())
                    .build()
                    .on_click(move |ctx, _, _| {
                        ctx.dispatch_typed_action(RuleViewAction::OpenFile(file_path.clone()));
                    })
                    .finish(),
            );
        }

        Container::new(row_flex.finish())
            .with_background(internal_colors::neutral_1(appearance.theme()))
            .with_corner_radius(CornerRadius::with_all(warpui::elements::Radius::Pixels(4.)))
            .with_border(
                Border::all(1.).with_border_color(internal_colors::neutral_2(appearance.theme())),
            )
            .with_horizontal_padding(style::ROW_HORIZONTAL_PADDING)
            .with_vertical_padding(style::RULE_VERTICAL_PADDING)
            .with_margin_bottom(style::ITEM_BOTTOM_MARGIN)
            .finish()
    }

    fn render_items(
        &self,
        appearance: &Appearance,
        mut rows: Vec<FileBackedRow>,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let mut col = Flex::column();

        let search_term = self.search_editor.as_ref(app).buffer_text(app);
        if !search_term.is_empty() {
            rows.retain(|row| row.matches_search_term(search_term.as_str()));
        }
        rows.sort_by(|a, b| a.file_path.display_path().cmp(&b.file_path.display_path()));

        for row in rows {
            col.add_child(self.render_file_backed_row(row, appearance, app));
        }
        col.finish()
    }

    fn render_zero_state(&self, appearance: &Appearance) -> Box<dyn Element> {
        let text = match self.current_scope {
            RuleScope::Global => ZERO_STATE_TEXT_GLOBAL,
            RuleScope::ProjectBased => zero_state_text_project(),
        };

        let centered_text = appearance
            .ui_builder()
            .wrappable_text(text, true)
            .with_style(style::description_text(appearance))
            .build()
            .finish();

        let mut zero_state = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(Align::new(centered_text).top_center().finish())
                    .with_horizontal_padding(style::ROW_HORIZONTAL_PADDING)
                    .finish(),
            );
        match self.current_scope {
            RuleScope::Global => zero_state.add_child(self.render_add_global_button()),
            RuleScope::ProjectBased => zero_state.add_child(self.render_initialize_button()),
        }

        Container::new(
            ConstrainedBox::new(Align::new(zero_state.finish()).finish())
                .with_height(style::ZERO_STATE_HEIGHT)
                .finish(),
        )
        .with_horizontal_padding(style::ROW_HORIZONTAL_PADDING)
        .with_border(
            Border::all(1.).with_border_color(internal_colors::neutral_2(appearance.theme())),
        )
        .with_margin_bottom(style::SECTION_MARGIN)
        .finish()
    }

    fn render_body(
        &self,
        appearance: &Appearance,
        rows: Vec<FileBackedRow>,
        app: &AppContext,
    ) -> Box<dyn Element> {
        // Global has at most one document — show Add only when missing (zero-state handles that).
        // Project tab keeps the initialize control when rows exist.
        let show_add = self.current_scope == RuleScope::ProjectBased && !rows.is_empty();
        Flex::column()
            .with_child(self.render_search_bar_row(show_add))
            .with_child(self.render_items(appearance, rows, app))
            .finish()
    }
}

fn truncate_preview(content: &str) -> String {
    if content.split('\n').count() > 3 {
        content.split('\n').take(3).collect::<Vec<_>>().join("\n") + "..."
    } else {
        content.to_string()
    }
}

impl Entity for RuleView {
    type Event = RuleViewEvent;
}

impl View for RuleView {
    fn ui_name() -> &'static str {
        "RuleView"
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            ctx.focus(&self.search_editor);
        }
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let mut col = Flex::column()
            .with_child(self.render_header(appearance))
            .with_child(self.render_description(appearance));

        col.add_child(self.render_scope_tabs(appearance));

        let ai_settings = AISettings::as_ref(app);
        if !ai_settings.is_memory_enabled(app) {
            col.add_child(self.render_disabled_banner(appearance));
        }

        let rows = match self.current_scope {
            RuleScope::Global => self.filtered_global_rows(),
            RuleScope::ProjectBased => self.filtered_project_rows(),
        };
        if rows.is_empty() {
            col.add_child(self.render_zero_state(appearance));
        } else {
            col.add_child(self.render_body(appearance, rows, app));
        };
        col.finish()
    }
}

impl TypedActionView for RuleView {
    type Action = RuleViewAction;

    fn handle_action(&mut self, action: &RuleViewAction, ctx: &mut ViewContext<Self>) {
        match action {
            RuleViewAction::EditGlobalRule => {
                ctx.emit(RuleViewEvent::EditGlobalRule);
            }
            RuleViewAction::OpenSettings => {
                ctx.emit(RuleViewEvent::OpenSettings);
            }
            RuleViewAction::SelectScope(scope) => {
                self.select_scope(*scope, ctx);
            }
            RuleViewAction::OpenFile(path) => {
                ctx.emit(RuleViewEvent::OpenFile(path.clone()));
            }
            RuleViewAction::InitializeProject => {
                let file_picker_config = FilePickerConfiguration::new().folders_only();
                let window_id = ctx.window_id();

                ctx.open_file_picker(
                    move |result, ctx| match result {
                        Ok(paths) => {
                            if let Some(directory_path) = paths.first() {
                                let path = PathBuf::from(directory_path);
                                ctx.emit(RuleViewEvent::InitializeProject(path));
                            }
                        }
                        Err(err) => {
                            ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                                toast_stack.add_ephemeral_toast(
                                    DismissibleToast::error(format!("{err}")),
                                    window_id,
                                    ctx,
                                );
                            });
                        }
                    },
                    file_picker_config,
                );
            }
        }
    }
}
