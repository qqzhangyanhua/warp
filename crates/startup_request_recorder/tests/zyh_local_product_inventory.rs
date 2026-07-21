use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize)]
struct Inventory {
    network_clients: Vec<NetworkClient>,
    endpoint_classes: Vec<EndpointClass>,
}

#[derive(Deserialize)]
struct NetworkClient {
    id: String,
    disposition: String,
    evidence: Vec<String>,
}

#[derive(Deserialize)]
struct EndpointClass {
    id: String,
    disposition: String,
    values: Vec<String>,
}

fn load_inventory() -> Inventory {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/zyh-local-product-inventory.json");
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn assert_deleted_network_contract(
    inventory: &Inventory,
    client_id: &str,
    expected_evidence: &[&str],
    endpoint_id: &str,
    expected_values: &[&str],
) {
    let client = inventory
        .network_clients
        .iter()
        .find(|client| client.id == client_id)
        .unwrap_or_else(|| panic!("{client_id} client must be inventoried"));

    assert_eq!(client.disposition, "deleted", "{client_id} disposition");
    for expected in expected_evidence {
        assert!(
            client.evidence.iter().any(|path| path == expected),
            "{client_id} must cite {expected}"
        );
    }

    let endpoint_class = inventory
        .endpoint_classes
        .iter()
        .find(|endpoint_class| endpoint_class.id == endpoint_id)
        .unwrap_or_else(|| panic!("{endpoint_id} endpoint class must be inventoried"));

    assert_eq!(
        endpoint_class.disposition, "deleted",
        "{endpoint_id} disposition"
    );
    for expected in expected_values {
        assert!(
            endpoint_class.values.iter().any(|value| value == expected),
            "{endpoint_id} must include {expected}"
        );
    }
}

#[test]
fn google_sts_iam_is_explicitly_classified_for_deletion() {
    let inventory = load_inventory();
    assert_deleted_network_contract(
        &inventory,
        "google_sts_iam_credentials",
        &["app/src/ai/geap_credentials.rs"],
        "google_sts_iam",
        &[
            "https://sts.googleapis.com/v1/token",
            "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/{sa_email}:generateAccessToken",
        ],
    );
}

#[test]
fn xai_oauth_is_explicitly_classified_for_deletion() {
    let inventory = load_inventory();
    assert_deleted_network_contract(
        &inventory,
        "xai_oauth",
        &["crates/ai/src/grok_subscription/oauth.rs"],
        "xai_oauth",
        &[
            "https://auth.x.ai/oauth2/authorize",
            "https://auth.x.ai/oauth2/token",
        ],
    );
}

#[test]
fn cloud_agent_otlp_is_explicitly_classified_for_deletion() {
    let inventory = load_inventory();
    assert_deleted_network_contract(
        &inventory,
        "cloud_agent_otlp",
        &[
            "app/src/tracing/native.rs",
            "app/src/tracing/cloud_agent_auth.rs",
        ],
        "cloud_agent_otlp",
        &["WARP_CLOUD_AGENT_OTLP_ENDPOINT"],
    );
}
