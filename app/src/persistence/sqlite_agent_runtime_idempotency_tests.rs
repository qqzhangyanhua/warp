use prost::Message as _;

use super::*;

fn runtime_task_with_attachments(attachment_keys: impl Iterator<Item = usize>) -> api::Task {
    let mut task = runtime_task("task-1", "Persist attachments exactly once");
    let Some(api::message::Message::UserQuery(user_query)) = task.messages[0].message.as_mut()
    else {
        panic!("runtime task should contain a user query");
    };
    for key in attachment_keys {
        user_query.referenced_attachments.insert(
            format!("attachment-{key}"),
            api::Attachment {
                value: Some(api::attachment::Value::PlainText(format!("contents-{key}"))),
            },
        );
    }
    task
}

#[test]
fn sqlite_writer_acknowledges_identical_runtime_mutation_after_acknowledgement_loss() {
    let task = runtime_task("task-1", "Persist exactly once");
    let harness = RuntimeWriterHarness::new(0, &[]);
    harness.lose_acknowledgement("commit-1", 0, vec![task.clone()], None);

    assert_eq!(
        harness.commit("commit-1", 0, vec![task.clone()], None),
        Ok(1)
    );

    harness.finish(|conn| assert_runtime_conversation(conn, 1, &[task]));
}

#[test]
fn sqlite_writer_acknowledges_logically_identical_task_maps_after_acknowledgement_loss() {
    let committed_task = runtime_task_with_attachments(0..16);
    let redelivered_task = runtime_task_with_attachments((0..16).rev());
    assert_eq!(committed_task, redelivered_task);
    assert_ne!(
        committed_task.encode_to_vec(),
        redelivered_task.encode_to_vec()
    );

    let harness = RuntimeWriterHarness::new(0, &[]);
    harness.lose_acknowledgement("commit-1", 0, vec![committed_task], None);

    assert_eq!(
        harness.commit("commit-1", 0, vec![redelivered_task.clone()], None),
        Ok(1)
    );

    harness.finish(|conn| assert_runtime_conversation(conn, 1, &[redelivered_task]));
}

#[test]
fn sqlite_writer_rejects_changed_runtime_mutation_with_reused_commit_identity() {
    let committed_task = runtime_task("task-1", "Persist this version");
    let harness = RuntimeWriterHarness::new(0, &[]);
    assert_eq!(
        harness.commit("commit-1", 0, vec![committed_task.clone()], None),
        Ok(1)
    );

    assert_eq!(
        harness.commit(
            "commit-1",
            0,
            vec![runtime_task("task-1", "Changed content")],
            None,
        ),
        Err(CommitAgentRuntimeMutationError::CommitIdentityConflict)
    );

    harness.finish(|conn| assert_runtime_conversation(conn, 1, &[committed_task]));
}

#[test]
fn sqlite_writer_rejects_last_commit_identity_reuse_after_history_changes() {
    let committed_task = runtime_task("task-1", "Committed output");
    let edited_task = runtime_task("task-1", "Edited history");
    let harness = RuntimeWriterHarness::new(0, &[]);
    assert_eq!(
        harness.commit("commit-1", 0, vec![committed_task], None),
        Ok(1)
    );
    harness.replace_history(2, std::slice::from_ref(&edited_task));

    assert_eq!(
        harness.commit(
            "commit-1",
            2,
            vec![runtime_task("task-1", "Reused identity")],
            None,
        ),
        Err(CommitAgentRuntimeMutationError::CommitIdentityConflict)
    );

    harness.finish(|conn| assert_runtime_conversation(conn, 2, &[edited_task]));
}

#[test]
fn sqlite_writer_rejects_commit_identity_reuse_with_changed_expected_revision() {
    let task = runtime_task("task-1", "Committed output");
    let harness = RuntimeWriterHarness::new(0, &[]);
    assert_eq!(
        harness.commit("commit-1", 0, vec![task.clone()], None),
        Ok(1)
    );

    assert_eq!(
        harness.commit("commit-1", 7, vec![task.clone()], None),
        Err(CommitAgentRuntimeMutationError::CommitIdentityConflict)
    );

    harness.finish(|conn| assert_runtime_conversation(conn, 1, &[task]));
}

#[test]
fn sqlite_writer_rejects_commit_identity_reuse_without_original_sidecar_mutation() {
    let task = runtime_task("task-1", "Retain the completed tool outcome");
    let harness = RuntimeWriterHarness::with_executing_tool(0, &[]);
    assert_eq!(
        harness.commit(
            "commit-1",
            0,
            vec![task.clone()],
            Some(complete_tool_execution()),
        ),
        Ok(1)
    );

    assert_eq!(
        harness.commit("commit-1", 0, vec![task], None),
        Err(CommitAgentRuntimeMutationError::CommitIdentityConflict)
    );

    harness.finish(assert_completed_tool);
}

#[test]
fn sqlite_writer_acknowledges_identical_tool_outcome_after_acknowledgement_loss() {
    let task = runtime_task("task-1", "Retain the completed tool outcome");
    let harness = RuntimeWriterHarness::with_executing_tool(0, &[]);
    harness.lose_acknowledgement(
        "commit-1",
        0,
        vec![task.clone()],
        Some(complete_tool_execution()),
    );

    assert_eq!(
        harness.commit("commit-1", 0, vec![task], Some(complete_tool_execution()),),
        Ok(1)
    );

    harness.finish(assert_completed_tool);
}
