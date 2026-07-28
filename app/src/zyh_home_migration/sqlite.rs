//! Forward-only cleanup of a **copied** ZYH SQLite database.
//!
//! The legacy database is never modified. Migration:
//! 1. Verified online backup into the destination file
//! 2. Diesel forward migrations on the copy
//! 3. Export supported local MCP installations to `.mcp.json` (+ secret map)
//! 4. Delete cloud-only tables/rows in one transaction
//! 5. Integrity and foreign-key verification
//!
//! # Retained (must stay readable)
//! - Terminal history: `commands`, `blocks`
//! - Window / tab restoration: `app`, `windows`, `tabs`, `tab_groups`, `panels`,
//!   `pane_nodes`, `pane_leaves`, `pane_branches`, `terminal_panes`,
//!   `code_panes`, `code_pane_tabs`, `code_review_panes`, `settings_panes`
//! - Local Conversations and Pi control: `agent_conversations`,
//!   `agent_runtime_runs`, `agent_tool_execution_records`, `agent_tasks`,
//!   `ai_queries`
//! - Local project metadata: `projects`, `ignored_suggestions`
//!
//! # Deleted (cloud-only product state)
//! See [`DELETED_CLOUD_TABLES`] and the nulling/rewrite statements in
//! [`CLEANUP_SQL`].

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, ptr, thread};

use anyhow::{anyhow, bail, Context as _, Result};
use diesel::connection::SimpleConnection as _;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use libsqlite3_sys as ffi;
use serde_json::{Map, Value};
use warpui_extras::owner_only_file::{content_hash, ensure_owner_only_file, ExpectedContent};

use crate::ai::mcp::local_mcp_config::{LocalMcpConfigDocument, ZYH_MCP_SECRETS_STORAGE_KEY};
use crate::ai::mcp::parsing::resolve_json;
use crate::ai::mcp::templatable_installation::TemplatableMCPServerInstallation;
use crate::ai::mcp::TemplatableMCPServer;

use super::{MigrationSecretError, MigrationSecretStore};

/// Cloud-only tables removed from the copied database.
///
/// Keep this list in sync with [`CLEANUP_SQL`]. Tables that may be absent on
/// very old schemas are deleted with defensive SQL that ignores missing tables.
pub const DELETED_CLOUD_TABLES: &[&str] = &[
    "active_mcp_servers",
    "ai_document_panes",
    "ai_memory_panes",
    "ambient_agent_panes",
    "cloud_objects_refreshes",
    "current_user_information",
    "env_var_collection_panes",
    "folders",
    "generic_string_objects",
    "mcp_environment_variables",
    "mcp_server_installations",
    "mcp_server_panes",
    "notebook_panes",
    "notebooks",
    "object_actions",
    "object_permissions",
    "object_metadata",
    "project_rules",
    "server_experiments",
    "team_members",
    "team_settings",
    "teams",
    "user_profiles",
    "users",
    "workflow_panes",
    "workflows",
    "workspace_teams",
    "workspaces",
];

/// Local history / control tables that must not appear in [`DELETED_CLOUD_TABLES`].
#[cfg(test)]
pub const RETAINED_LOCAL_TABLES: &[&str] = &[
    "agent_conversations",
    "agent_runtime_runs",
    "agent_tasks",
    "agent_tool_execution_records",
    "ai_queries",
    "app",
    "blocks",
    "code_pane_tabs",
    "code_panes",
    "code_review_panes",
    "commands",
    "ignored_suggestions",
    "pane_branches",
    "pane_leaves",
    "pane_nodes",
    "panels",
    "projects",
    "settings_panes",
    "tab_groups",
    "tabs",
    "terminal_panes",
    "windows",
];

