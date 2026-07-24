use ai::api_keys::{ApiKeys, CustomEndpoint, CustomEndpointModel};

use super::provider::validate_provider_inventory;
use super::MissingProviderField;

fn endpoint(url: &str, api_key: &str, model_name: &str) -> CustomEndpoint {
    CustomEndpoint {
        url: url.to_owned(),
        api_key: api_key.to_owned(),
        models: vec![CustomEndpointModel {
            name: model_name.to_owned(),
            config_key: "model-config-key".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn provider_inventory_reports_the_first_missing_configuration_field() {
    assert_eq!(
        validate_provider_inventory(&ApiKeys::default()),
        Err(MissingProviderField::BaseUrl)
    );
    assert_eq!(
        validate_provider_inventory(&ApiKeys {
            custom_endpoints: vec![endpoint("http://localhost:11434/v1", "key", "")],
            ..Default::default()
        }),
        Err(MissingProviderField::Model)
    );
    assert_eq!(
        validate_provider_inventory(&ApiKeys {
            custom_endpoints: vec![endpoint("http://localhost:11434/v1", "", "model")],
            ..Default::default()
        }),
        Err(MissingProviderField::ApiKey)
    );
}

#[test]
fn provider_inventory_accepts_a_complete_provider() {
    let keys = ApiKeys {
        custom_endpoints: vec![endpoint(
            "http://localhost:11434/v1",
            "provider-key",
            "model",
        )],
        ..Default::default()
    };

    assert_eq!(validate_provider_inventory(&keys), Ok(()));
}
