use ::ai::api_keys::ApiKeys;

use crate::ai::agent::api::RequestParams;
use crate::ai::agent::runtime::configuration::{ChatCompletionsProvider, RunConfigurationError};
use crate::ai::agent::runtime::MissingProviderField;

pub(crate) fn validate_provider_configuration(
    keys: &ApiKeys,
    selected_model: &str,
) -> Result<(), MissingProviderField> {
    let Some(endpoint) = keys.custom_endpoints.iter().find(|endpoint| {
        endpoint
            .models
            .iter()
            .any(|model| model.config_key == selected_model)
    }) else {
        return if keys
            .custom_endpoints
            .iter()
            .any(|endpoint| endpoint.url_is_valid())
        {
            Err(MissingProviderField::Model)
        } else {
            Err(MissingProviderField::BaseUrl)
        };
    };

    if !endpoint.url_is_valid() {
        return Err(MissingProviderField::BaseUrl);
    }
    if !endpoint
        .models
        .iter()
        .any(|model| model.config_key == selected_model && model.is_valid_for_request())
    {
        return Err(MissingProviderField::Model);
    }
    if endpoint.api_key.trim().is_empty() {
        return Err(MissingProviderField::ApiKey);
    }

    Ok(())
}

pub(crate) fn validate_provider_inventory(keys: &ApiKeys) -> Result<(), MissingProviderField> {
    let valid_url_endpoints = keys
        .custom_endpoints
        .iter()
        .filter(|endpoint| endpoint.url_is_valid())
        .collect::<Vec<_>>();
    if valid_url_endpoints.is_empty() {
        return Err(MissingProviderField::BaseUrl);
    }

    let configured_model_endpoints = valid_url_endpoints
        .into_iter()
        .filter(|endpoint| {
            endpoint
                .models
                .iter()
                .any(|model| model.is_valid_for_request())
        })
        .collect::<Vec<_>>();
    if configured_model_endpoints.is_empty() {
        return Err(MissingProviderField::Model);
    }

    if configured_model_endpoints
        .iter()
        .all(|endpoint| endpoint.api_key.trim().is_empty())
    {
        return Err(MissingProviderField::ApiKey);
    }

    Ok(())
}

pub(super) fn selected_custom_provider(
    params: &RequestParams,
) -> Result<ChatCompletionsProvider, MissingProviderField> {
    if !params.model_config_is_backed_by_custom_providers() {
        return Err(MissingProviderField::Model);
    }
    let selected_model = params.model.as_str();
    let (provider, model) = params
        .custom_model_providers
        .as_ref()
        .ok_or(MissingProviderField::Model)?
        .providers
        .iter()
        .find_map(|provider| {
            provider
                .models
                .iter()
                .find(|model| model.config_key == selected_model)
                .map(|model| (provider, model))
        })
        .ok_or(MissingProviderField::Model)?;

    if provider.api_key.trim().is_empty() {
        return Err(MissingProviderField::ApiKey);
    }

    ChatCompletionsProvider::new(
        &provider.base_url,
        model.slug.clone(),
        provider.api_key.clone(),
    )
    .map_err(|error| match error {
        RunConfigurationError::InvalidProviderUrl => MissingProviderField::BaseUrl,
        RunConfigurationError::EmptyModel => MissingProviderField::Model,
        RunConfigurationError::EmptyWorkingDirectory
        | RunConfigurationError::InvalidContextLimit => MissingProviderField::Model,
    })
}
