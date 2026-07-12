use serde_json::json;
use warp_cli::agent::Harness;
use warp_cli::artifact::{
    ArtifactCommand, DownloadArtifactArgs, GetArtifactArgs, UploadArtifactArgs,
};
use warp_cli::task::{MessageCommand, MessageSendArgs, MessageWatchArgs, TaskCommand};
use warp_cli::CliCommand;
use warp_core::telemetry::TelemetryEvent;
use warpui::App;
use warpui_extras::user_preferences;

use super::{command_requires_auth, command_to_telemetry_event, reconcile_task_harness};

const TASK_ID: &str = "00000000-0000-0000-0000-000000000001";

fn register_private_preferences(ctx: &mut warpui::AppContext) {
    ctx.add_singleton_model(move |_| -> settings::PrivatePreferences {
        settings::PrivatePreferences::new(
            Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
        )
    });
}

#[test]
fn logout_does_not_require_auth() {
    assert!(!command_requires_auth(&CliCommand::Logout));
}

#[test]
fn login_does_not_require_auth() {
    assert!(!command_requires_auth(&CliCommand::Login));
}

#[test]
#[serial_test::serial]
fn local_only_login_returns_stable_error() {
    let _flag =
        warp_core::features::FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(true);

    App::test((), |mut app| async move {
        let err =
            app.update(|ctx| super::admin::login(ctx).expect_err("login should be unavailable"));

        assert_eq!(
            err.to_string(),
            crate::local_mode::account_sign_in_unavailable_message()
        );
    });
}

#[test]
#[serial_test::serial]
fn local_only_whoami_output_returns_local_identity() {
    let _flag =
        warp_core::features::FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(true);

    App::test((), |mut app| async move {
        let (output, text_output) = app.update(|ctx| {
            register_private_preferences(ctx);
            let output =
                super::admin::local_whoami_output(ctx).expect("local whoami should be available");
            let text_output = super::admin::format_local_whoami_output(
                &output,
                warp_cli::agent::OutputFormat::Text,
            )
            .expect("local whoami text output should render");
            (output, text_output)
        });

        assert!(output.uid.starts_with("local:"));
        assert_eq!(text_output, output.uid);
        assert_eq!(output.principal_type, "local");
        assert!(output.display_name.is_none());
        assert!(output.email.is_none());
        assert!(output.team_uid.is_none());
        assert!(output.team_name.is_none());
    });
}

#[test]
#[serial_test::serial]
fn local_only_logout_returns_stable_error() {
    let _flag =
        warp_core::features::FeatureFlag::LocalOnlyCustomProviderMode.override_enabled(true);

    App::test((), |mut app| async move {
        let err =
            app.update(|ctx| super::admin::logout(ctx).expect_err("logout should be unavailable"));

        assert_eq!(
            err.to_string(),
            crate::local_mode::account_logout_unavailable_message()
        );
    });
}

#[test]
fn artifact_download_requires_auth() {
    assert!(command_requires_auth(&CliCommand::Artifact(
        ArtifactCommand::Download(DownloadArtifactArgs {
            artifact_uid: "artifact-123".to_string(),
            out: None,
        },)
    )));
}

#[test]
fn run_message_send_requires_auth() {
    assert!(command_requires_auth(&CliCommand::Run(
        TaskCommand::Message(MessageCommand::Send(MessageSendArgs {
            to: vec!["run-456".to_string()],
            subject: "subject".to_string(),
            body: "body".to_string(),
            sender_run_id: "run-123".to_string(),
        }),)
    )));
}

#[test]
fn artifact_get_requires_auth() {
    assert!(command_requires_auth(&CliCommand::Artifact(
        ArtifactCommand::Get(GetArtifactArgs {
            artifact_uid: "artifact-123".to_string(),
        },)
    )));
}

#[test]
fn artifact_upload_requires_auth() {
    assert!(command_requires_auth(&CliCommand::Artifact(
        ArtifactCommand::Upload(UploadArtifactArgs {
            path: "artifact.txt".into(),
            run_id: Some("run-123".to_string()),
            conversation_id: None,
            description: None,
        },)
    )));
}

