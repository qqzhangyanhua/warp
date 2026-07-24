use super::*;

#[test]
fn hosted_semantic_indexing_is_disabled_even_when_legacy_flags_are_forced_on() {
    let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(true);
    let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(true);
    assert!(!codebase_indexing_enabled(
        CodebaseAutoIndexingSurface::Local,
        true,
    ));
    assert!(!codebase_indexing_enabled(
        CodebaseAutoIndexingSurface::Remote,
        true,
    ));
    assert!(!codebase_auto_indexing_enabled(
        CodebaseAutoIndexingSurface::Local,
        true,
        true,
    ));
    assert!(!codebase_auto_indexing_enabled(
        CodebaseAutoIndexingSurface::Remote,
        true,
        true,
    ));
    assert!(!should_use_codebase_indexing_with_flags(
        CodebaseAutoIndexingSurface::Local,
        true,
        true,
        true,
    ));
}

/// Helper that mirrors the pre-removal gate inputs for documentation only.
fn should_use_codebase_indexing_with_flags(
    surface: CodebaseAutoIndexingSurface,
    full_source: bool,
    remote: bool,
    codebase_context: bool,
) -> bool {
    let _ = (surface, full_source, remote, codebase_context);
    // Product removal wins over every legacy combination.
    crate::ai::semantic_indexing_removal::may_use_hosted_semantic_indexing()
        && full_source
        && codebase_context
}

#[test]
fn local_auto_indexing_stays_off_when_flag_disabled() {
    let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(false);
    assert!(!codebase_auto_indexing_enabled(
        CodebaseAutoIndexingSurface::Local,
        true,
        true,
    ));
}

#[test]
fn remote_auto_indexing_stays_off_when_product_removed() {
    let _remote_flag = FeatureFlag::RemoteCodebaseIndexing.override_enabled(true);
    let _flag = FeatureFlag::FullSourceCodeEmbedding.override_enabled(true);
    assert!(!codebase_auto_indexing_enabled(
        CodebaseAutoIndexingSurface::Remote,
        true,
        true,
    ));
}

#[test]
fn candidate_roots_are_deduped_before_filtering() {
    let roots = vec!["/repo", "/repo", "/other"];
    let candidates = auto_index_candidate_roots(roots, |root| *root != "/other");

    assert_eq!(candidates, vec!["/repo"]);
}