const CLEANUP_SQL: &str = r#"
PRAGMA foreign_keys = OFF;
BEGIN IMMEDIATE;
DELETE FROM active_mcp_servers;
DELETE FROM ai_document_panes;
DELETE FROM ai_memory_panes;
DELETE FROM ambient_agent_panes;
DELETE FROM cloud_objects_refreshes;
DELETE FROM current_user_information;
DELETE FROM env_var_collection_panes;
DELETE FROM folders;
DELETE FROM generic_string_objects;
DELETE FROM mcp_environment_variables;
DELETE FROM mcp_server_installations;
DELETE FROM mcp_server_panes;
DELETE FROM notebook_panes;
DELETE FROM notebooks;
DELETE FROM object_actions;
DELETE FROM object_permissions;
DELETE FROM object_metadata;
DELETE FROM project_rules;
DELETE FROM server_experiments;
DELETE FROM team_members;
DELETE FROM team_settings;
DELETE FROM teams;
DELETE FROM user_profiles;
DELETE FROM users;
DELETE FROM workflow_panes;
DELETE FROM workflows;
DELETE FROM workspace_teams;
DELETE FROM workspaces;
UPDATE commands SET cloud_workflow_id = NULL;
UPDATE windows SET warp_drive_index_width = NULL, agent_management_filters = NULL;
UPDATE settings_panes
SET current_page = 'Appearance'
WHERE current_page IN (
    'Account',
    'Billing and usage',
    'Referrals',
    'Shared blocks',
    'Teams',
    'WarpDrive',
    'Warp Drive',
    'ZYH Drive',
    'CloudEnvironments',
    'Oz Cloud API Keys',
    'OzCloudAPIKeys'
);
COMMIT;
PRAGMA foreign_keys = ON;
"#;

/// Result of exporting local MCP installations out of SQLite.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct McpExportResult {
    pub exported_servers: usize,
    pub skipped_gallery_or_unsupported: usize,
    pub secrets: HashMap<String, String>,
    pub mcp_json_path: Option<PathBuf>,
}

