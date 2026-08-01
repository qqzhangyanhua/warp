//! Desktop-only product boundary for the permanent ZYH local product.
//!
//! The product ships macOS, Linux, and Windows desktop builds only. Automatic
//! update checks and downloads, Voice/transcription, feedback and crash upload,
//! remote changelog, surveys, remote referrals, and WASM/Web product targets are
//! not supported. Local version information and user-initiated local diagnostics
//! export remain.

/// Product flag: hosted updater / Voice / upload / WASM shell services are removed.
pub const DESKTOP_ONLY_HOSTED_SERVICES_REMOVED: bool = true;

/// Guidance when automatic updates are requested.
pub const AUTOUPDATE_REMOVED_GUIDANCE: &str = "Automatic updates are no longer available. \
Install a new ZYH desktop build manually when you choose to upgrade.";

/// Guidance when Voice or transcription is requested.
pub const VOICE_REMOVED_GUIDANCE: &str =
    "Voice input and transcription are no longer available in ZYH.";

/// Guidance when feedback, crash, or diagnostics upload is requested.
pub const UPLOAD_REMOVED_GUIDANCE: &str =
    "Feedback, crash reports, and remote diagnostics are no longer uploaded. \
Use a local diagnostics export when you need to share debug information.";

/// Guidance when remote changelog, survey, or referral fetch is requested.
pub const REMOTE_CONTENT_REMOVED_GUIDANCE: &str =
    "Remote changelog, surveys, and referral services are no longer available.";

/// Guidance when a WASM/Web product path is requested.
pub const WASM_PRODUCT_REMOVED_GUIDANCE: &str =
    "WASM and Web product targets are not part of ZYH. Use a desktop build.";

/// Whether the app may run automatic update checks, prompts, or downloads.
pub fn may_run_automatic_updater() -> bool {
    !DESKTOP_ONLY_HOSTED_SERVICES_REMOVED
}

/// Whether Voice / transcription UI, settings, or providers may run.
pub fn may_use_voice_or_transcription() -> bool {
    !DESKTOP_ONLY_HOSTED_SERVICES_REMOVED
}

/// Whether feedback, crash reports, or diagnostics may be uploaded to a host.
///
/// Local diagnostics export after an explicit user action remains allowed.
pub fn may_upload_feedback_crash_or_diagnostics() -> bool {
    !DESKTOP_ONLY_HOSTED_SERVICES_REMOVED
}

/// Whether remote changelog, survey, or referral network fetches may run.
pub fn may_fetch_remote_changelog_survey_or_referral() -> bool {
    !DESKTOP_ONLY_HOSTED_SERVICES_REMOVED
}

/// Whether a WASM/Web product target may start or serve as a supported surface.
pub fn may_run_wasm_or_web_product_target() -> bool {
    !DESKTOP_ONLY_HOSTED_SERVICES_REMOVED
}

/// Whether crash reporting / Sentry initialization may run.
pub fn may_initialize_crash_reporting() -> bool {
    !DESKTOP_ONLY_HOSTED_SERVICES_REMOVED
}

/// Local version / About information remains available.
pub fn may_show_local_version_information() -> bool {
    true
}

/// User-initiated local diagnostics export (file write) remains available.
pub fn may_export_local_diagnostics() -> bool {
    true
}

pub fn autoupdate_unavailable_message() -> String {
    AUTOUPDATE_REMOVED_GUIDANCE.to_string()
}

pub fn voice_unavailable_message() -> String {
    VOICE_REMOVED_GUIDANCE.to_string()
}

pub fn upload_unavailable_message() -> String {
    UPLOAD_REMOVED_GUIDANCE.to_string()
}

pub fn remote_content_unavailable_message() -> String {
    REMOTE_CONTENT_REMOVED_GUIDANCE.to_string()
}

pub fn wasm_product_unavailable_message() -> String {
    WASM_PRODUCT_REMOVED_GUIDANCE.to_string()
}

#[cfg(test)]
#[path = "desktop_only_product_removal_tests.rs"]
mod tests;