#[test]
#[serial_test::serial]
fn run_message_send_telemetry_uses_canonical_harness_from_env() {
    std::env::set_var("OZ_HARNESS", "  CLAUDE  ");
    let event = command_to_telemetry_event(&CliCommand::Run(TaskCommand::Message(
        MessageCommand::Send(MessageSendArgs {
            to: vec!["run-456".to_string()],
            subject: "subject".to_string(),
            body: "body".to_string(),
            sender_run_id: "run-123".to_string(),
        }),
    )));
    std::env::remove_var("OZ_HARNESS");

    assert_eq!(event.payload(), Some(json!({ "harness": "claude" })));
}

#[test]
#[serial_test::serial]
fn run_message_send_telemetry_supports_claude_code_alias() {
    std::env::set_var("OZ_HARNESS", "CLAUDE_CODE");
    let event = command_to_telemetry_event(&CliCommand::Run(TaskCommand::Message(
        MessageCommand::Send(MessageSendArgs {
            to: vec!["run-456".to_string()],
            subject: "subject".to_string(),
            body: "body".to_string(),
            sender_run_id: "run-123".to_string(),
        }),
    )));
    std::env::remove_var("OZ_HARNESS");

    assert_eq!(event.payload(), Some(json!({ "harness": "claude" })));
}

#[test]
#[serial_test::serial]
fn run_message_send_telemetry_supports_opencode_harness() {
    std::env::set_var("OZ_HARNESS", "opencode");
    let event = command_to_telemetry_event(&CliCommand::Run(TaskCommand::Message(
        MessageCommand::Send(MessageSendArgs {
            to: vec!["run-456".to_string()],
            subject: "subject".to_string(),
            body: "body".to_string(),
            sender_run_id: "run-123".to_string(),
        }),
    )));
    std::env::remove_var("OZ_HARNESS");

    assert_eq!(event.payload(), Some(json!({ "harness": "opencode" })));
}

#[test]
#[serial_test::serial]
fn run_message_send_telemetry_defaults_to_unknown_harness() {
    std::env::remove_var("OZ_HARNESS");
    let event = command_to_telemetry_event(&CliCommand::Run(TaskCommand::Message(
        MessageCommand::Send(MessageSendArgs {
            to: vec!["run-456".to_string()],
            subject: "subject".to_string(),
            body: "body".to_string(),
            sender_run_id: "run-123".to_string(),
        }),
    )));

    assert_eq!(event.payload(), Some(json!({ "harness": "unknown" })));
}

#[test]
fn reconcile_task_harness_adopts_task_harness_when_cli_uses_default() {
    let mut selected_harness = Harness::Oz;
    let harness = reconcile_task_harness(TASK_ID, &mut selected_harness, Harness::Claude)
        .expect("default harness should adopt task harness");

    assert_eq!(selected_harness, Harness::Claude);
    assert_eq!(harness.harness(), Harness::Claude);
}

#[test]
fn reconcile_task_harness_allows_matching_explicit_harness() {
    let mut selected_harness = Harness::Claude;
    let harness = reconcile_task_harness(TASK_ID, &mut selected_harness, Harness::Claude)
        .expect("matching harness should succeed");

    assert_eq!(selected_harness, Harness::Claude);
    assert_eq!(harness.harness(), Harness::Claude);
}

#[test]
fn reconcile_task_harness_rejects_explicit_mismatch() {
    let mut selected_harness = Harness::Gemini;
    let err = reconcile_task_harness(TASK_ID, &mut selected_harness, Harness::Claude)
        .expect_err("mismatched harness should fail");

    assert_eq!(selected_harness, Harness::Gemini);
    assert!(err.to_string().contains("Task"));
    assert!(err.to_string().contains("--harness gemini"));
    assert!(err.to_string().contains("claude"));
}

#[test]
#[serial_test::serial]
fn run_message_watch_telemetry_defaults_to_unknown_harness() {
    std::env::remove_var("OZ_HARNESS");
    let event = command_to_telemetry_event(&CliCommand::Run(TaskCommand::Message(
        MessageCommand::Watch(MessageWatchArgs {
            run_id: "run-123".to_string(),
            since_sequence: 0,
        }),
    )));

    assert_eq!(event.payload(), Some(json!({ "harness": "unknown" })));
}