pub(super) fn migrate_sqlite(
    source: &Path,
    destination: &Path,
    secrets: Option<(&dyn MigrationSecretStore, &Path)>,
) -> Result<()> {
    unsafe {
        crate::persistence::init_sqlite_logging();
    }
    backup_database(source, destination)?;
    ensure_owner_only_file(destination)?;

    let mut connection = crate::persistence::setup_database(destination)
        .context("running destination database migrations")?;

    let mcp_path = mcp_json_path_for_sqlite_destination(destination);
    let export = export_local_mcp_installations(&mut connection, mcp_path.as_deref())
        .context("exporting local MCP installations before cloud row deletion")?;
    if let (Some((store, staging_root)), true) = (secrets, !export.secrets.is_empty()) {
        write_exported_mcp_secrets(store, staging_root, &export.secrets)
            .context("writing exported MCP secrets to secure storage")?;
    }

    connection
        .batch_execute(CLEANUP_SQL)
        .context("cleaning non-local destination database rows")?;
    verify_database(&mut connection)?;
    verify_cloud_tables_empty(&mut connection)?;
    connection
        .batch_execute("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
        .context("checkpointing the migrated destination database")?;
    drop(connection);
    ensure_owner_only_file(destination)?;
    Ok(())
}

fn write_exported_mcp_secrets(
    store: &dyn MigrationSecretStore,
    staging_root: &Path,
    secrets: &HashMap<String, String>,
) -> Result<()> {
    let existing = store
        .read_destination(ZYH_MCP_SECRETS_STORAGE_KEY, staging_root)
        .map_err(|e| anyhow!("reading existing MCP secrets: {e}"))?;
    let mut map: HashMap<String, String> = existing
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    for (key, value) in secrets {
        map.insert(key.clone(), value.clone());
    }
    let serialized = serde_json::to_string(&map).context("serializing MCP secrets map")?;
    store
        .write_destination(ZYH_MCP_SECRETS_STORAGE_KEY, &serialized, staging_root)
        .map_err(|e| match e {
            MigrationSecretError::Unavailable => {
                anyhow!("secure storage unavailable for MCP secret export")
            }
        })?;
    Ok(())
}

fn mcp_json_path_for_sqlite_destination(sqlite_destination: &Path) -> Option<PathBuf> {
    let parent = sqlite_destination.parent()?;
    if parent.file_name().and_then(|s| s.to_str()) == Some("tui") {
        parent.parent().map(|home| home.join(".mcp.json"))
    } else {
        Some(parent.join(".mcp.json"))
    }
}

/// Export non-gallery MCP installations to local `.mcp.json` before SQLite rows
/// are deleted. Literal secret env/header values become placeholders; raw values
/// are returned in [`McpExportResult::secrets`] for secure-storage write by the
/// caller when available. Failures to parse individual rows skip that server
/// rather than aborting the whole migration.
fn export_local_mcp_installations(
    connection: &mut SqliteConnection,
    mcp_json_path: Option<&Path>,
) -> Result<McpExportResult> {
    let Some(mcp_json_path) = mcp_json_path else {
        return Ok(McpExportResult::default());
    };

    let installations = load_mcp_installations(connection)?;
    let mut servers = Map::new();
    let mut skipped = 0usize;
    let mut secrets = HashMap::new();

    for installation in installations {
        if installation.templatable_mcp_server().gallery_data.is_some() {
            skipped += 1;
            continue;
        }
        let rendered = resolve_json(&installation);
        let Ok(value) = serde_json::from_str::<Value>(&rendered) else {
            skipped += 1;
            continue;
        };
        if !is_exportable_local_server(&value) {
            skipped += 1;
            continue;
        }
        let name = sanitize_server_name(&installation.templatable_mcp_server().name);
        if name.is_empty() {
            skipped += 1;
            continue;
        }
        servers.insert(name, value);
    }

    if servers.is_empty() {
        return Ok(McpExportResult {
            exported_servers: 0,
            skipped_gallery_or_unsupported: skipped,
            secrets,
            mcp_json_path: Some(mcp_json_path.to_path_buf()),
        });
    }

    if let Some(parent) = mcp_json_path.parent() {
        fs::create_dir_all(parent).context("creating parent for .mcp.json")?;
    }

    let document = LocalMcpConfigDocument::with_path(mcp_json_path);
    let expected = match content_hash(mcp_json_path).context("hashing existing .mcp.json")? {
        None => ExpectedContent::Missing,
        Some(hash) => ExpectedContent::Hash(hash),
    };
    let exported_count = servers.len();
    let (_, extracted) = document
        .upsert_servers(servers, expected)
        .context("writing exported MCP servers to .mcp.json")?;
    for secret in extracted {
        secrets.insert(secret.storage_key(), secret.value);
    }

    Ok(McpExportResult {
        exported_servers: exported_count,
        skipped_gallery_or_unsupported: skipped,
        secrets,
        mcp_json_path: Some(mcp_json_path.to_path_buf()),
    })
}

fn load_mcp_installations(
    connection: &mut SqliteConnection,
) -> Result<Vec<TemplatableMCPServerInstallation>> {
    #[derive(QueryableByName)]
    struct McpRow {
        #[diesel(sql_type = Text)]
        id: String,
        #[diesel(sql_type = Text)]
        templatable_mcp_server: String,
        #[diesel(sql_type = Text)]
        variable_values: String,
    }

    let rows: Vec<McpRow> = diesel::sql_query(
        "SELECT id, templatable_mcp_server, variable_values FROM mcp_server_installations",
    )
    .load(connection)
    .context("querying mcp_server_installations")?;

    let mut out = Vec::new();
    for row in rows {
        let Ok(uuid) = uuid::Uuid::parse_str(&row.id) else {
            continue;
        };
        let Ok(server) = serde_json::from_str::<TemplatableMCPServer>(&row.templatable_mcp_server)
        else {
            continue;
        };
        let vars = serde_json::from_str(&row.variable_values).unwrap_or_default();
        out.push(TemplatableMCPServerInstallation::new(uuid, server, vars));
    }
    Ok(out)
}

fn is_exportable_local_server(value: &Value) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    // stdio command server
    if obj.get("command").and_then(Value::as_str).is_some() {
        return true;
    }
    // remote URL server (user-configured, not gallery-managed)
    if obj.get("url").and_then(Value::as_str).is_some() {
        return true;
    }
    false
}

