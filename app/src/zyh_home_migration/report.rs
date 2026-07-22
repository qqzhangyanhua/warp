use serde::Serialize;

#[derive(Serialize)]
pub(super) struct MigrationReport {
    pub(super) manifest_version: u32,
    pub(super) entries: Vec<EntryReport>,
    pub(super) omitted_setting_keys: Vec<String>,
    pub(super) unknown_setting_keys: Vec<String>,
    pub(super) skipped_paths: Vec<String>,
    pub(super) secure_storage: Vec<SecretReport>,
}

#[derive(Serialize)]
pub(super) struct EntryReport {
    pub(super) id: &'static str,
    pub(super) status: EntryStatus,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EntryStatus {
    Copied,
    CopiedAndCleaned,
    Malformed,
    Missing,
    SkippedSymlink,
    SkippedUnsupported,
    Translated,
}

#[derive(Serialize)]
pub(super) struct SecretReport {
    pub(super) key: &'static str,
    pub(super) status: SecretStatus,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SecretStatus {
    CopiedAndVerified,
    Missing,
}

#[derive(Serialize)]
pub(super) struct MigrationMarker {
    pub(super) manifest_version: u32,
    pub(super) complete: bool,
}
