//! This module should houses all horizontal/cross-cutting AI functionality throughout
//! Warp (including Agent Mode).
//!
//! The side panel Warp AI implementation lives in `super::ai_assistant`.
pub(crate) mod active_agent_views_model;
pub(crate) mod agent;
// TODO(issue #23): Remove with legacy Rust-bound Conversation server synchronization.
#[allow(dead_code)]
pub(crate) mod agent_conversations_model;
pub(crate) mod agent_events;
pub(crate) mod agent_management;
pub(crate) mod agent_tips;
pub(crate) mod ai_document_view;
pub mod ambient_agents;
pub(crate) mod artifact_download;
pub mod artifacts;
pub(crate) mod attachment_utils;
pub mod auth_secret_types;
#[cfg(not(target_family = "wasm"))]
// TODO(issue #23): Remove with the legacy Warp-hosted Agent SDK and harness flows.
#[allow(dead_code)]
pub mod aws_credentials;
#[cfg(not(target_family = "wasm"))]
// TODO(issue #23): Remove with the legacy cloud-provider credential refresh path.
#[allow(dead_code)]
pub(crate) mod bedrock_credentials;
pub(crate) mod block_context;
pub(crate) mod blocklist;
#[cfg(any(feature = "local_fs", not(target_family = "wasm")))]
pub(crate) mod codebase_auto_indexing;
pub(crate) mod semantic_indexing_removal;
pub mod control_code_parser;
pub(crate) mod conversation_details_panel;
pub(crate) mod conversation_navigation;
pub(crate) mod conversation_rename;
pub(crate) mod conversation_status_ui;
pub(crate) mod conversation_utils;
pub(crate) mod custom_model_router_editor;
pub(crate) mod custom_model_routers;
pub(crate) mod document;
#[cfg(not(target_family = "wasm"))]
// TODO(issue #23): Remove with the legacy GEAP credential refresh path.
#[allow(dead_code)]
pub mod geap_credentials;
pub(crate) mod get_relevant_files;
// TODO(issue #23): Remove with the legacy hosted harness availability model.
#[allow(dead_code)]
pub mod harness_availability;
pub(crate) mod harness_display;
pub(crate) mod llms;
pub(crate) mod local_harness_setup;
pub(crate) mod metadata_project_rules;
pub mod onboarding;
// TODO(issue #23): Remove with legacy server-backed Agent workspace persistence.
#[allow(dead_code)]
pub(crate) mod persisted_workspace;
pub(crate) mod predict;
#[cfg(all(not(target_family = "wasm"), feature = "local_fs"))]
pub(crate) mod remote_agent_context;
pub(crate) mod remote_context_files;
// TODO(issue #23): Remove with Warp-hosted AI quota and credit state.
#[allow(dead_code)]
pub mod request_usage_model;
pub(crate) mod restored_conversations;
pub(crate) mod skills;
pub(crate) mod voice;
pub use agent_tips::*;
pub use request_usage_model::*;
use warpui::AppContext;
#[cfg(not(target_family = "wasm"))]
// TODO(issue #23): Remove with the legacy Warp-hosted Agent SDK and harness flows.
#[allow(dead_code)]
pub mod agent_sdk;
pub mod cloud_agent_config;
pub mod cloud_agent_settings;
pub mod cloud_environments;
// TODO(issue #23): Remove with Warp-hosted worker discovery.
#[allow(dead_code)]
pub mod connected_self_hosted_workers;
pub mod execution_profiles;
// TODO(issue #23): Remove with server-backed AI Facts.
#[allow(dead_code)]
pub mod facts;
pub(crate) mod generate_block_title;
pub(crate) mod generate_code_review_content;
pub(crate) mod loading;
pub mod mcp;
pub mod outline;

pub(crate) use ai::paths;

pub fn init(app: &mut AppContext) {
    blocklist::keyboard_navigable_buttons::init(app);
    blocklist::block::number_shortcut_buttons::init(app);
    blocklist::toggleable_items::init(app);
    blocklist::suggested_agent_mode_workflow_modal::init(app);
    ai_document_view::init(app);
    conversation_details_panel::init(app);
    agent_management::init(app);
}
