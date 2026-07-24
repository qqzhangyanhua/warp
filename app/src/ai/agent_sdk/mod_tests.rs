use ai::api_keys::{ApiKeys, CustomEndpoint, CustomEndpointModel};
use warp_cli::agent::Harness;

use super::{reconcile_task_harness, validate_local_agent_run};

const TASK_ID: &str = "00000000-0000-0000-0000-000000000001";

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
fn configured_local_agent_run_never_falls_back_to_hosted_services() {
    let keys = ApiKeys {
        custom_endpoints: vec![CustomEndpoint {
            url: "http://localhost:11434/v1".to_owned(),
            api_key: "provider-key".to_owned(),
            models: vec![CustomEndpointModel {
                name: "model".to_owned(),
                config_key: "model-config-key".to_owned(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = validate_local_agent_run(&keys)
        .expect_err("configured local Agent runs must not use the hosted driver");
    assert!(error.to_string().contains("Pi Agent Runtime"));
}
