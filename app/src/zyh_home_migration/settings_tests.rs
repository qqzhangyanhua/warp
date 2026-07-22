use super::{translate_legacy_settings, SettingDisposition, SettingRule, SettingsTranslation};
use crate::zyh_home_migration::settings_rules::SETTINGS_RULES;

#[test]
fn translates_explicit_settings_and_reports_only_unknown_key_paths() {
    let source = r#"
[appearance.text]
font_size = 15

[terminal]
old_copy_on_select = true

[account]
access_token = "cloud-secret"

[custom]
private_value = "unknown-secret"
"#;
    let rules = [
        SettingRule::new("appearance.text.font_size", SettingDisposition::Copy),
        SettingRule::new(
            "terminal.old_copy_on_select",
            SettingDisposition::Rename("terminal.copy_on_select"),
        ),
        SettingRule::new("account.access_token", SettingDisposition::OmitCloud),
    ];

    let SettingsTranslation {
        settings,
        omitted_keys,
        unknown_keys,
    } = translate_legacy_settings(source, &rules).unwrap();

    let translated = settings.to_string();
    assert!(translated.contains("font_size = 15"));
    assert!(translated.contains("copy_on_select = true"));
    assert!(!translated.contains("access_token"));
    assert!(!translated.contains("private_value"));
    assert_eq!(omitted_keys, ["account.access_token"]);
    assert_eq!(unknown_keys, ["custom.private_value"]);

    let report = format!("{omitted_keys:?}{unknown_keys:?}");
    assert!(!report.contains("cloud-secret"));
    assert!(!report.contains("unknown-secret"));
}

#[test]
fn production_rules_preserve_local_settings_and_omit_cloud_settings() {
    let source = r#"
[appearance.text]
font_name = "JetBrains Mono"

[general]
restore_session = true
autoupdate_enabled = true

[agents]
model = "provider/model"

[privacy]
telemetry_enabled = true

[privacy.secret_redaction]
enabled = false

[account]
is_settings_sync_enabled = true

[cloud_platform.third_party_api_keys]
can_use_warp_credits_with_byok = true

[custom]
private_value = "unknown-secret"
"#;

    let translation = translate_legacy_settings(source, SETTINGS_RULES).unwrap();
    let translated = translation.settings.to_string();

    assert!(translated.contains("font_name = \"JetBrains Mono\""));
    assert!(translated.contains("restore_session = true"));
    assert!(translated.contains("model = \"provider/model\""));
    assert!(translated.contains("enabled = false"));
    assert!(!translated.contains("autoupdate_enabled"));
    assert!(!translated.contains("telemetry_enabled"));
    assert!(!translated.contains("is_settings_sync_enabled"));
    assert!(!translated.contains("can_use_warp_credits_with_byok"));
    assert_eq!(
        translation.omitted_keys,
        [
            "general.autoupdate_enabled",
            "privacy.telemetry_enabled",
            "account.is_settings_sync_enabled",
            "cloud_platform.third_party_api_keys.can_use_warp_credits_with_byok",
        ]
    );
    assert_eq!(translation.unknown_keys, ["custom.private_value"]);
}
