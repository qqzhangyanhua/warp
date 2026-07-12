//! General-purpose administrative commands in the Warp CLI.

use anyhow::{Context, Result};
use serde::Serialize;
use warp_cli::agent::OutputFormat;
use warp_core::features::FeatureFlag;
use warpui::platform::TerminationMode;
use warpui::{AppContext, SingletonEntity};

use crate::auth::auth_manager::{AuthManager, AuthManagerEvent};
use crate::auth::user::PrincipalType;
use crate::auth::AuthStateProvider;
use crate::workspaces::user_workspaces::UserWorkspaces;

/// Kick off a device authorization login flow and handle auth events.
pub fn login(ctx: &mut AppContext) -> Result<()> {
    if crate::local_mode::is_local_only_custom_provider_mode() {
        anyhow::bail!(crate::local_mode::account_sign_in_unavailable_message());
    }

    if FeatureFlag::AnonymousOnlyMode.is_enabled() {
        anyhow::bail!("This build only supports anonymous mode; account sign-in is unavailable");
    }

    let auth_state = AuthStateProvider::as_ref(ctx).get();
    let has_cached_credentials = auth_state.is_logged_in();

    // If the user is already logged in, we require that the user log out before logging
    // back in to ensure their existing state isn't replaced (especially if using both the CLI
    // and the desktop app). In this case, try refreshing their credentials first. If the user
    // is trying to log in because the cached credentials are invalid, we should let them do so.
    // Track whether we've started the device auth flow. Failure events
    // that arrive before device auth has started are leftover refresh
    // errors and should be ignored rather than treated as terminal.
    let mut started_device_auth = !has_cached_credentials;
    ctx.subscribe_to_model(
        &AuthManager::handle(ctx),
        move |_, event, ctx| match event {
            AuthManagerEvent::AuthComplete => {
                if !started_device_auth {
                    // Refresh succeeded - credentials are still valid.
                    let auth_state = AuthStateProvider::as_ref(ctx).get();
                    match (auth_state.username_for_display(), auth_state.user_email()) {
                        (Some(username), Some(email)) if username != email => {
                            println!("You are already logged in as {username} ({email}).")
                        }
                        (Some(name), _) | (None, Some(name)) => {
                            println!("You are already logged in as {name}.")
                        }
                        (None, None) => {
                            println!("You are already logged in.")
                        }
                    }
                    ctx.terminate_app(TerminationMode::ForceTerminate, None);
                } else {
                    // Device auth succeeded.
                    println!("Logged in successfully");
                    ctx.terminate_app(TerminationMode::ForceTerminate, None);
                }
            }
            AuthManagerEvent::AuthFailed(_) => {
                if !started_device_auth {
                    // Refresh failed - start a fresh device auth flow.
                    started_device_auth = true;
                    AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                        auth_manager.authorize_device(ctx);
                    });
                } else {
                    // Device auth failed.
                    let err_msg = match event {
                        AuthManagerEvent::AuthFailed(err) => {
                            format!("Authentication failed: {err:#}")
                        }
                        _ => "Authentication failed".to_string(),
                    };
                    ctx.terminate_app(
                        TerminationMode::ForceTerminate,
                        Some(Err(anyhow::anyhow!(err_msg))),
                    );
                }
            }
            AuthManagerEvent::ReceivedDeviceAuthorizationCode {
                verification_url,
                verification_url_complete,
                user_code,
            } => {
                if let Some(url) = verification_url_complete {
                    println!("To log in, open this URL in your browser:\n{url}");
                } else {
                    println!(
                        "To log in, visit {verification_url} and enter this code: {user_code}"
                    );
                }
            }
            _ => {}
        },
    );

    // Either refresh existing credentials or start device auth from scratch.
    AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
        if has_cached_credentials {
            auth_manager.refresh_user(ctx);
        } else {
            auth_manager.authorize_device(ctx);
        }
    });

    Ok(())
}

#[derive(Serialize)]
pub(super) struct WhoamiOutput {
    pub(super) uid: String,
    #[serde(rename = "type")]
    pub(super) principal_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) team_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) team_name: Option<String>,
}

pub(super) fn local_whoami_output(ctx: &AppContext) -> Result<WhoamiOutput> {
    Ok(WhoamiOutput {
        uid: crate::local_mode::get_or_create_local_identity(ctx)?.as_uid(),
        principal_type: "local",
        display_name: None,
        email: None,
        team_uid: None,
        team_name: None,
    })
}

pub(super) fn format_local_whoami_output(
    info: &WhoamiOutput,
    output_format: OutputFormat,
) -> Result<String> {
    match output_format {
        OutputFormat::Json => serde_json::to_string(info).context("whoami output should serialize"),
        OutputFormat::Pretty => Ok(format!("Local ID: {}", info.uid)),
        OutputFormat::Text => Ok(info.uid.clone()),
        OutputFormat::Ndjson => {
            anyhow::bail!("`whoami` does not support --output-format ndjson");
        }
    }
}

/// Singleton model that provides a `ModelContext` for the `whoami` command's async work.
struct WhoamiRunner;

impl warpui::Entity for WhoamiRunner {
    type Event = ();
}

impl SingletonEntity for WhoamiRunner {}

