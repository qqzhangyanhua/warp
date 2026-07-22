use std::path::{Path, PathBuf};

use crate::channel::{Channel, ChannelState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyPlatform {
    MacOs,
    Linux,
    Windows,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyInstallation {
    channel: Channel,
    project_path: String,
}

impl LegacyInstallation {
    pub fn new(channel: Channel, project_path: impl Into<String>) -> Self {
        Self {
            channel,
            project_path: project_path.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyRoots {
    home_config_dir: PathBuf,
    config_dir: PathBuf,
    data_dir: PathBuf,
    state_dir: PathBuf,
    secure_state_dir: PathBuf,
    logs_dir: PathBuf,
    log_file_name: String,
    cache_dir: PathBuf,
    tui_config_dir: PathBuf,
    tui_state_dir: PathBuf,
}

impl LegacyRoots {
    pub fn current() -> Option<Self> {
        let user_home = dirs::home_dir()?;
        let platform = LegacyPlatform::current()?;
        let project_path = super::project_dirs_for_app_id(
            ChannelState::app_id(),
            ChannelState::data_profile().as_deref(),
        )?
        .project_path()
        .to_string_lossy()
        .into_owned();
        let mut roots = Self::resolve(
            &user_home,
            platform,
            LegacyInstallation::new(ChannelState::channel(), project_path),
        );
        roots.log_file_name = ChannelState::logfile_name().into_owned();
        Some(roots)
    }

    pub fn resolve(
        user_home: &Path,
        platform: LegacyPlatform,
        installation: LegacyInstallation,
    ) -> Self {
        let home_config_dir = user_home.join(home_config_dir_name(installation.channel));
        let log_file_name = legacy_log_file_name(installation.channel).to_owned();
        match platform {
            LegacyPlatform::MacOs => {
                let config_dir = user_home.join(macos_config_dir_name(installation.channel));
                let state_dir = user_home
                    .join("Library")
                    .join("Application Support")
                    .join(&installation.project_path);
                let secure_state_dir = user_home
                    .join("Library")
                    .join("Group Containers")
                    .join("2BBY89MBSN.dev.warp")
                    .join("Library")
                    .join("Application Support")
                    .join(&installation.project_path);
                let logs_dir = user_home.join("Library").join("Logs");
                let tui_config_dir =
                    user_home.join(macos_tui_config_dir_name(installation.channel));
                let tui_state_dir = state_dir.join("tui");
                Self {
                    home_config_dir,
                    config_dir: config_dir.clone(),
                    data_dir: config_dir,
                    state_dir: state_dir.clone(),
                    secure_state_dir,
                    logs_dir,
                    log_file_name,
                    cache_dir: state_dir,
                    tui_config_dir,
                    tui_state_dir,
                }
            }
            LegacyPlatform::Linux => {
                let config_dir = user_home.join(".config").join(&installation.project_path);
                let data_dir = user_home
                    .join(".local")
                    .join("share")
                    .join(&installation.project_path);
                let state_dir = user_home
                    .join(".local")
                    .join("state")
                    .join(&installation.project_path);
                let cache_dir = user_home.join(".cache").join(&installation.project_path);
                let tui_config_dir = config_dir.join("cli");
                let tui_state_dir = state_dir.join("tui");
                Self {
                    home_config_dir,
                    config_dir,
                    data_dir,
                    state_dir: state_dir.clone(),
                    secure_state_dir: state_dir.clone(),
                    logs_dir: state_dir,
                    log_file_name,
                    cache_dir,
                    tui_config_dir,
                    tui_state_dir,
                }
            }
            LegacyPlatform::Windows => {
                let config_dir = windows_join(
                    user_home,
                    &format!(r"AppData\Local\{}\config", installation.project_path),
                );
                let data_dir = windows_join(
                    user_home,
                    &format!(r"AppData\Roaming\{}\data", installation.project_path),
                );
                let state_dir = windows_join(
                    user_home,
                    &format!(r"AppData\Local\{}\data", installation.project_path),
                );
                let cache_dir = windows_join(
                    user_home,
                    &format!(r"AppData\Local\{}\cache", installation.project_path),
                );
                let tui_config_dir = windows_join(&config_dir, "cli");
                let tui_state_dir = windows_join(&state_dir, "tui");
                let logs_dir = windows_join(&state_dir, "logs");
                Self {
                    home_config_dir,
                    config_dir,
                    data_dir,
                    state_dir: state_dir.clone(),
                    secure_state_dir: state_dir,
                    logs_dir,
                    log_file_name,
                    cache_dir,
                    tui_config_dir,
                    tui_state_dir,
                }
            }
        }
    }

    pub fn home_config_dir(&self) -> &Path {
        &self.home_config_dir
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    pub fn secure_state_dir(&self) -> &Path {
        &self.secure_state_dir
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn log_file_name(&self) -> &str {
        &self.log_file_name
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn tui_config_dir(&self) -> &Path {
        &self.tui_config_dir
    }

    pub fn tui_state_dir(&self) -> &Path {
        &self.tui_state_dir
    }
}

impl LegacyPlatform {
    fn current() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            Some(Self::Linux)
        } else if cfg!(target_os = "windows") {
            Some(Self::Windows)
        } else {
            None
        }
    }
}

fn home_config_dir_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable | Channel::Preview => ".warp",
        Channel::Oss => ".warp-oss",
        Channel::Dev => ".warp-dev",
        Channel::Integration => ".warp-integration",
        Channel::Local => ".warp-local",
    }
}

fn macos_config_dir_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => ".warp",
        Channel::Preview => ".warp-preview",
        Channel::Oss => ".warp-oss",
        Channel::Dev => ".warp-dev",
        Channel::Integration => ".warp-integration",
        Channel::Local => ".warp-local",
    }
}

fn macos_tui_config_dir_name(channel: Channel) -> String {
    macos_config_dir_name(channel).replacen(".warp", ".warp_cli", 1)
}

fn legacy_log_file_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => "warp.log",
        Channel::Preview => "warp-preview.log",
        Channel::Oss => "warp-oss.log",
        Channel::Dev => "warp-dev.log",
        Channel::Integration => "warp_integration.log",
        Channel::Local => "warp-local.log",
    }
}

fn windows_join(base: &Path, suffix: &str) -> PathBuf {
    let base = base
        .to_string_lossy()
        .trim_end_matches(['/', '\\'])
        .to_owned();
    PathBuf::from(format!(r"{base}\{suffix}"))
}
