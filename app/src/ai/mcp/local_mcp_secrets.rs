//! Secure-storage backed secrets for ZYH-managed MCP config.
//!
//! File MCP parsing runs off the UI thread without `AppContext`, so secrets are
//! refreshed into a process-wide cache whenever settings write them and when
//! the file watcher starts.

use std::collections::HashMap;
use std::io;
use std::sync::{LazyLock, Mutex};

use warpui_extras::secure_storage::AppContextExt as _;

use super::local_mcp_config::{LocalMcpConfigError, ZYH_MCP_SECRETS_STORAGE_KEY};

static ZYH_MCP_SECRETS_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Replace the process-wide ZYH MCP secrets cache.
pub fn set_zyh_mcp_secrets_cache(secrets: HashMap<String, String>) {
    if let Ok(mut guard) = ZYH_MCP_SECRETS_CACHE.lock() {
        *guard = secrets;
    }
}

/// Snapshot of the process-wide ZYH MCP secrets cache.
pub fn zyh_mcp_secrets_cache() -> HashMap<String, String> {
    ZYH_MCP_SECRETS_CACHE
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

/// Load ZYH MCP secrets from secure storage into the process-wide cache.
///
/// Skipped under `cfg(test)` because many unit harnesses do not register secure
/// storage (same pattern as `TemplatableMCPServerManager`).
pub fn load_zyh_mcp_secrets_into_cache(app: &mut warpui::AppContext) {
    if cfg!(test) {
        return;
    }
    let secrets = app
        .secure_storage()
        .read_value(ZYH_MCP_SECRETS_STORAGE_KEY)
        .ok()
        .and_then(|value| serde_json::from_str::<HashMap<String, String>>(&value).ok())
        .unwrap_or_default();
    set_zyh_mcp_secrets_cache(secrets);
}

/// Persist secrets to secure storage and refresh the process-wide cache.
pub fn persist_zyh_mcp_secrets(
    app: &mut warpui::AppContext,
    secrets: &HashMap<String, String>,
) -> Result<(), LocalMcpConfigError> {
    let json = serde_json::to_string(secrets)?;
    app.secure_storage()
        .write_value(ZYH_MCP_SECRETS_STORAGE_KEY, &json)
        .map_err(|err| {
            LocalMcpConfigError::Io(io::Error::other(format!(
                "failed to write MCP secrets to secure storage: {err}"
            )))
        })?;
    set_zyh_mcp_secrets_cache(secrets.clone());
    Ok(())
}
