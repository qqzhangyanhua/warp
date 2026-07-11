use serde::{Deserialize, Serialize};
use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename_all = "snake_case")]
pub enum LocalePreference {
    #[default]
    System,
    En,
    ZhCn,
}

define_settings_group!(LocalizationSettings, settings: [
    locale_preference: LocalePreferenceSetting {
        type: LocalePreference,
        default: LocalePreference::System,
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.general.language",
        description: "The display language for Warp.",
    },
]);
