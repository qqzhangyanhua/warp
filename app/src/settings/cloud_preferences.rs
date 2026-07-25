//! Transitional cloud Preference object adapters.
//!
//! Settings live only in local `settings.toml` (settings sync and
//! CloudPreferencesSyncer are gone). [`Preference`] / [`CloudPreference`] remain
//! so legacy Preference GSOs can still hydrate in-memory/sqlite state. They do
//! not enqueue create or update queue items (#41 PR14).

pub use cloud_object_models::{CloudPreference, CloudPreferenceModel, Platform, Preference};

use crate::cloud_object::model::generic_string_model::StringModel;
use crate::cloud_object::model::json_model::JsonModel;
use crate::cloud_object::{
    GenericStringObjectFormat, GenericStringObjectUniqueKey, JsonObjectType, Revision, UniquePer,
};
use crate::server::sync_queue::QueueItem;

/// Residual Preference cloud objects: deserialize and store only; never sync out.
impl StringModel for Preference {
    type CloudObjectType = CloudPreference;

    fn model_type_name(&self) -> &'static str {
        "Preference"
    }

    fn should_enforce_revisions() -> bool {
        // Last write wins for legacy cloud prefs
        false
    }

    fn should_show_activity_toasts() -> bool {
        false
    }

    fn warn_if_unsaved_at_quit() -> bool {
        false
    }

    fn model_format() -> GenericStringObjectFormat {
        GenericStringObjectFormat::Json(Self::json_object_type())
    }

    fn display_name(&self) -> String {
        self.model_type_name().to_owned()
    }

    fn update_object_queue_item(
        &self,
        _revision_ts: Option<Revision>,
        _object: &CloudPreference,
    ) -> Option<QueueItem> {
        // Preferences are local-only; never enqueue server updates.
        None
    }

    fn enqueues_server_mutations() -> bool {
        false
    }

    fn should_clear_on_unique_key_conflict(&self) -> bool {
        true
    }

    fn uniqueness_key(&self) -> Option<GenericStringObjectUniqueKey> {
        Some(GenericStringObjectUniqueKey {
            key: format!("{}_{}", self.platform, self.storage_key),
            unique_per: UniquePer::User,
        })
    }
}

impl JsonModel for Preference {
    fn json_object_type() -> JsonObjectType {
        JsonObjectType::Preference
    }
}
