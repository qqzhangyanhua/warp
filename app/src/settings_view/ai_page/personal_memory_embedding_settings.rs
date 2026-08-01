use ai::api_keys::{ApiKeyManager, EmbeddingProviderConfig};
use warpui::elements::{Container, CrossAxisAlignment, Flex, ParentElement, Text};
use warpui::ui_components::components::UiComponent;
use warpui::{AppContext, Element, SingletonEntity, ViewContext, ViewHandle};

use super::{AISettingsPageAction, AISettingsPageView, CONTENT_FONT_SIZE};
use crate::ai::personal_memory::{
    EmbeddingConnectionTestResult, EmbeddingProvider, EmbeddingProviderError,
};
use crate::appearance::Appearance;
use crate::editor::{
    EditorView, Event as EditorEvent, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions,
    TextColors,
};
use crate::i18n::{tr, Message};
use crate::view_components::action_button::{ActionButton, ButtonSize, SecondaryTheme};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum EmbeddingConnectionState {
    #[default]
    Idle,
    Testing,
    Compatible,
    CompatibilityWarning,
    Failed(EmbeddingProviderError),
}

pub(super) struct PersonalMemoryEmbeddingControls {
    base_url: ViewHandle<EditorView>,
    model: ViewHandle<EditorView>,
    api_key: ViewHandle<EditorView>,
    test_button: ViewHandle<ActionButton>,
    connection_state: EmbeddingConnectionState,
}

impl PersonalMemoryEmbeddingControls {
    pub(super) fn new(ctx: &mut ViewContext<AISettingsPageView>) -> Self {
        let configured = ApiKeyManager::as_ref(ctx).keys().embedding_provider.clone();
        let base_url = create_editor(
            configured
                .as_ref()
                .map(|provider| provider.base_url.as_str())
                .unwrap_or_default(),
            "https://provider.example/v1",
            false,
            ctx,
        );
        let model = create_editor(
            configured
                .as_ref()
                .map(|provider| provider.model.as_str())
                .unwrap_or_default(),
            "text-embedding-model",
            false,
            ctx,
        );
        let api_key = create_editor(
            configured
                .as_ref()
                .map(|provider| provider.api_key.as_str())
                .unwrap_or_default(),
            tr(ctx, Message::PersonalMemoryApiKey),
            true,
            ctx,
        );
        for editor in [&base_url, &model, &api_key] {
            ctx.subscribe_to_view(editor, |view, _, event, ctx| {
                if matches!(event, EditorEvent::Blurred | EditorEvent::Enter) {
                    view.persist_personal_memory_embedding_provider(ctx);
                }
            });
        }
        let test_button = ctx.add_typed_action_view(|ctx| {
            ActionButton::new(
                tr(ctx, Message::PersonalMemoryTestConnection),
                SecondaryTheme,
            )
            .with_size(ButtonSize::Small)
            .on_click(|ctx| {
                ctx.dispatch_typed_action(
                    AISettingsPageAction::TestPersonalMemoryEmbeddingProvider,
                );
            })
        });
        Self {
            base_url,
            model,
            api_key,
            test_button,
            connection_state: EmbeddingConnectionState::Idle,
        }
    }

    fn config(&self, ctx: &AppContext) -> EmbeddingProviderConfig {
        EmbeddingProviderConfig {
            base_url: self
                .base_url
                .as_ref(ctx)
                .buffer_text(ctx)
                .trim()
                .to_string(),
            model: self.model.as_ref(ctx).buffer_text(ctx).trim().to_string(),
            api_key: self.api_key.as_ref(ctx).buffer_text(ctx),
        }
    }
}

