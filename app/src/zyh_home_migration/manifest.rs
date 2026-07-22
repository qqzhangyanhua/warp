#[derive(Clone, Copy)]
pub(super) enum LegacyRoot {
    HomeConfig,
    Config,
    Data,
    State,
    TuiConfig,
    TuiState,
}

#[derive(Clone, Copy)]
pub(super) enum EntryKind {
    File,
    Directory,
    Settings { backup_name: &'static str },
    Sqlite,
}

#[derive(Clone, Copy)]
pub(super) struct ManifestEntry {
    pub(super) id: &'static str,
    pub(super) root: LegacyRoot,
    pub(super) source: &'static str,
    pub(super) destination: &'static str,
    pub(super) kind: EntryKind,
}

pub(super) const MANIFEST_VERSION: u32 = 1;

pub(super) const MIGRATION_MANIFEST: &[ManifestEntry] = &[
    ManifestEntry {
        id: "settings",
        root: LegacyRoot::Config,
        source: "settings.toml",
        destination: "settings.toml",
        kind: EntryKind::Settings {
            backup_name: "settings.toml.legacy",
        },
    },
    ManifestEntry {
        id: "keybindings",
        root: LegacyRoot::Config,
        source: "keybindings.yaml",
        destination: "keybindings.yaml",
        kind: EntryKind::File,
    },
    ManifestEntry {
        id: "themes",
        root: LegacyRoot::Data,
        source: "themes",
        destination: "themes",
        kind: EntryKind::Directory,
    },
    ManifestEntry {
        id: "workflows",
        root: LegacyRoot::Data,
        source: "workflows",
        destination: "workflows",
        kind: EntryKind::Directory,
    },
    ManifestEntry {
        id: "mcp",
        root: LegacyRoot::HomeConfig,
        source: ".mcp.json",
        destination: ".mcp.json",
        kind: EntryKind::File,
    },
    ManifestEntry {
        id: "tab_configs",
        root: LegacyRoot::Data,
        source: "tab_configs",
        destination: "tab_configs",
        kind: EntryKind::Directory,
    },
    ManifestEntry {
        id: "launch_configs",
        root: LegacyRoot::Data,
        source: "launch_configurations",
        destination: "launch_configurations",
        kind: EntryKind::Directory,
    },
    ManifestEntry {
        id: "skills",
        root: LegacyRoot::HomeConfig,
        source: "skills",
        destination: "skills",
        kind: EntryKind::Directory,
    },
    ManifestEntry {
        id: "plugins",
        root: LegacyRoot::HomeConfig,
        source: "plugins",
        destination: "plugins",
        kind: EntryKind::Directory,
    },
    ManifestEntry {
        id: "ssh_hosts",
        root: LegacyRoot::Data,
        source: "ssh_hosts.json",
        destination: "ssh_hosts.json",
        kind: EntryKind::File,
    },
    ManifestEntry {
        id: "gui_sqlite",
        root: LegacyRoot::State,
        source: "warp.sqlite",
        destination: "warp.sqlite",
        kind: EntryKind::Sqlite,
    },
    ManifestEntry {
        id: "logs",
        root: LegacyRoot::State,
        source: "logs",
        destination: "logs",
        kind: EntryKind::Directory,
    },
    ManifestEntry {
        id: "tui_settings",
        root: LegacyRoot::TuiConfig,
        source: "settings.toml",
        destination: "tui/settings.toml",
        kind: EntryKind::Settings {
            backup_name: "tui-settings.toml.legacy",
        },
    },
    ManifestEntry {
        id: "tui_sqlite",
        root: LegacyRoot::TuiState,
        source: "warp.sqlite",
        destination: "tui/warp.sqlite",
        kind: EntryKind::Sqlite,
    },
];
