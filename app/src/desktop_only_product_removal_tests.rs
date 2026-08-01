use super::*;

#[test]
fn desktop_only_hosted_services_are_removed() {
    assert!(DESKTOP_ONLY_HOSTED_SERVICES_REMOVED);
    assert!(!may_run_automatic_updater());
    assert!(!may_use_voice_or_transcription());
    assert!(!may_upload_feedback_crash_or_diagnostics());
    assert!(!may_fetch_remote_changelog_survey_or_referral());
    assert!(!may_run_wasm_or_web_product_target());
    assert!(!may_initialize_crash_reporting());
}

#[test]
fn retained_local_version_and_diagnostics_export() {
    assert!(may_show_local_version_information());
    assert!(may_export_local_diagnostics());
}

#[test]
fn guidance_forbids_background_update_and_upload() {
    assert!(
        autoupdate_unavailable_message().contains("manually")
            || autoupdate_unavailable_message().contains("Automatic")
    );
    assert!(
        upload_unavailable_message().contains("not uploaded")
            || upload_unavailable_message().contains("local diagnostics")
    );
    assert!(voice_unavailable_message().contains("Voice"));
    assert!(wasm_product_unavailable_message().contains("desktop"));
}

#[test]
fn no_update_download_url_in_product_policy() {
    // Policy messages must not point users at a hosted download endpoint.
    for msg in [
        AUTOUPDATE_REMOVED_GUIDANCE,
        UPLOAD_REMOVED_GUIDANCE,
        REMOTE_CONTENT_REMOVED_GUIDANCE,
        WASM_PRODUCT_REMOVED_GUIDANCE,
    ] {
        assert!(!msg.contains("http://"));
        assert!(!msg.contains("https://"));
        assert!(!msg.contains("releases."));
        assert!(!msg.contains("/download"));
    }
}
