use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Context as _, Result};
use diesel::connection::SimpleConnection as _;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use libsqlite3_sys as ffi;
use warpui_extras::owner_only_file::ensure_owner_only_file;

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

pub(super) fn migrate_sqlite(source: &Path, destination: &Path) -> Result<()> {
    unsafe {
        crate::persistence::init_sqlite_logging();
    }
    backup_database(source, destination)?;
    ensure_owner_only_file(destination)?;

    let mut connection = crate::persistence::setup_database(destination)
        .context("running destination database migrations")?;
    connection
        .batch_execute(CLEANUP_SQL)
        .context("cleaning non-local destination database rows")?;
    verify_database(&mut connection)?;
    connection
        .batch_execute("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
        .context("checkpointing the migrated destination database")?;
    drop(connection);
    ensure_owner_only_file(destination)?;
    Ok(())
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
