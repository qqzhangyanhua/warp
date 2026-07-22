use settings::Setting as _;
use warp::features::FeatureFlag;
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;
use warp::settings::FontSettings;
use warp_core::paths::ZYH_HOME_OVERRIDE_ENV;
use warpui_core::integration::TestStep;
use warpui_core::{async_assert, async_assert_eq, SingletonEntity};

use super::{new_builder, Builder};

pub fn test_integration_startup_uses_isolated_zyh_home() -> Builder {
    FeatureFlag::SettingsFile.set_enabled(true);

    new_builder()
        .with_setup(|utils| {
            let test_root = utils
                .test_dir()
                .canonicalize()
                .unwrap_or_else(|_| utils.test_dir());
            let expected_zyh_home = test_root.join("zyh-home");
            let configured_zyh_home = std::env::var_os(ZYH_HOME_OVERRIDE_ENV)
                .map(std::path::PathBuf::from)
                .expect("integration harness must set an isolated ZYH_HOME");
            assert_eq!(configured_zyh_home, expected_zyh_home);

            std::fs::create_dir_all(&configured_zyh_home).expect("should create isolated ZYH home");
            std::fs::write(
                configured_zyh_home.join("settings.toml"),
                "[appearance.text]\nfont_size = 14.0\n",
            )
            .expect("should seed isolated ZYH settings");

            let development_home = test_root.join(".zyh-dev");
            std::fs::create_dir_all(&development_home)
                .expect("should create development decoy home");
            std::fs::write(
                development_home.join("settings.toml"),
                "[appearance.text]\nfont_size = 30.0\n",
            )
            .expect("should seed development decoy settings");
        })
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Settings load from isolated ZYH home")
                .add_named_assertion("monospace_font_size == 14.0", |app, _| {
                    app.read(|ctx| {
                        let font_size = FontSettings::as_ref(ctx).monospace_font_size.value();
                        async_assert_eq!(
                            *font_size,
                            14.0,
                            "startup must read settings from the harness-provided ZYH home"
                        )
                    })
                }),
        )
}

pub fn test_settings_writes_are_owner_only_atomic_and_backed_up() -> Builder {
    FeatureFlag::SettingsFile.set_enabled(true);

    new_builder()
        .with_setup(|_| {
            let settings_path = warp::settings::user_preferences_toml_file_path();
            std::fs::create_dir_all(settings_path.parent().unwrap())
                .expect("should create isolated ZYH home");
            std::fs::write(&settings_path, "[appearance.text]\nfont_size = 14.0\n")
                .expect("should seed settings");
        })
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            TestStep::new("Update a public setting").with_action(|app, _, _| {
                FontSettings::handle(app).update(app, |settings, ctx| {
                    settings
                        .monospace_font_size
                        .set_value(18.0, ctx)
                        .expect("should update font size");
                });
            }),
        )
        .with_step(
            new_step_with_default_assertions("Settings write is durable and private")
                .add_named_assertion("current and backup settings are correct", |_, _| {
                    let settings_path = warp::settings::user_preferences_toml_file_path();
                    let current = std::fs::read_to_string(&settings_path).unwrap_or_default();
                    let backup =
                        std::fs::read_to_string(format!("{}.bak", settings_path.to_string_lossy()))
                            .unwrap_or_default();
                    async_assert!(
                        current.contains("font_size = 18.0") && backup.contains("font_size = 14.0"),
                        "normal settings writes must retain one last-known-good backup"
                    )
                })
                .add_named_assertion("settings file is owner-only", |_, _| {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt as _;

                        let settings_path = warp::settings::user_preferences_toml_file_path();
                        let mode = std::fs::metadata(settings_path)
                            .map(|metadata| metadata.permissions().mode() & 0o777)
                            .unwrap_or_default();
                        async_assert_eq!(mode, 0o600, "settings.toml must be owner-only")
                    }
                    #[cfg(not(unix))]
                    {
                        warpui_core::integration::AssertionOutcome::Success
                    }
                }),
        )
}
