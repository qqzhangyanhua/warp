use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::channel::{Channel, ChannelState};

pub const ZYH_HOME_OVERRIDE_ENV: &str = "ZYH_HOME";

const PRODUCTION_HOME_DIR: &str = ".zyh";
const DEVELOPMENT_HOME_DIR: &str = ".zyh-dev";
const CACHE_DIR: &str = "cache";
const LOGS_DIR: &str = "logs";
const TUI_DIR: &str = "tui";
const DATABASE_FILE: &str = "warp.sqlite";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppHomeProfile {
    Production,
    Development,
    Integration,
}

impl AppHomeProfile {
    pub fn current() -> Self {
        match ChannelState::channel() {
            Channel::Integration => Self::Integration,
            Channel::Dev | Channel::Local => Self::Development,
            Channel::Stable | Channel::Preview | Channel::Oss if cfg!(debug_assertions) => {
                Self::Development
            }
            Channel::Stable | Channel::Preview | Channel::Oss => Self::Production,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppHomeError {
    #[error("could not determine the user's home directory")]
    MissingUserHome,
    #[error("integration tests must provide an isolated ZYH home")]
    MissingIntegrationRoot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppHome {
    root: PathBuf,
}

impl AppHome {
    pub fn resolve(
        user_home: &Path,
        profile: AppHomeProfile,
        integration_root: Option<&Path>,
    ) -> Result<Self, AppHomeError> {
        let root = match profile {
            AppHomeProfile::Production => user_home.join(PRODUCTION_HOME_DIR),
            AppHomeProfile::Development => user_home.join(DEVELOPMENT_HOME_DIR),
            AppHomeProfile::Integration => integration_root
                .map(Path::to_path_buf)
                .ok_or(AppHomeError::MissingIntegrationRoot)?,
        };
        Ok(Self { root })
    }

    pub fn current() -> Result<Self, AppHomeError> {
        let profile = AppHomeProfile::current();
        if profile == AppHomeProfile::Integration {
            let root = std::env::var_os(ZYH_HOME_OVERRIDE_ENV)
                .map(PathBuf::from)
                .ok_or(AppHomeError::MissingIntegrationRoot)?;
            return Ok(Self { root });
        }

        let user_home = dirs::home_dir().ok_or(AppHomeError::MissingUserHome)?;
        Self::resolve(&user_home, profile, None)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.clone()
    }

    pub fn data_dir(&self) -> PathBuf {
        self.root.clone()
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.clone()
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join(CACHE_DIR)
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join(LOGS_DIR)
    }

    pub fn tui_config_dir(&self) -> PathBuf {
        self.root.join(TUI_DIR)
    }

    pub fn tui_state_dir(&self) -> PathBuf {
        self.root.join(TUI_DIR)
    }

    pub fn gui_database_file(&self) -> PathBuf {
        self.root.join(DATABASE_FILE)
    }

    pub fn tui_database_file(&self) -> PathBuf {
        self.tui_state_dir().join(DATABASE_FILE)
    }
}
