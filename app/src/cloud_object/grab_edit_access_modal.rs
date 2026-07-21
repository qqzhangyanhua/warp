use warpui::elements::{Container, Element, MouseStateHandle, Text};
use warpui::fonts::{Properties, Style, Weight};
use warpui::platform::Cursor;
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::UiComponent;
use warpui::{AppContext, Entity, SingletonEntity, TypedActionView, View, ViewContext};

use crate::appearance::Appearance;
use crate::i18n::{tr_cached, Message};
use crate::ui_components::buttons::close_button;
use crate::ui_components::dialog::{dialog_styles, Dialog};

fn edit_anyway_cta_label() -> &'static str {
    tr_cached(Message::EditAnyway)
}
fn cancel_cta_label() -> &'static str {
    tr_cached(Message::CommonCancel)
}
fn edit_anyway_text() -> &'static str {
    tr_cached(Message::TakeEditControlsForceView)
}
fn currently_edited_label() -> &'static str {
    tr_cached(Message::NotebookCurrentlyEdited)
}

#[derive(Default)]
struct MouseStateHandles {
    close_button: MouseStateHandle,
    edit_anyway_button: MouseStateHandle,
    cancel_button: MouseStateHandle,
}

pub struct GrabEditAccessModal {
    mouse_state_handles: MouseStateHandles,
}

impl Default for GrabEditAccessModal {
    fn default() -> Self {
        Self::new()
    }
}

impl GrabEditAccessModal {
    pub fn new() -> Self {
        Self {
            mouse_state_handles: Default::default(),
        }
    }

    pub fn close(&self, ctx: &mut ViewContext<Self>) {
        ctx.emit(GrabEditAccessModalEvent::Close);
    }

    pub fn grab_edit_access(&self, ctx: &mut ViewContext<Self>) {
        // TODO @ianhodge actually make the call to grab access on the server
        ctx.emit(GrabEditAccessModalEvent::GrabEditAccess);
    }

    pub fn render_modal(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder();

        let description = Text::new(edit_anyway_text(), appearance.ui_font_family(), 13.)
            .with_style(Properties {
                style: Style::Normal,
                weight: Weight::Bold,
            })
            .with_color(theme.active_ui_text_color().into())
            .finish();

        let close_button = close_button(appearance, self.mouse_state_handles.close_button.clone())
            .build()
            .on_click(|ctx, _, _| ctx.dispatch_typed_action(GrabEditAccessModalAction::Close))
            .with_cursor(Cursor::PointingHand)
            .finish();

        Dialog::new(
            currently_edited_label().to_string(),
            None,
            dialog_styles(appearance),
        )
        .with_close_button(close_button)
        .with_child(description)
        .with_bottom_row_child(
            Container::new(
                ui_builder
                    .button(
                        ButtonVariant::Basic,
                        self.mouse_state_handles.cancel_button.clone(),
                    )
                    .with_text_label(cancel_cta_label().to_string())
                    .build()
                    .on_click(|ctx, _, _| {
                        ctx.dispatch_typed_action(GrabEditAccessModalAction::Close)
                    })
                    .with_cursor(Cursor::PointingHand)
                    .finish(),
            )
            .with_padding_right(5.)
            .finish(),
        )
        .with_bottom_row_child(
            ui_builder
                .button(
                    ButtonVariant::Warn,
                    self.mouse_state_handles.edit_anyway_button.clone(),
                )
                .with_text_label(edit_anyway_cta_label().to_string())
                .build()
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(GrabEditAccessModalAction::GrabEditAccess)
                })
                .with_cursor(Cursor::PointingHand)
                .finish(),
        )
        .build()
        .on_dismiss(|ctx, _app| ctx.dispatch_typed_action(GrabEditAccessModalAction::Close))
        .finish()
    }
}

impl Entity for GrabEditAccessModal {
    type Event = GrabEditAccessModalEvent;
}

#[derive(PartialEq, Eq)]
pub enum GrabEditAccessModalEvent {
    Close,
    GrabEditAccess,
}

#[derive(Clone, Copy, Debug)]
pub enum GrabEditAccessModalAction {
    Close,
    GrabEditAccess,
}

impl TypedActionView for GrabEditAccessModal {
    type Action = GrabEditAccessModalAction;

    fn handle_action(&mut self, action: &GrabEditAccessModalAction, ctx: &mut ViewContext<Self>) {
        use GrabEditAccessModalAction::*;

        match action {
            Close => self.close(ctx),
            GrabEditAccess => self.grab_edit_access(ctx),
        }
    }
}

impl View for GrabEditAccessModal {
    fn ui_name() -> &'static str {
        "GrabEditAccessModal"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        self.render_modal(appearance)
    }
}