fn sanitize_server_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn backup_database(source: &Path, destination: &Path) -> Result<()> {
    let source = RawConnection::open(source, ffi::SQLITE_OPEN_READONLY)?;
    let destination = RawConnection::open(
        destination,
        ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE,
    )?;
    let main = c"main";

    let backup =
        unsafe { ffi::sqlite3_backup_init(destination.0, main.as_ptr(), source.0, main.as_ptr()) };
    if backup.is_null() {
        bail!(
            "could not initialize SQLite backup: {}",
            destination.error()
        );
    }

    let mut status;
    let mut retries = 0;
    loop {
        status = unsafe { ffi::sqlite3_backup_step(backup, -1) };
        if status == ffi::SQLITE_DONE {
            break;
        }
        if matches!(status, ffi::SQLITE_BUSY | ffi::SQLITE_LOCKED) && retries < 100 {
            retries += 1;
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        break;
    }

    let finish_status = unsafe { ffi::sqlite3_backup_finish(backup) };
    if status != ffi::SQLITE_DONE || finish_status != ffi::SQLITE_OK {
        bail!(
            "SQLite backup failed with status {status}/{finish_status}: {}",
            destination.error()
        );
    }
    Ok(())
}

fn verify_database(connection: &mut SqliteConnection) -> Result<()> {
    let integrity = diesel::sql_query("PRAGMA integrity_check")
        .load::<IntegrityCheck>(connection)
        .context("running SQLite integrity_check")?;
    if integrity.len() != 1 || integrity[0].integrity_check != "ok" {
        bail!("SQLite integrity_check failed");
    }

    let foreign_key_violations = diesel::sql_query("PRAGMA foreign_key_check")
        .load::<ForeignKeyViolation>(connection)
        .context("running SQLite foreign_key_check")?;
    if !foreign_key_violations.is_empty() {
        bail!("SQLite foreign_key_check failed");
    }
    Ok(())
}

fn verify_cloud_tables_empty(connection: &mut SqliteConnection) -> Result<()> {
    for table in DELETED_CLOUD_TABLES {
        let count = count_rows(connection, table)?;
        if count != 0 {
            bail!("cloud table {table} still has {count} rows after cleanup");
        }
    }
    Ok(())
}

fn count_rows(connection: &mut SqliteConnection, table: &str) -> Result<i64> {
    // Table names come only from the static classification lists.
    #[derive(QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }
    let sql = format!("SELECT COUNT(*) AS count FROM {table}");
    let row: CountRow = diesel::sql_query(sql)
        .get_result(connection)
        .with_context(|| format!("counting rows in {table}"))?;
    Ok(row.count)
}

struct RawConnection(*mut ffi::sqlite3);

impl RawConnection {
    fn open(path: &Path, flags: i32) -> Result<Self> {
        let path = path
            .to_str()
            .ok_or_else(|| anyhow!("SQLite path is not valid UTF-8"))?;
        let path = CString::new(path).context("SQLite path contains a NUL byte")?;
        let mut connection = ptr::null_mut();
        let status =
            unsafe { ffi::sqlite3_open_v2(path.as_ptr(), &mut connection, flags, ptr::null()) };
        let connection = Self(connection);
        if status != ffi::SQLITE_OK {
            bail!("could not open SQLite database: {}", connection.error());
        }
        Ok(connection)
    }

    fn error(&self) -> String {
        if self.0.is_null() {
            return "unknown SQLite error".to_owned();
        }
        unsafe { CStr::from_ptr(ffi::sqlite3_errmsg(self.0)) }
            .to_string_lossy()
            .into_owned()
    }
}

impl Drop for RawConnection {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                ffi::sqlite3_close(self.0);
            }
        }
    }
}

#[derive(QueryableByName)]
struct IntegrityCheck {
    #[diesel(sql_type = Text)]
    integrity_check: String,
}

#[allow(dead_code)]
#[derive(QueryableByName)]
struct ForeignKeyViolation {
    #[diesel(sql_type = Text, column_name = "table")]
    table_name: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    rowid: Option<i64>,
    #[diesel(sql_type = Text)]
    parent: String,
    #[diesel(sql_type = Integer)]
    fkid: i32,
}

#[cfg(test)]
#[path = "sqlite_tests.rs"]
mod tests;
