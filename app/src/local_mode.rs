use uuid::Uuid;
use warp_core::features::FeatureFlag;
use warp_core::user_preferences::GetUserPreferences as _;
use warpui::AppContext;

const LOCAL_IDENTITY_KEY: &str = "LocalOnlyIdentityId";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalIdentity {
    id: Uuid,
}

impl LocalIdentity {
    pub fn as_uid(&self) -> String {
        format!("local:{}", self.id)
    }
}

pub fn is_local_only_custom_provider_mode() -> bool {
    FeatureFlag::LocalOnlyCustomProviderMode.is_enabled()
}

pub fn account_sign_in_unavailable_message() -> &'static str {
    "This build only supports Local-only Mode; Account Sign-in is unavailable"
}

pub fn account_logout_unavailable_message() -> &'static str {
    "This build only supports Local-only Mode; account logout is unavailable"
}

pub fn get_or_create_local_identity(ctx: &AppContext) -> anyhow::Result<LocalIdentity> {
    let prefs = ctx.private_user_preferences();
    if let Some(id) = prefs
        .read_value(LOCAL_IDENTITY_KEY)
        .map_err(|err| anyhow::anyhow!("Failed to read Local-only identity: {err}"))?
        .and_then(|stored| Uuid::parse_str(&stored).ok())
    {
        return Ok(LocalIdentity { id });
    }

    let id = Uuid::new_v4();
    prefs
        .write_value(LOCAL_IDENTITY_KEY, id.to_string())
        .map_err(|err| anyhow::anyhow!("Failed to persist Local-only identity: {err}"))?;
    Ok(LocalIdentity { id })
}

#[cfg(any(test, feature = "test-util"))]
pub fn local_identity_for_test(id: Uuid) -> LocalIdentity {
    LocalIdentity { id }
}

#[cfg(test)]
#[path = "local_mode_tests.rs"]
mod tests;
