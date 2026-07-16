use crate::ai::agent::api::RequestParams;
use crate::ai::agent::runtime::configuration::ChatCompletionsProvider;

pub(super) fn selected_custom_provider(params: &RequestParams) -> Option<ChatCompletionsProvider> {
    if !params.model_config_is_backed_by_custom_providers() {
        return None;
    }
    let selected_model = params.model.as_str();
    params
        .custom_model_providers
        .as_ref()?
        .providers
        .iter()
        .find_map(|provider| {
            provider
                .models
                .iter()
                .find(|model| model.config_key == selected_model)
                .and_then(|model| {
                    ChatCompletionsProvider::new(
                        &provider.base_url,
                        model.slug.clone(),
                        provider.api_key.clone(),
                    )
                    .ok()
                })
        })
}
