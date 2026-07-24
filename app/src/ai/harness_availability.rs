use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use warp_cli::agent::Harness;
use warp_core::features::FeatureFlag;
use warp_managed_secrets::client::SecretOwner;
use warp_managed_secrets::ManagedSecretValue;
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::harness_display;

const HOSTED_AUTH_SECRETS_UNAVAILABLE: &str = "Hosted harness auth secrets are unavailable in ZYH";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarnessModelInfo {
    pub id: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_level: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HarnessAvailability {
    pub harness: Harness,
    pub display_name: String,
    pub enabled: bool,
    #[serde(default)]
    pub available_models: Vec<HarnessModelInfo>,
}

fn default_harnesses() -> Vec<HarnessAvailability> {
    vec![HarnessAvailability {
        harness: Harness::Oz,
        display_name: "ZYH".to_string(),
        enabled: true,
        available_models: vec![],
    }]
}

#[derive(Debug, Clone)]
pub enum AuthSecretFetchState {
    NotFetched,
    Loading,
    Loaded(Vec<AuthSecretEntry>),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct AuthSecretEntry {
    pub name: String,
    pub owner: SecretOwner,
}

pub enum HarnessAvailabilityEvent {
    Changed,
    AuthSecretsLoaded,
    AuthSecretsFetchFailed,
    AuthSecretCreated {
        harness: Harness,
        name: String,
    },
    AuthSecretCreationFailed {
        error: String,
    },
    AuthSecretDeleted {
        harness: Harness,
        name: String,
        owner: SecretOwner,
    },
    AuthSecretDeletionFailed {
        harness: Harness,
        name: String,
        owner: SecretOwner,
        error: String,
    },
}

pub struct HarnessAvailabilityModel {
    harnesses: Vec<HarnessAvailability>,
    auth_secrets: HashMap<Harness, AuthSecretFetchState>,
}

impl HarnessAvailabilityModel {
    pub fn new_local(_ctx: &mut ModelContext<Self>) -> Self {
        Self {
            harnesses: default_harnesses(),
            auth_secrets: HashMap::new(),
        }
    }

    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        Self::new_local(ctx)
    }

    pub fn available_harnesses(&self) -> &[HarnessAvailability] {
        &self.harnesses
    }

    pub fn display_name_for(&self, harness: Harness) -> &str {
        self.harnesses
            .iter()
            .find(|availability| availability.harness == harness)
            .map(|availability| availability.display_name.as_str())
            .unwrap_or_else(|| harness_display::display_name(harness))
    }

    pub fn should_show_harness_selector(&self) -> bool {
        FeatureFlag::AgentHarness.is_enabled() && self.harnesses.len() > 1
    }

    pub fn has_any_enabled_harness(&self) -> bool {
        self.harnesses.iter().any(|harness| harness.enabled)
    }

    pub fn is_harness_enabled(&self, harness: Harness) -> bool {
        self.harnesses
            .iter()
            .any(|availability| availability.harness == harness && availability.enabled)
    }

    pub fn models_for(&self, harness: Harness) -> Option<&[HarnessModelInfo]> {
        self.harnesses
            .iter()
            .find(|availability| availability.harness == harness)
            .map(|availability| availability.available_models.as_slice())
            .filter(|models| !models.is_empty())
    }

    pub fn auth_secrets_for(&self, harness: Harness) -> &AuthSecretFetchState {
        self.auth_secrets
            .get(&harness)
            .unwrap_or(&AuthSecretFetchState::NotFetched)
    }

    pub fn ensure_auth_secrets_fetched(&mut self, harness: Harness, ctx: &mut ModelContext<Self>) {
        if matches!(
            self.auth_secrets_for(harness),
            AuthSecretFetchState::NotFetched | AuthSecretFetchState::Failed(_)
        ) {
            self.auth_secrets
                .insert(harness, AuthSecretFetchState::Loaded(Vec::new()));
            ctx.emit(HarnessAvailabilityEvent::AuthSecretsLoaded);
        }
    }

    pub fn invalidate_auth_secrets(&mut self, harness: Harness) {
        self.auth_secrets.remove(&harness);
    }

    pub fn create_auth_secret(
        &mut self,
        harness: Harness,
        name: String,
        value: ManagedSecretValue,
        owner: SecretOwner,
        ctx: &mut ModelContext<Self>,
    ) {
        drop((harness, name, value, owner));
        ctx.emit(HarnessAvailabilityEvent::AuthSecretCreationFailed {
            error: HOSTED_AUTH_SECRETS_UNAVAILABLE.to_string(),
        });
    }

    pub fn delete_auth_secret(
        &mut self,
        harness: Harness,
        name: String,
        owner: SecretOwner,
        ctx: &mut ModelContext<Self>,
    ) {
        ctx.emit(HarnessAvailabilityEvent::AuthSecretDeletionFailed {
            harness,
            name,
            owner,
            error: HOSTED_AUTH_SECRETS_UNAVAILABLE.to_string(),
        });
    }

    pub fn refresh(&self, ctx: &mut ModelContext<Self>) {
        ctx.emit(HarnessAvailabilityEvent::Changed);
    }
}

impl Entity for HarnessAvailabilityModel {
    type Event = HarnessAvailabilityEvent;
}

impl SingletonEntity for HarnessAvailabilityModel {}
