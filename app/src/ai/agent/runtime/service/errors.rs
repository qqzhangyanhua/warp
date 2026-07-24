use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MissingProviderField {
    BaseUrl,
    Model,
    ApiKey,
}

impl MissingProviderField {
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::BaseUrl => "Base URL",
            Self::Model => "Model",
            Self::ApiKey => "API Key",
        }
    }
}

impl fmt::Display for MissingProviderField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Provider {} is not configured", self.display_name())
    }
}

impl std::error::Error for MissingProviderField {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeStartError {
    BridgeStartupFailed,
    RunAlreadyActive,
    MissingPersistence,
    MissingProvider(MissingProviderField),
    InvalidRunConfiguration,
    TranscriptProjectionFailed,
}

impl fmt::Display for RuntimeStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BridgeStartupFailed => write!(f, "local Bridge runtime could not be started"),
            Self::RunAlreadyActive => write!(f, "conversation already has an active Agent Run"),
            Self::MissingPersistence => write!(f, "runtime persistence is unavailable"),
            Self::MissingProvider(field) => {
                write!(f, "Provider {} is not configured", field.display_name())
            }
            Self::InvalidRunConfiguration => write!(f, "runtime run configuration is invalid"),
            Self::TranscriptProjectionFailed => write!(f, "runtime transcript could not be built"),
        }
    }
}

impl std::error::Error for RuntimeStartError {}
