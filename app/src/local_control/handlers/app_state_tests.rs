use ::local_control::{ActionKind, ErrorCode};

use super::{ensure_surface_available, settings_section, validate_staged_input_text};
use crate::features::FeatureFlag;
use crate::local_control::handlers::metadata::SurfaceDestination;
use crate::settings_view::SettingsSection;

#[test]
fn staged_input_rejects_line_breaks_and_control_sequences() {
    assert!(validate_staged_input_text(ActionKind::InputInsert, "safe staged text").is_ok());

    for text in ["line\nbreak", "line\rbreak", "tab\tbreak", "\u{1b}[31m"] {
        let error = validate_staged_input_text(ActionKind::InputInsert, text).err();
        assert!(error.is_some_and(|error| error.code == ErrorCode::InvalidParams));
    }
}

#[test]
fn unavailable_surface_open_returns_structured_error() {
    let flag_guard = FeatureFlag::AgentManagementView.override_enabled(false);
    warpui::App::test((), |mut app| async move {
        let error = app
            .update(|ctx| {
                ensure_surface_available(
                    ActionKind::SurfaceAgentManagementOpen,
                    SurfaceDestination::AgentManagement,
                    ctx,
                )
            })
            .expect_err("disabled surface is rejected");
        assert_eq!(error.code, ErrorCode::UnsupportedAction);
        assert!(error.message.contains("surface.agent_management.open"));
    });
    drop(flag_guard);
}

#[test]
#[serial_test::serial]
fn zyh_surface_settings_open_redirects_forbidden_pages_to_warp_agent() {
    for page in ["Account", "Billing and usage", "CloudEnvironments"] {
        assert_eq!(
            settings_section(page.to_string()).expect("page should resolve"),
            SettingsSection::WarpAgent
        );
    }
}

#[test]
#[serial_test::serial]
fn surface_settings_open_still_rejects_warp_drive() {
    let error = settings_section("ZYH Drive".to_string()).expect_err("Warp Drive is unsupported");
    assert_eq!(error.code, ErrorCode::UnsupportedAction);
}
