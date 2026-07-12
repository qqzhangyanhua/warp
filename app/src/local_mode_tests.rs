use warp_core::features::FeatureFlag;
use warp_core::user_preferences::GetUserPreferences as _;
use warpui::App;
use warpui_extras::user_preferences;

use super::*;

fn init_preferences(ctx: &mut warpui::AppContext) {
    ctx.add_singleton_model(move |_| -> settings::PrivatePreferences {
        settings::PrivatePreferences::new(
            Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
        )
    });
}

#[test]
fn local_identity_is_persisted() {
    App::test((), |mut app| async move {
        app.update(init_preferences);
        let (first, second) = app.update(|ctx| {
            let first = get_or_create_local_identity(ctx).unwrap();
            let second = get_or_create_local_identity(ctx).unwrap();
            (first, second)
        });

        assert_eq!(first, second);
    });
}

#[test]
fn malformed_local_identity_is_replaced() {
    App::test((), |mut app| async move {
        app.update(init_preferences);
        let identity = app.update(|ctx| {
            ctx.private_user_preferences()
                .write_value(LOCAL_IDENTITY_KEY, "not-a-uuid".to_owned())
                .unwrap();

            get_or_create_local_identity(ctx).unwrap()
        });

        assert!(identity.as_uid().starts_with("local:"));
        assert_ne!(identity.as_uid(), "local:not-a-uuid");
    });
}

#[test]
#[serial_test::serial]
fn local_only_policy_follows_feature_flag() {
    let _flag = FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(true);

    assert!(is_local_only_custom_provider_mode());
}

#[test]
#[serial_test::serial]
fn local_only_policy_is_disabled_by_default() {
    let _flag = FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(false);

    assert!(!is_local_only_custom_provider_mode());
}

#[test]
fn local_identity_for_test_uses_provided_id() {
    let id = uuid::Uuid::nil();

    assert_eq!(local_identity_for_test(id).as_uid(), format!("local:{id}"));
}
