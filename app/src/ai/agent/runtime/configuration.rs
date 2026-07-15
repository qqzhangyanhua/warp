use serde::Serialize;
use thiserror::Error;
use url::Url;

use super::resources::ResourceSnapshot;
use super::transcript::RuntimeContentBlock;

const MAX_PROVIDER_ATTEMPTS: u8 = 2;
const MAX_PROVIDER_REDIRECTS: u8 = 3;

#[derive(Clone, Serialize)]
pub(super) struct ChatCompletionsProvider {
    protocol: ProviderProtocol,
    base_url: String,
    provider_origin: String,
    model: String,
    api_key: String,
    max_provider_attempts: u8,
    max_redirects: u8,
}

impl ChatCompletionsProvider {
    pub(super) fn new(
        base_url: &str,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, RunConfigurationError> {
        let mut url =
            Url::parse(base_url).map_err(|_| RunConfigurationError::InvalidProviderUrl)?;
        if !matches!(url.scheme(), "http" | "https") || !url.has_host() {
            return Err(RunConfigurationError::InvalidProviderUrl);
        }
        let model = model.into();
        if model.is_empty() {
            return Err(RunConfigurationError::EmptyModel);
        }
        let path = url.path().trim_end_matches('/');
        if !path.ends_with("/chat/completions") {
            url.set_path(&format!("{path}/chat/completions"));
        }
        let provider_origin = url.origin().ascii_serialization();

        Ok(Self {
            protocol: ProviderProtocol::ChatCompletions,
            base_url: url.to_string(),
            provider_origin,
            model,
            api_key: api_key.into(),
            max_provider_attempts: MAX_PROVIDER_ATTEMPTS,
            max_redirects: MAX_PROVIDER_REDIRECTS,
        })
    }

    #[cfg(test)]
    pub(super) fn chat_completions_url(&self) -> &str {
        &self.base_url
    }

    #[cfg(test)]
    pub(super) fn origin(&self) -> &str {
        &self.provider_origin
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProviderProtocol {
    ChatCompletions,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
#[expect(
    dead_code,
    reason = "Runtime Selection maps all reasoning settings when it is enabled in Phase 7"
)]
pub(super) enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

#[derive(Clone, Serialize)]
pub(super) struct RunConfiguration {
    provider: ChatCompletionsProvider,
    working_directory: String,
    context_limit: u64,
    reasoning_effort: ReasoningEffort,
    tool_request_limit: u32,
    tools: Vec<ToolCatalogEntry>,
    resources: Vec<AgentResource>,
}

impl RunConfiguration {
    pub(super) fn text_only(
        provider: ChatCompletionsProvider,
        working_directory: impl Into<String>,
        context_limit: u64,
        reasoning_effort: ReasoningEffort,
        resources: Vec<ResourceSnapshot>,
    ) -> Result<Self, RunConfigurationError> {
        let working_directory = working_directory.into();
        if working_directory.is_empty() {
            return Err(RunConfigurationError::EmptyWorkingDirectory);
        }
        if context_limit == 0 {
            return Err(RunConfigurationError::InvalidContextLimit);
        }
        Ok(Self {
            provider,
            working_directory,
            context_limit,
            reasoning_effort,
            tool_request_limit: 0,
            tools: Vec::new(),
            resources: resources.into_iter().map(AgentResource::from).collect(),
        })
    }
}

#[derive(Clone, Serialize)]
struct ToolCatalogEntry {
    id: String,
    name: String,
    description: String,
    input_schema: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Serialize)]
struct AgentResource {
    id: String,
    name: String,
    content: Vec<RuntimeContentBlock>,
}

impl From<ResourceSnapshot> for AgentResource {
    fn from(snapshot: ResourceSnapshot) -> Self {
        Self {
            id: snapshot.resource_id,
            name: snapshot.name,
            content: snapshot.content,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum RunConfigurationError {
    #[error("Chat Completions Provider Base URL is invalid")]
    InvalidProviderUrl,
    #[error("Chat Completions Provider model is empty")]
    EmptyModel,
    #[error("Agent Run working directory is empty")]
    EmptyWorkingDirectory,
    #[error("Agent Run context limit must be positive")]
    InvalidContextLimit,
}
