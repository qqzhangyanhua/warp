use warpui::elements::{Element, Flex, ParentElement};
use warpui::ui_components::switch::SwitchStateHandle;
use warpui::{AppContext, SingletonEntity};

use super::{
    render_ai_setting_description, render_ai_setting_toggle, AISettingsPageAction,
    AISettingsPageView,
};
use crate::i18n::{tr, Message};
use crate::settings::{AISettings, PersonalMemoryEnabled};

#[derive(Default)]
pub(super) struct PersonalMemoryToggle {
    state: SwitchStateHandle,
}

impl PersonalMemoryToggle {
    pub(super) fn render(&self, view: &AISettingsPageView, app: &AppContext) -> Box<dyn Element> {
        let ai_settings = AISettings::as_ref(app);
        let toggleable = ai_settings.is_any_ai_enabled(app);
        let toggle = render_ai_setting_toggle::<PersonalMemoryEnabled>(
            tr(app, Message::PersonalMemoryEnable),
            AISettingsPageAction::TogglePersonalMemory,
            *ai_settings.personal_memory_enabled,
            toggleable,
            self.state.clone(),
            &view.local_only_icon_tooltip_states,
            app,
        );
        let description = render_ai_setting_description(
            tr(app, Message::PersonalMemoryEnableDescription),
            toggleable,
            app,
        );
        Flex::column()
            .with_child(toggle)
            .with_child(description)
            .finish()
    }
}
