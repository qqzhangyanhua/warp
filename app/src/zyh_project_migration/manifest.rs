#[derive(Clone, Copy)]
pub(super) enum ManifestEntryKind {
    Directory,
    SanitizedMcp,
}

#[derive(Clone, Copy)]
pub(super) struct ManifestEntry {
    pub(super) path: &'static str,
    pub(super) kind: ManifestEntryKind,
}

pub(super) const MANIFEST_VERSION: u32 = 1;

pub(super) const PROJECT_MIGRATION_MANIFEST: &[ManifestEntry] = &[
    ManifestEntry {
        path: "workflows",
        kind: ManifestEntryKind::Directory,
    },
    ManifestEntry {
        path: "launch_configurations",
        kind: ManifestEntryKind::Directory,
    },
    ManifestEntry {
        path: "tab_configs",
        kind: ManifestEntryKind::Directory,
    },
    ManifestEntry {
        path: "themes",
        kind: ManifestEntryKind::Directory,
    },
    ManifestEntry {
        path: "skills",
        kind: ManifestEntryKind::Directory,
    },
    ManifestEntry {
        path: "plugins",
        kind: ManifestEntryKind::Directory,
    },
    ManifestEntry {
        path: ".mcp.json",
        kind: ManifestEntryKind::SanitizedMcp,
    },
];
