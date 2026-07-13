#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConnectionTestFailure {
    Authentication,
    MissingModel,
    MalformedProtocol,
    Timeout,
    RateLimited,
    Server,
    Connection,
    Unexpected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConnectionTestState {
    Idle,
    Testing,
    Succeeded,
    Failed(ConnectionTestFailure),
}

pub(super) struct ConnectionTestController {
    state: ConnectionTestState,
    generation: u64,
    cancellation: Option<Box<dyn FnOnce()>>,
}

impl Default for ConnectionTestController {
    fn default() -> Self {
        Self {
            state: ConnectionTestState::Idle,
            generation: 0,
            cancellation: None,
        }
    }
}

impl ConnectionTestController {
    pub(super) fn state(&self) -> &ConnectionTestState {
        &self.state
    }

    pub(super) fn begin(&mut self) -> u64 {
        self.cancel_pending();
        self.generation = self.generation.wrapping_add(1);
        self.state = ConnectionTestState::Testing;
        self.generation
    }

    pub(super) fn set_cancellation(
        &mut self,
        generation: u64,
        cancellation: impl FnOnce() + 'static,
    ) {
        if generation == self.generation && self.state == ConnectionTestState::Testing {
            self.cancellation = Some(Box::new(cancellation));
        } else {
            cancellation();
        }
    }

    pub(super) fn complete(&mut self, generation: u64, result: Result<(), ConnectionTestFailure>) {
        if generation != self.generation || self.state != ConnectionTestState::Testing {
            return;
        }
        self.cancellation = None;
        self.state = match result {
            Ok(()) => ConnectionTestState::Succeeded,
            Err(error) => ConnectionTestState::Failed(error),
        };
    }

    pub(super) fn cancel(&mut self) {
        self.cancel_pending();
        self.generation = self.generation.wrapping_add(1);
        self.state = ConnectionTestState::Idle;
    }

    fn cancel_pending(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation();
        }
    }
}

impl ConnectionTestFailure {
    pub(super) fn from_api_error(error: AIApiError) -> Self {
        match error {
            AIApiError::ProviderErrorStatus { status, .. } => match status {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Self::Authentication,
                StatusCode::NOT_FOUND => Self::MissingModel,
                StatusCode::REQUEST_TIMEOUT => Self::Timeout,
                StatusCode::TOO_MANY_REQUESTS => Self::RateLimited,
                status if status.is_server_error() => Self::Server,
                _ => Self::Unexpected,
            },
            AIApiError::Transport(_) => Self::Connection,
            AIApiError::Deserialization(_) => Self::MalformedProtocol,
            AIApiError::Other(error) => {
                let message = error.to_string();
                if message.contains("timed out") {
                    Self::Timeout
                } else if message.contains("malformed") || message.contains("oversized") {
                    Self::MalformedProtocol
                } else if message.contains("Could not connect") {
                    Self::Connection
                } else {
                    Self::Unexpected
                }
            }
            _ => Self::Unexpected,
        }
    }
}

pub(super) fn render_connection_test_control(
    state: &ConnectionTestState,
    mouse_state: MouseStateHandle,
    is_valid: bool,
    button_style: UiComponentStyles,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let label = if *state == ConnectionTestState::Testing {
        tr(app, Message::CustomInferenceTestingConnection)
    } else {
        tr(app, Message::CustomInferenceTestConnection)
    };
    let mut button = appearance
        .ui_builder()
        .button(ButtonVariant::Secondary, mouse_state)
        .with_text_label(label.to_string())
        .with_style(button_style);
    if !is_valid {
        button = button.disabled();
    }
    let mut column = Flex::column().with_child(
        Container::new(
            button
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(&CustomEndpointModalAction::TestConnection);
                })
                .finish(),
        )
        .with_margin_bottom(8.)
        .finish(),
    );
    match state {
        ConnectionTestState::Succeeded => column.add_child(status_text(
            tr(app, Message::CustomInferenceConnectionSucceeded).to_string(),
            appearance.theme().ansi_fg_green(),
            app,
        )),
        ConnectionTestState::Failed(error) => column.add_child(status_text(
            tr(app, failure_message(*error)).to_string(),
            appearance.theme().ui_error_color(),
            app,
        )),
        ConnectionTestState::Idle | ConnectionTestState::Testing => {}
    }
    column.finish()
}

fn status_text(message: String, color: ColorU, app: &AppContext) -> Box<dyn Element> {
    Container::new(
        Text::new(message, Appearance::as_ref(app).ui_font_family(), 12.)
            .with_color(color)
            .soft_wrap(true)
            .finish(),
    )
    .with_margin_bottom(16.)
    .finish()
}

fn failure_message(error: ConnectionTestFailure) -> Message {
    match error {
        ConnectionTestFailure::Authentication => Message::CustomInferenceConnectionAuthFailed,
        ConnectionTestFailure::MissingModel => Message::CustomInferenceConnectionModelMissing,
        ConnectionTestFailure::MalformedProtocol => {
            Message::CustomInferenceConnectionMalformedProtocol
        }
        ConnectionTestFailure::Timeout => Message::CustomInferenceConnectionTimedOut,
        ConnectionTestFailure::RateLimited => Message::CustomInferenceConnectionRateLimited,
        ConnectionTestFailure::Server => Message::CustomInferenceConnectionServerError,
        ConnectionTestFailure::Connection => Message::CustomInferenceConnectionFailed,
        ConnectionTestFailure::Unexpected => Message::CustomInferenceConnectionUnexpectedError,
    }
}

#[cfg(test)]
#[path = "custom_inference_connection_test_tests.rs"]
mod tests;
use http::StatusCode;
use pathfinder_color::ColorU;
use warpui::elements::{Container, Flex, MouseStateHandle, ParentElement, Text};
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{UiComponent, UiComponentStyles};
use warpui::{AppContext, Element, SingletonEntity};

use crate::appearance::Appearance;
use crate::i18n::{tr, Message};
use crate::server::server_api::AIApiError;
use crate::settings_view::custom_inference_modal::CustomEndpointModalAction;
