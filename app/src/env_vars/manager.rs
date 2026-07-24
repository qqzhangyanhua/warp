use std::collections::hash_map::Entry;
use std::collections::HashMap;

use warpui::{Entity, EntityId, ModelContext, SingletonEntity, WeakViewHandle};

use crate::cloud_object::Owner;
use crate::env_vars::view::env_var_collection::EnvVarCollectionView;
use crate::pane_group::{EnvVarCollectionPane, PaneContent};
use crate::server::ids::SyncId;
use crate::{safe_warn, PaneViewLocator, WindowId};

pub struct EnvVarCollectionManager {
    panes_by_hashed_id: HashMap<String, EnvVarCollectionPaneData>,
}

#[derive(Debug, Clone)]
pub enum EnvVarCollectionSource {
    Existing(SyncId),
    New {
        title: Option<String>,
        owner: Owner,
        initial_folder_id: Option<SyncId>,
    },
}

/// Manages EnvVarCollection panes
impl EnvVarCollectionManager {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        EnvVarCollectionManager {
            panes_by_hashed_id: HashMap::new(),
        }
    }

    /// If the collection is already open in a pane, finds the location of that pane.
    pub fn find_pane(
        &self,
        source: &EnvVarCollectionSource,
    ) -> Option<(WindowId, PaneViewLocator)> {
        match source {
            EnvVarCollectionSource::Existing(env_var_collection_id) => {
                let pane_data = self.panes_by_hashed_id.get(&env_var_collection_id.uid())?;
                Some((pane_data.window_id, pane_data.locator))
            }
            EnvVarCollectionSource::New { .. } => None,
        }
    }

    /// Create an EVC pane.
    ///
    /// Environment Variable Collections are removed from the product. This no
    /// longer loads CloudModel or opens an editable collection. Callers should
    /// check [`crate::env_vars::may_open_or_create_evc`] and show
    /// [`crate::env_vars::EVC_REMOVED_GUIDANCE`] instead of creating panes.
    pub fn create_pane(
        &mut self,
        source: &EnvVarCollectionSource,
        window_id: WindowId,
        ctx: &mut ModelContext<Self>,
    ) -> EnvVarCollectionPane {
        let _ = source;
        // Fail closed: empty view, no CloudModel / UpdateManager access.
        let view = ctx.add_typed_action_view(window_id, EnvVarCollectionView::new);
        safe_warn!(
            safe: ("Refusing to open Environment Variable Collection pane"),
            full: ("{}", crate::env_vars::EVC_REMOVED_GUIDANCE)
        );
        EnvVarCollectionPane::new(view, ctx)
    }

    /// Whether create/open is allowed for the product.
    pub fn is_supported() -> bool {
        crate::env_vars::may_open_or_create_evc()
    }

    pub fn register_pane(
        &mut self,
        pane: &EnvVarCollectionPane,
        pane_group_id: EntityId,
        window_id: WindowId,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(env_var_collection_id) = pane
            .env_var_collection_view(ctx)
            .as_ref(ctx)
            .env_var_collection_id(ctx)
        else {
            log::warn!("EnvVarCollection pane has no ID");
            return;
        };

        let entry = self.panes_by_hashed_id.entry(env_var_collection_id.uid());
        if let Entry::Vacant(entry) = entry {
            entry.insert(EnvVarCollectionPaneData {
                env_var_collection_id,
                window_id,
                locator: PaneViewLocator {
                    pane_group_id,
                    pane_id: pane.id(),
                },
                handle: pane.env_var_collection_view(ctx).downgrade(),
            });
        } else {
            safe_warn!(
                safe: ("Ignoring duplicate EnvVarCollection pane registration"),
                full: ("Ignoring duplicate EnvVarCollection pane registration for {env_var_collection_id}")
            );
        }
    }

    pub fn deregister_pane(&mut self, pane: &EnvVarCollectionPane, ctx: &mut ModelContext<Self>) {
        let Some(env_var_collection_id) = pane
            .env_var_collection_view(ctx)
            .as_ref(ctx)
            .env_var_collection_id(ctx)
        else {
            log::warn!("EnvVarCollection pane has no ID");
            return;
        };

        // If an EVC pane is restored, the EVC may have been reopened in the meantime. In
        // that case, don't let closing the original pane clear out the new pane.
        if let Entry::Occupied(entry) = self.panes_by_hashed_id.entry(env_var_collection_id.uid()) {
            if entry.get().locator.pane_id == pane.id() {
                entry.remove();
            } else {
                log::warn!(
                    "Ignoring duplicate registration of panes for {}",
                    env_var_collection_id.uid()
                );
            }
        }
    }

    pub fn reload_collection(
        &mut self,
        source: &EnvVarCollectionSource,
        _ctx: &mut ModelContext<Self>,
    ) {
        let _ = source;
        // EVC product removal: do not touch CloudModel for reloads.
        safe_warn!(
            safe: ("Ignoring Environment Variable Collection reload"),
            full: ("{}", crate::env_vars::EVC_REMOVED_GUIDANCE)
        );
    }

    pub fn reset(&mut self) {
        self.panes_by_hashed_id.clear();
    }
}

struct EnvVarCollectionPaneData {
    env_var_collection_id: SyncId,
    window_id: WindowId,
    handle: WeakViewHandle<EnvVarCollectionView>,
    locator: PaneViewLocator,
}

impl Entity for EnvVarCollectionManager {
    type Event = ();
}

impl SingletonEntity for EnvVarCollectionManager {}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