fn create_editor(
    value: &str,
    placeholder: &str,
    is_password: bool,
    ctx: &mut ViewContext<AISettingsPageView>,
) -> ViewHandle<EditorView> {
    let value = value.to_string();
    let placeholder = placeholder.to_string();
    ctx.add_typed_action_view(move |ctx| {
        let appearance = Appearance::as_ref(ctx);
        let options = SingleLineEditorOptions {
            is_password,
            propagate_and_no_op_vertical_navigation_keys: PropagateAndNoOpNavigationKeys::Always,
            text: crate::editor::TextOptions {
                font_size_override: Some(appearance.ui_font_size()),
                font_family_override: Some(appearance.monospace_font_family()),
                text_colors_override: Some(TextColors {
                    default_color: appearance.theme().active_ui_text_color(),
                    disabled_color: appearance.theme().disabled_ui_text_color(),
                    hint_color: appearance.theme().disabled_ui_text_color(),
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut editor = EditorView::single_line(options, ctx);
        editor.set_placeholder_text(placeholder, ctx);
        editor.set_buffer_text(&value, ctx);
        editor
    })
}

impl AISettingsPageView {
    #[cfg(test)]
    pub(super) fn personal_memory_embedding_config_for_test(
        &self,
        ctx: &AppContext,
    ) -> EmbeddingProviderConfig {
        self.personal_memory_embedding_controls.config(ctx)
    }

    #[cfg(test)]
    pub(super) fn personal_memory_embedding_test_failed_for_test(&self) -> bool {
        matches!(
            self.personal_memory_embedding_controls.connection_state,
            EmbeddingConnectionState::Failed(_)
        )
    }

    fn persist_personal_memory_embedding_provider(&mut self, ctx: &mut ViewContext<Self>) {
        let config = self.personal_memory_embedding_controls.config(ctx);
        let provider =
            (!config.base_url.is_empty() || !config.model.is_empty() || !config.api_key.is_empty())
                .then_some(config);
        ApiKeyManager::handle(ctx).update(ctx, |manager, ctx| {
            manager.set_embedding_provider(provider, ctx);
        });
        self.personal_memory_embedding_controls.connection_state = EmbeddingConnectionState::Idle;
        ctx.notify();
    }

    pub(super) fn test_personal_memory_embedding_provider(&mut self, ctx: &mut ViewContext<Self>) {
        let config = self.personal_memory_embedding_controls.config(ctx);
        ApiKeyManager::handle(ctx).update(ctx, |manager, ctx| {
            manager.set_embedding_provider(Some(config.clone()), ctx);
        });
        let provider = match EmbeddingProvider::new(config) {
            Ok(provider) => provider,
            Err(error) => {
                let state = EmbeddingConnectionState::Failed(error);
                self.personal_memory_embedding_controls.connection_state = state;
                self.show_personal_memory_embedding_test_toast(state, ctx);
                ctx.notify();
                return;
            }
        };
        self.personal_memory_embedding_controls.connection_state =
            EmbeddingConnectionState::Testing;
        ctx.notify();
        ctx.spawn(
            async move { provider.test_connection().await },
            |view, result, ctx| {
                let state = match result {
                    Ok(EmbeddingConnectionTestResult::Compatible) => {
                        EmbeddingConnectionState::Compatible
                    }
                    Ok(EmbeddingConnectionTestResult::CompatibilityWarning) => {
                        EmbeddingConnectionState::CompatibilityWarning
                    }
                    Err(error) => EmbeddingConnectionState::Failed(error),
                };
                view.personal_memory_embedding_controls.connection_state = state;
                view.show_personal_memory_embedding_test_toast(state, ctx);
                ctx.notify();
            },
        );
    }

    fn show_personal_memory_embedding_test_toast(
        &self,
        state: EmbeddingConnectionState,
        ctx: &mut ViewContext<Self>,
    ) {
        let message = connection_state_message(state, false);
        let window_id = ctx.window_id();
        crate::ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            let text = tr(ctx, message).to_string();
            let toast = match state {
                EmbeddingConnectionState::Compatible => {
                    crate::view_components::DismissibleToast::success(text)
                }
                EmbeddingConnectionState::CompatibilityWarning => {
                    crate::view_components::DismissibleToast::default(text)
                }
                EmbeddingConnectionState::Failed(_) => {
                    crate::view_components::DismissibleToast::error(text)
                }
                EmbeddingConnectionState::Idle | EmbeddingConnectionState::Testing => return,
            };
            toast_stack.add_ephemeral_toast(toast, window_id, ctx);
        });
    }
}

pub(super) fn render_personal_memory_embedding_controls(
    controls: &PersonalMemoryEmbeddingControls,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let config = controls.config(app);
    let status = tr(
        app,
        connection_state_message(controls.connection_state, config.is_valid()),
    );
    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(field(
            tr(app, Message::PersonalMemoryBaseUrl),
            controls.base_url.clone(),
            appearance,
        ))
        .with_child(field(
            tr(app, Message::PersonalMemoryModel),
            controls.model.clone(),
            appearance,
        ))
        .with_child(field(
            tr(app, Message::PersonalMemoryApiKey),
            controls.api_key.clone(),
            appearance,
        ))
        .with_child(warpui::elements::ChildView::new(&controls.test_button).finish())
        .with_child(
            Container::new(status_text(status, appearance))
                .with_padding_top(8.)
                .finish(),
        );
    if config.uses_plaintext_transport() {
        column.add_child(
            Container::new(status_text(
                tr(app, Message::PersonalMemoryPlaintextWarning),
                appearance,
            ))
            .with_padding_top(8.)
            .finish(),
        );
    }
    Container::new(column.finish())
        .with_padding_bottom(16.)
        .finish()
}

fn connection_state_message(state: EmbeddingConnectionState, configured: bool) -> Message {
    match state {
        EmbeddingConnectionState::Idle if configured => Message::PersonalMemorySemanticConfigured,
        EmbeddingConnectionState::Idle => Message::PersonalMemorySemanticUnavailable,
        EmbeddingConnectionState::Testing => Message::PersonalMemoryTestingEmbeddingProvider,
        EmbeddingConnectionState::Compatible => Message::PersonalMemoryConnectionCompatible,
        EmbeddingConnectionState::CompatibilityWarning => {
            Message::PersonalMemoryConnectionCompatibilityWarning
        }
        EmbeddingConnectionState::Failed(error) => embedding_error_message(error),
    }
}

fn embedding_error_message(error: EmbeddingProviderError) -> Message {
    match error {
        EmbeddingProviderError::Authentication => Message::PersonalMemoryConnectionAuthFailed,
        EmbeddingProviderError::MissingModel => Message::PersonalMemoryConnectionModelMissing,
        EmbeddingProviderError::MalformedProtocol => {
            Message::PersonalMemoryConnectionMalformedProtocol
        }
        EmbeddingProviderError::Timeout => Message::PersonalMemoryConnectionTimedOut,
        EmbeddingProviderError::RateLimited => Message::PersonalMemoryConnectionRateLimited,
        EmbeddingProviderError::Server => Message::PersonalMemoryConnectionServerError,
        EmbeddingProviderError::Transport => Message::PersonalMemoryConnectionFailed,
    }
}

fn field(
    label: &'static str,
    editor: ViewHandle<EditorView>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    Flex::column()
        .with_child(
            Text::new(label, appearance.ui_font_family(), CONTENT_FONT_SIZE)
                .with_color(appearance.theme().active_ui_text_color().into())
                .finish(),
        )
        .with_child(
            Container::new(appearance.ui_builder().text_input(editor).build().finish())
                .with_padding_top(4.)
                .with_padding_bottom(8.)
                .finish(),
        )
        .finish()
}

fn status_text(text: &'static str, appearance: &Appearance) -> Box<dyn Element> {
    Text::new(text, appearance.ui_font_family(), CONTENT_FONT_SIZE)
        .with_color(appearance.theme().nonactive_ui_text_color().into())
        .finish()
}
