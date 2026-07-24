use super::{ExternalUpdateOutcome, LocalNotebookSession, SavePlan};
use warpui_extras::owner_only_file::{content_hash, ExpectedContent};

fn hash_of(bytes: &[u8]) -> warpui_extras::owner_only_file::ContentHash {
    // Derive hash through a real file so we use the same ContentHash type as production.
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), bytes).unwrap();
    content_hash(temp.path()).unwrap().unwrap()
}

#[test]
fn new_unsaved_has_no_path_and_save_needs_path() {
    let session = LocalNotebookSession::new_unsaved("Notes");
    assert!(session.is_unsaved());
    assert!(session.path().is_none());
    assert!(!session.is_dirty());
    assert!(!session.has_conflict());
    assert_eq!(session.title_hint(), "Notes");
    assert_eq!(session.save_plan(), SavePlan::NeedsPath);
    assert!(session.document_path_for_relative_content().is_none());
}

#[test]
fn empty_title_becomes_untitled() {
    assert_eq!(
        LocalNotebookSession::new_unsaved("  ").title_hint(),
        "Untitled"
    );
}

#[test]
fn bound_clean_save_requires_expected_hash() {
    let hash = hash_of(b"# v1\n");
    let session = LocalNotebookSession::bound("/docs/a.md", hash);
    assert!(!session.is_unsaved());
    assert_eq!(session.path().unwrap().as_os_str(), "/docs/a.md");
    assert_eq!(
        session.save_plan(),
        SavePlan::Write {
            path: "/docs/a.md".into(),
            expected: ExpectedContent::Hash(hash),
        }
    );
    assert_eq!(
        session.document_path_for_relative_content().unwrap().as_os_str(),
        "/docs/a.md"
    );
}

#[test]
fn mark_edited_makes_bound_dirty_but_save_still_uses_hash() {
    let hash = hash_of(b"x");
    let mut session = LocalNotebookSession::bound("/n.md", hash);
    session.mark_edited();
    assert!(session.is_dirty());
    assert_eq!(
        session.save_plan(),
        SavePlan::Write {
            path: "/n.md".into(),
            expected: ExpectedContent::Hash(hash),
        }
    );
}

#[test]
fn apply_save_ok_binds_unsaved_and_clears_dirty() {
    let hash = hash_of(b"# saved\n");
    let mut session = LocalNotebookSession::new_unsaved("Draft");
    session.mark_edited(); // no-op for unsaved dirtiness tracking
    session.apply_save_ok("/tmp/draft.md", hash);

    assert!(!session.is_unsaved());
    assert!(!session.is_dirty());
    assert!(!session.has_conflict());
    assert_eq!(session.path().unwrap().file_name().unwrap(), "draft.md");
    assert_eq!(session.content_hash(), Some(hash));
    assert_eq!(
        session.save_plan(),
        SavePlan::Write {
            path: "/tmp/draft.md".into(),
            expected: ExpectedContent::Hash(hash),
        }
    );
}

#[test]
fn external_update_while_dirty_enters_conflict_and_blocks_save() {
    let original = hash_of(b"local\n");
    let external = hash_of(b"external\n");
    let mut session = LocalNotebookSession::bound("/c.md", original);
    session.mark_edited();

    assert_eq!(
        session.apply_external_update(external),
        ExternalUpdateOutcome::Conflict
    );
    assert!(session.has_conflict());
    assert_eq!(
        session.save_plan(),
        SavePlan::BlockedByConflict {
            path: "/c.md".into(),
        }
    );
}

#[test]
fn apply_save_conflict_blocks_until_refresh() {
    let hash = hash_of(b"a");
    let mut session = LocalNotebookSession::bound("/c.md", hash);
    session.mark_edited();
    session.apply_save_conflict();
    assert!(session.has_conflict());
    assert!(matches!(session.save_plan(), SavePlan::BlockedByConflict { .. }));

    let disk = hash_of(b"disk");
    session.apply_refresh(disk);
    assert!(!session.has_conflict());
    assert!(!session.is_dirty());
    assert_eq!(session.content_hash(), Some(disk));
    assert_eq!(
        session.save_plan(),
        SavePlan::Write {
            path: "/c.md".into(),
            expected: ExpectedContent::Hash(disk),
        }
    );
}

#[test]
fn clean_external_update_accepts_reload_with_new_hash() {
    let old = hash_of(b"old");
    let new = hash_of(b"new");
    let mut session = LocalNotebookSession::bound("/r.md", old);
    assert_eq!(
        session.apply_external_update(new),
        ExternalUpdateOutcome::AcceptReload
    );
    assert!(!session.has_conflict());
    assert_eq!(session.content_hash(), Some(new));
}

#[test]
fn matching_external_hash_while_dirty_is_treated_as_own_save() {
    let hash = hash_of(b"same");
    let mut session = LocalNotebookSession::bound("/s.md", hash);
    session.mark_edited();
    assert_eq!(
        session.apply_external_update(hash),
        ExternalUpdateOutcome::Ignore
    );
    assert!(!session.is_dirty());
    assert!(!session.has_conflict());
}

#[test]
fn bound_session_never_plans_expected_any() {
    let hash = hash_of(b"data");
    let mut session = LocalNotebookSession::bound("/x.md", hash);
    session.mark_edited();
    match session.save_plan() {
        SavePlan::Write { expected, .. } => {
            assert_eq!(expected, ExpectedContent::Hash(hash));
            assert_ne!(expected, ExpectedContent::Any);
        }
        other => panic!("expected Write plan, got {other:?}"),
    }
}
