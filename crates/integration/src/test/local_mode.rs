use startup_request_recorder::RequestRecorder;
use warp::features::FeatureFlag;
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::tab::{assert_pane_title, assert_tab_title};
use warp::integration_testing::terminal::{
    assert_input_is_focused, wait_until_bootstrapped_single_pane_for_tab,
};
use warp::integration_testing::workspace::assert_tab_count;
use warp::settings_view::{SettingsSection, SettingsView};
use warpui_core::integration::{AssertionOutcome, TestStep};
use warpui_core::{async_assert_eq, ViewHandle};

use crate::Builder;

pub fn test_local_only_gui_startup_and_settings_respect_network_boundary() -> Builder {
    FeatureFlag::LocalOnlyCustomProviderMode.set_enabled(true);
    let recorder = RequestRecorder::start().expect("startup request recorder should start");
    for (variable, value) in recorder.proxy_environment() {
        std::env::set_var(variable, value);
    }

    Builder::new()
        .with_step(
            wait_until_bootstrapped_single_pane_for_tab(0).add_named_assertion(
                "Local-only terminal input is focused",
                assert_input_is_focused(),
            ),
        )
        .with_step(
            new_step_with_default_assertions("Open Settings in Local-only Mode")
                .with_keystrokes(&["cmdorctrl-,"])
                .add_assertion(assert_tab_count(2))
                .add_assertion(assert_tab_title(1, "Settings"))
                .add_assertion(assert_pane_title(1, 0, "Settings"))
                .add_named_assertion(
                    "Settings opens the local Agent section",
                    |app, window_id| {
                        let settings_views: Vec<ViewHandle<SettingsView>> = app
                            .views_of_type(window_id)
                            .expect("Settings views should be available");
                        assert_eq!(settings_views.len(), 1, "Settings view should exist");

                        settings_views[0].read(app, |view, _| {
                            async_assert_eq!(
                                view.current_settings_section(),
                                SettingsSection::WarpAgent
                            )
                        })
                    },
                ),
        )
        .with_step(
            TestStep::new("Record GUI startup network baseline").add_assertion(move |_, _| {
                let requests = recorder
                    .requests()
                    .expect("GUI request recorder should synchronize");
                assert!(
                    requests.is_empty(),
                    "GUI startup made app-initiated requests: {requests:#?}"
                );
                AssertionOutcome::Success
            }),
        )
}
