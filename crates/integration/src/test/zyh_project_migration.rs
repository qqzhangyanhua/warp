use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use settings::Setting as _;
use warp::integration_testing::command_palette::open_command_palette;
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;
use warp::integration_testing::view_getters::{command_palette_view, workspace_view};
use warp::settings::{LocalePreference, LocalePreferenceSetting};
use warpui_core::integration::{AssertionOutcome, TestStep};

use super::new_builder;
use crate::util::write_all_rc_files_for_test;
use crate::Builder;

const TEST_ROOT_ENV: &str = "ZYH_PROJECT_MIGRATION_TEST_ROOT";
const MIGRATION_ACTION: &str = "Migrate Legacy Project Configuration";

fn test_root() -> PathBuf {
    PathBuf::from(std::env::var(TEST_ROOT_ENV).expect("migration test root should be configured"))
}

fn open_migration_command() -> Vec<TestStep> {
    vec![
        open_command_palette(),
        TestStep::new("Select migration command")
            .with_typed_characters(&[MIGRATION_ACTION])
            .add_named_assertion("Migration command is the first result", |app, window_id| {
                let palette = command_palette_view(app, window_id);
                let labels = palette.read(app, |palette, ctx| {
                    palette
                        .search_results(ctx)
                        .take(3)
                        .map(|result| result.accessibility_label())
                        .collect::<Vec<_>>()
                });
                let expected = format!("Selected {MIGRATION_ACTION},");
                if labels
                    .first()
                    .is_some_and(|label| label.starts_with(&expected))
                {
                    AssertionOutcome::Success
                } else {
                    AssertionOutcome::failure(format!(
                        "migration command is not the first result: {labels:?}"
                    ))
                }
            }),
        TestStep::new("Run migration command").with_keystrokes(&["enter"]),
    ]
}

pub fn test_zyh_project_migration_requires_confirmation() -> Builder {
    new_builder()
        .with_user_defaults(HashMap::from([(
            LocalePreferenceSetting::storage_key().to_owned(),
            serde_json::to_string(&LocalePreference::En)
                .expect("locale preference should serialize"),
        )]))
        .with_setup(|utils| {
            let test_dir = utils.test_dir();
            let status = Command::new("git")
                .args(["init", test_dir.to_str().expect("test path should be UTF-8")])
                .status()
                .expect("git should be available");
            assert!(status.success(), "test repository should initialize");

            let workflow = test_dir.join(".warp/workflows/test.yaml");
            std::fs::create_dir_all(workflow.parent().unwrap())
                .expect("legacy workflow directory should be created");
            std::fs::write(&workflow, "name: Test\ncommand: echo test\n")
                .expect("legacy workflow should be written");
            std::fs::write(test_dir.join(".warp/unsupported.txt"), "unsupported")
                .expect("unsupported legacy file should be written");

            let dir_string = test_dir
                .to_str()
                .expect("test directory should be UTF-8");
            write_all_rc_files_for_test(&test_dir, format!("cd {dir_string}"));
            utils.set_env(TEST_ROOT_ENV, Some(dir_string.to_owned()));
        })
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            TestStep::new("Opening a repository does not migrate legacy configuration")
                .add_named_assertion("ZYH project directory is absent", |_, _| {
                    if test_root().join(".zyh").exists() {
                        AssertionOutcome::failure(
                            "opening the repository unexpectedly created .zyh".to_owned(),
                        )
                    } else {
                        AssertionOutcome::Success
                    }
                }),
        )
        .with_steps(open_migration_command())
        .with_step(
            new_step_with_default_assertions("Wait for migration preview").add_named_assertion(
                "Migration preview is visible",
                |app, window_id| {
                    let workspace = workspace_view(app, window_id);
                    if workspace.read(app, |workspace, ctx| {
                        workspace.is_zyh_project_migration_preview_visible(ctx)
                    }) {
                        AssertionOutcome::Success
                    } else {
                        AssertionOutcome::failure("migration preview is not visible".to_owned())
                    }
                },
            ),
        )
        .with_step(
            new_step_with_default_assertions("Decline migration")
                .with_keystrokes(&["escape"])
                .add_named_assertion("Decline leaves repository unchanged", |app, window_id| {
                    let workspace = workspace_view(app, window_id);
                    let dialog_open = workspace
                        .read(app, |workspace, _| workspace.is_zyh_project_migration_dialog_open());
                    if dialog_open || test_root().join(".zyh").exists() {
                        AssertionOutcome::failure(
                            "declining migration did not close without writing".to_owned(),
                        )
                    } else {
                        AssertionOutcome::Success
                    }
                }),
        )
        .with_steps(open_migration_command())
        .with_step(
            new_step_with_default_assertions("Wait for migration preview again")
                .add_named_assertion("Migration preview is visible", |app, window_id| {
                    let workspace = workspace_view(app, window_id);
                    if workspace.read(app, |workspace, ctx| {
                        workspace.is_zyh_project_migration_preview_visible(ctx)
                    }) {
                        AssertionOutcome::Success
                    } else {
                        AssertionOutcome::failure("migration preview is not visible".to_owned())
                    }
                }),
        )
        .with_step(new_step_with_default_assertions("Confirm migration").with_keystrokes(&["enter"]))
        .with_step(
            new_step_with_default_assertions("Inspect migration result").add_named_assertion(
                "Only approved files were copied and result remains visible",
                |app, window_id| {
                    let workspace = workspace_view(app, window_id);
                    let result_visible = workspace.read(app, |workspace, ctx| {
                        workspace.is_zyh_project_migration_result_visible(ctx)
                    });
                    let root = test_root();
                    let copied = root.join(".zyh/workflows/test.yaml").exists();
                    let unsupported = root.join(".zyh/unsupported.txt").exists();
                    let legacy = root.join(".warp/workflows/test.yaml").exists();
                    if result_visible && copied && legacy && !unsupported {
                        AssertionOutcome::Success
                    } else {
                        AssertionOutcome::failure(format!(
                            "unexpected migration state: result_visible={result_visible}, copied={copied}, legacy={legacy}, unsupported={unsupported}"
                        ))
                    }
                },
            ),
        )
}
