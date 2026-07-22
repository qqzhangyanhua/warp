use dirs::home_dir;

use super::*;

#[test]
fn zyh_home_profiles_use_one_home_relative_root() {
    let home = Path::new("/users/tester");

    let production = AppHome::resolve(home, AppHomeProfile::Production, None).unwrap();
    assert_eq!(production.root(), home.join(".zyh"));
    assert_eq!(production.config_dir(), production.root());
    assert_eq!(production.data_dir(), production.root());
    assert_eq!(production.state_dir(), production.root());
    assert_eq!(production.cache_dir(), production.root().join("cache"));
    assert_eq!(production.logs_dir(), production.root().join("logs"));
    assert_eq!(
        production.gui_database_file(),
        production.root().join("warp.sqlite")
    );
    assert_eq!(
        production.tui_database_file(),
        production.root().join("tui").join("warp.sqlite")
    );

    let development = AppHome::resolve(home, AppHomeProfile::Development, None).unwrap();
    assert_eq!(development.root(), home.join(".zyh-dev"));
}

#[test]
fn integration_home_requires_an_explicit_isolated_root() {
    let home = Path::new("/users/tester");
    assert_eq!(
        AppHome::resolve(home, AppHomeProfile::Integration, None),
        Err(AppHomeError::MissingIntegrationRoot)
    );

    let test_root = Path::new("/tmp/zyh-integration/test-case");
    let integration = AppHome::resolve(home, AppHomeProfile::Integration, Some(test_root)).unwrap();
    assert_eq!(integration.root(), test_root);
    assert!(!integration.root().starts_with(home));
}

#[test]
fn zyh_home_paths_are_platform_independent() {
    let unix =
        AppHome::resolve(Path::new("/Users/tester"), AppHomeProfile::Production, None).unwrap();
    let windows = AppHome::resolve(
        Path::new(r"C:\Users\tester"),
        AppHomeProfile::Production,
        None,
    )
    .unwrap();

    assert_eq!(unix.root(), Path::new("/Users/tester/.zyh"));
    assert_eq!(windows.root(), Path::new(r"C:\Users\tester").join(".zyh"));
}

#[test]
fn legacy_roots_match_each_platforms_existing_layout() {
    let mac = LegacyRoots::resolve(
        Path::new("/Users/tester"),
        LegacyPlatform::MacOs,
        LegacyIdentity::new(Channel::Stable, "dev.warp.Warp"),
    );
    assert_eq!(mac.home_config_dir(), Path::new("/Users/tester/.warp"));
    assert_eq!(mac.config_dir(), Path::new("/Users/tester/.warp"));
    assert_eq!(mac.data_dir(), Path::new("/Users/tester/.warp"));
    assert_eq!(
        mac.state_dir(),
        Path::new("/Users/tester/Library/Application Support/dev.warp.Warp")
    );
    assert_eq!(mac.tui_config_dir(), Path::new("/Users/tester/.warp_cli"));

    let linux = LegacyRoots::resolve(
        Path::new("/home/tester"),
        LegacyPlatform::Linux,
        LegacyIdentity::new(Channel::Oss, "warp-oss"),
    );
    assert_eq!(linux.home_config_dir(), Path::new("/home/tester/.warp-oss"));
    assert_eq!(
        linux.config_dir(),
        Path::new("/home/tester/.config/warp-oss")
    );
    assert_eq!(
        linux.data_dir(),
        Path::new("/home/tester/.local/share/warp-oss")
    );
    assert_eq!(
        linux.state_dir(),
        Path::new("/home/tester/.local/state/warp-oss")
    );
    assert_eq!(linux.tui_config_dir(), linux.config_dir().join("cli"));
    assert_eq!(linux.tui_state_dir(), linux.state_dir().join("tui"));

    let windows = LegacyRoots::resolve(
        Path::new(r"C:\Users\tester"),
        LegacyPlatform::Windows,
        LegacyIdentity::new(Channel::Oss, r"warp\WarpOss"),
    );
    assert_eq!(
        windows.config_dir(),
        Path::new(r"C:\Users\tester\AppData\Local\warp\WarpOss\config")
    );
    assert_eq!(
        windows.data_dir(),
        Path::new(r"C:\Users\tester\AppData\Roaming\warp\WarpOss\data")
    );
    assert_eq!(
        windows.state_dir(),
        Path::new(r"C:\Users\tester\AppData\Local\warp\WarpOss\data")
    );
}

#[test]
fn test_data_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    assert_eq!(data_dir(), home_dir.join(".zyh-dev"));
}

#[test]
fn test_config_local_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    assert_eq!(config_local_dir(), home_dir.join(".zyh-dev"));
}

#[test]
fn test_warp_home_config_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    assert_eq!(warp_home_config_dir(), Some(home_dir.join(".zyh-dev")));
}

#[test]
fn test_warp_home_skills_and_mcp_paths() {
    let Some(config_dir) = warp_home_config_dir() else {
        panic!("Should be able to compute Warp home config directory");
    };

    assert_eq!(warp_home_skills_dir(), Some(config_dir.join("skills")));
    assert_eq!(
        warp_home_mcp_config_file_path(),
        Some(config_dir.join(".mcp.json"))
    );
}
#[test]
fn test_cache_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    assert_eq!(cache_dir(), home_dir.join(".zyh-dev/cache"));
}

#[test]
fn test_state_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    assert_eq!(state_dir(), home_dir.join(".zyh-dev"));
    assert_eq!(secure_state_dir(), Some(home_dir.join(".zyh-dev")));
}

#[test]
fn test_tui_state_dir_is_tui_subdir_of_gui_state_base() {
    let tui_dir = tui_state_dir();
    assert_eq!(tui_dir.file_name(), Some(std::ffi::OsStr::new("tui")));

    // The TUI state dir must be a direct `tui` child of the same base
    // directory that holds the GUI's SQLite database (the secure state dir
    // when available, otherwise the plain state dir), so the two front-ends
    // keep sibling — never shared — databases.
    let gui_state_base = secure_state_dir().unwrap_or_else(state_dir);
    assert_eq!(tui_dir.parent(), Some(gui_state_base.as_path()));
}

#[test]
fn test_project_path_for_warp_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "warp", "Warp"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.warp.Warp");
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(project_dirs.project_path(), "warp-terminal");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "warp\\Warp");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_warp_dev_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "warp", "WarpDev"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.warp.WarpDev");
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(project_dirs.project_path(), "warp-terminal-dev");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "warp\\WarpDev");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_oss_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "warp", "WarpOss"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.warp.WarpOss");
        } else if #[cfg(any(target_os = "linux", target_os = "freebsd"))] {
            assert_eq!(project_dirs.project_path(), "warp-oss");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "warp\\WarpOss");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}
