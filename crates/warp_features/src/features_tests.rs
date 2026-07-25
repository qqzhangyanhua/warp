use super::*;

#[test]
#[ignore = "CORE-3768 - need to clean up PREVIEW_FLAGS, but this is a temporary fix for the cluttered changelog"]
fn test_all_preview_flags_have_a_description() {
    for flag in PREVIEW_FLAGS {
        assert!(
            flag.flag_description()
                .is_some_and(|description| !description.is_empty()),
            "Missing description for preview-enabled flag {flag:?}"
        );
    }
}

#[test]
fn local_child_harnesses_are_local_only_by_default() {
    assert!(LOCAL_FLAGS.contains(&FeatureFlag::LocalClaudeCodexChildHarnesses));
    assert!(!DEBUG_FLAGS.contains(&FeatureFlag::LocalClaudeCodexChildHarnesses));
    assert!(!DOGFOOD_FLAGS.contains(&FeatureFlag::LocalClaudeCodexChildHarnesses));
}

#[test]
fn terminal_lifecycle_recovery_is_dogfood_only_by_default() {
    assert!(DOGFOOD_FLAGS.contains(&FeatureFlag::TerminalLifecycleRecovery));
    assert!(!PREVIEW_FLAGS.contains(&FeatureFlag::TerminalLifecycleRecovery));
    assert!(!RELEASE_FLAGS.contains(&FeatureFlag::TerminalLifecycleRecovery));
}

#[test]
fn release_flags_exclude_autoupdate_changelog_and_crash_upload() {
    // ZYH desktop-only product: no automatic updater, remote changelog, or crash upload.
    assert!(!RELEASE_FLAGS.contains(&FeatureFlag::Autoupdate));
    assert!(!RELEASE_FLAGS.contains(&FeatureFlag::Changelog));
    assert!(!RELEASE_FLAGS.contains(&FeatureFlag::CrashReporting));
    assert!(!RELEASE_FLAGS.contains(&FeatureFlag::AutoupdateUIRevamp));
}
