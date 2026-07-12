use warp_core::features::FeatureFlag;
use warpui::{App, SingletonEntity};

use super::*;
use crate::auth::AuthStateProvider;

#[test]
#[serial_test::serial]
fn local_only_tui_starts_logged_in_without_auth_flow() {
    let _flag = FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(true);

    App::test((), |mut app| async move {
        app.update(|ctx| {
            ctx.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());

            init(Box::new(|_| {}), ctx);

            assert!(matches!(
                TuiLoginModel::as_ref(ctx).phase(),
                TuiLoginPhase::LoggedIn
            ));
            assert!(!should_create_anonymous_session(false));
        });
    });
}

#[test]
#[serial_test::serial]
fn anonymous_only_tui_still_starts_logged_in_and_requests_anonymous_session() {
    let _local_flag = FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(false);
    let _anonymous_flag = FeatureFlag::AnonymousOnlyMode.override_enabled(true);

    assert!(matches!(
        initial_login_phase(false),
        TuiLoginPhase::LoggedIn
    ));
    assert!(should_create_anonymous_session(false));
    assert!(!should_create_anonymous_session(true));
}