/// Print information about the currently authenticated principal.
pub fn whoami(ctx: &mut AppContext, output_format: OutputFormat) -> Result<()> {
    if crate::local_mode::is_local_only_custom_provider_mode() {
        let info = local_whoami_output(ctx)?;
        println!("{}", format_local_whoami_output(&info, output_format)?);
        ctx.terminate_app(TerminationMode::ForceTerminate, None);
        return Ok(());
    }

    let auth_state = AuthStateProvider::as_ref(ctx).get();
    if FeatureFlag::AnonymousOnlyMode.is_enabled() {
        let info = WhoamiOutput {
            uid: auth_state
                .user_id()
                .map(|id| id.as_string())
                .unwrap_or_else(|| auth_state.anonymous_id()),
            principal_type: "anonymous",
            display_name: None,
            email: None,
            team_uid: None,
            team_name: None,
        };

        match output_format {
            OutputFormat::Json => println!(
                "{}",
                serde_json::to_string(&info).context("whoami output should serialize")?
            ),
            OutputFormat::Pretty => println!("Anonymous ID: {}", info.uid),
            OutputFormat::Text => println!("anonymous:{}", info.uid),
            OutputFormat::Ndjson => {
                anyhow::bail!("`whoami` does not support --output-format ndjson");
            }
        }
        ctx.terminate_app(TerminationMode::ForceTerminate, None);
        return Ok(());
    }

    let principal_type = auth_state.principal_type().unwrap_or_default();

    let uid = auth_state
        .user_id()
        .map(|id| {
            let s = id.as_string();
            s.strip_prefix("serviceAccount:")
                .map(String::from)
                .unwrap_or(s)
        })
        .ok_or_else(|| anyhow::anyhow!("Could not determine user ID. Are you logged in?"))?;

    let mut info = WhoamiOutput {
        uid,
        principal_type: match principal_type {
            PrincipalType::User => "user",
            PrincipalType::ServiceAccount => "service_account",
        },
        display_name: auth_state.display_name(),
        email: match principal_type {
            PrincipalType::User => auth_state.user_email().filter(|e| !e.is_empty()),
            PrincipalType::ServiceAccount => None,
        },
        team_uid: None,
        team_name: None,
    };

    // Refresh workspace metadata before reading team info, so we don't print
    // stale or missing team data if the metadata hasn't been fetched yet.
    let runner = ctx.add_singleton_model(|_| WhoamiRunner);
    runner.update(ctx, move |_, ctx| {
        let refresh_future = super::common::refresh_workspace_metadata(ctx);
        ctx.spawn(refresh_future, move |_, result, ctx| {
            if let Err(err) = result {
                // Do not prevent showing user info if fetching team metadata fails.
                log::warn!("Failed to refresh team metadata for whoami: {err:#}");
            }

            let current_team = UserWorkspaces::as_ref(ctx).current_team();
            info.team_uid = current_team.map(|t| t.uid.to_string());
            info.team_name = current_team
                .map(|t| t.name.clone())
                .filter(|n| !n.is_empty());

            match output_format {
                OutputFormat::Json => {
                    match serde_json::to_string(&info).context("whoami output should serialize") {
                        Ok(json) => println!("{json}"),
                        Err(err) => {
                            ctx.terminate_app(TerminationMode::ForceTerminate, Some(Err(err)));
                            return;
                        }
                    }
                }
                OutputFormat::Pretty => {
                    match principal_type {
                        PrincipalType::User => println!("User ID: {}", info.uid),
                        PrincipalType::ServiceAccount => {
                            println!("Service account ID: {}", info.uid)
                        }
                    }
                    if let Some(name) = &info.display_name {
                        println!("Display Name: {name}");
                    }
                    if let Some(email) = &info.email {
                        println!("Email: {email}");
                    }
                    if let Some(team_uid) = &info.team_uid {
                        println!("Team ID: {team_uid}");
                    }
                    if let Some(team_name) = &info.team_name {
                        println!("Team Name: {team_name}");
                    }
                }
                OutputFormat::Text => {
                    println!("{}:{}", info.principal_type, info.uid);
                }
                OutputFormat::Ndjson => {
                    ctx.terminate_app(
                        TerminationMode::ForceTerminate,
                        Some(Err(anyhow::anyhow!(
                            "`whoami` does not support `--output-format ndjson`"
                        ))),
                    );
                    return;
                }
            }

            ctx.terminate_app(TerminationMode::ForceTerminate, None);
        });
    });

    Ok(())
}

/// Log out of Warp using the same logic as the app.
pub fn logout(ctx: &mut AppContext) -> Result<()> {
    if crate::local_mode::is_local_only_custom_provider_mode() {
        anyhow::bail!(crate::local_mode::account_logout_unavailable_message());
    }

    if FeatureFlag::AnonymousOnlyMode.is_enabled() {
        anyhow::bail!("This build only supports anonymous mode; account logout is unavailable");
    }

    let auth_state = AuthStateProvider::as_ref(ctx).get();
    if !auth_state.is_logged_in() {
        println!("You are not logged in.");
        ctx.terminate_app(TerminationMode::ForceTerminate, None);
        return Ok(());
    }

    crate::auth::log_out(ctx);
    println!("Logged out successfully.");
    ctx.terminate_app(TerminationMode::ForceTerminate, None);
    Ok(())
}
