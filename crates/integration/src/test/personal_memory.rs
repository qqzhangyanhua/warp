use std::path::{Path, PathBuf};
use std::time::Duration;

use warp::features::FeatureFlag;
use warp::integration_testing::agent_mode::{
    assert_latest_personal_memory_response_text, assert_latest_personal_memory_source,
    assert_personal_memory_management_surface, configure_personal_memory_test_provider,
    enter_agent_view, open_latest_personal_memory_source, start_new_personal_memory_conversation,
    submit_personal_memory_query_and_wait_until_done, user_defaults_map_for_ai_input,
    user_defaults_map_with_active_ai,
};
use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;

use crate::Builder;

const REMEMBER_REQUEST: &str = "记住我的管理员帐号是admin123";
const RECALL_REQUEST: &str = "我的管理员帐号是什么？";
const EXACT_VALUE: &str = "admin123";

pub fn test_personal_memory_remember_recall_and_open_source() -> Builder {
    FeatureFlag::PersonalMemory.set_enabled(true);
    let mut user_defaults = user_defaults_map_for_ai_input();
    user_defaults.extend(user_defaults_map_with_active_ai(true));

    Builder::new()
        .with_user_defaults(user_defaults)
        .with_setup(|utils| {
            let observer_directory = utils.test_dir().join("personal-memory-bridge");
            std::fs::create_dir_all(&observer_directory)
                .expect("Personal Memory Bridge observer directory should be created");
            let bridge = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/warp-bridge/test/supervisor-fake-bridge.mjs")
                .canonicalize()
                .expect("Personal Memory fake Bridge should exist");
            let node = node_executable();
            let arguments = std::env::join_paths([
                bridge.as_path(),
                Path::new("text-run-personal-memory-gui"),
                observer_directory.as_path(),
            ])
            .expect("Personal Memory fake Bridge arguments should be valid")
            .to_string_lossy()
            .into_owned();
            utils.set_env(
                "WARP_PI_BRIDGE_PROGRAM",
                Some(node.to_string_lossy().into_owned()),
            );
            utils.set_env("WARP_PI_BRIDGE_ARGS", Some(arguments));
        })
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(configure_personal_memory_test_provider())
        .with_step(enter_agent_view())
        .with_step(
            submit_personal_memory_query_and_wait_until_done(
                REMEMBER_REQUEST,
                Duration::from_secs(30),
            )
            .add_named_assertion(
                "The write receipt confirms the exact value",
                assert_latest_personal_memory_response_text(|text| {
                    text.contains("已记住") && text.contains(EXACT_VALUE)
                }),
            ),
        )
        .with_step(start_new_personal_memory_conversation())
        .with_step(
            submit_personal_memory_query_and_wait_until_done(
                RECALL_REQUEST,
                Duration::from_secs(30),
            )
            .add_named_assertion(
                "The later conversation recalls the exact value",
                assert_latest_personal_memory_response_text(|text| text.contains(EXACT_VALUE)),
            )
            .add_named_assertion(
                "The answer renders a committed Personal Memory source",
                assert_latest_personal_memory_source(EXACT_VALUE),
            ),
        )
        .with_step(open_latest_personal_memory_source().add_named_assertion(
            "The source opens the canonical record in Personal Memory Settings",
            assert_personal_memory_management_surface(EXACT_VALUE),
        ))
}

fn node_executable() -> PathBuf {
    let executable = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(executable))
        .find(|candidate| candidate.is_file())
        .expect("Node.js must be available for the Personal Memory GUI test")
}
