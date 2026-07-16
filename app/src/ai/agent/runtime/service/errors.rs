use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeStartError {
    BridgeStartupFailed,
    RunAlreadyActive,
    MissingPersistence,
    MissingProvider,
    InvalidRunConfiguration,
    TranscriptProjectionFailed,
}

impl fmt::Display for RuntimeStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BridgeStartupFailed => write!(f, "local Bridge runtime could not be started"),
            Self::RunAlreadyActive => write!(f, "conversation already has an active Agent Run"),
            Self::MissingPersistence => write!(f, "runtime persistence is unavailable"),
            Self::MissingProvider => write!(f, "custom Provider configuration is unavailable"),
            Self::InvalidRunConfiguration => write!(f, "runtime run configuration is invalid"),
            Self::TranscriptProjectionFailed => write!(f, "runtime transcript could not be built"),
        }
    }
}

impl std::error::Error for RuntimeStartError {}
