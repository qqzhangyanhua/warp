CREATE TABLE agent_runtime_runs (
    id INTEGER PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    retry_of_run_id TEXT,
    starting_revision BIGINT NOT NULL CHECK (starting_revision >= 0),
    state TEXT NOT NULL CHECK (
        state IN (
            'starting',
            'running',
            'waiting_for_commit',
            'waiting_for_tool_result',
            'finished'
        )
    ),
    terminal_outcome TEXT CHECK (
        terminal_outcome IN ('completed', 'cancelled', 'failed', 'limit_reached')
    ),
    last_commit_id TEXT,
    last_committed_revision BIGINT CHECK (last_committed_revision >= 0),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (conversation_id) REFERENCES agent_conversations (conversation_id),
    FOREIGN KEY (conversation_id, retry_of_run_id)
        REFERENCES agent_runtime_runs (conversation_id, run_id),
    UNIQUE (conversation_id, run_id),
    CHECK (
        (state = 'finished' AND terminal_outcome IS NOT NULL)
        OR (state != 'finished' AND terminal_outcome IS NULL)
    ),
    CHECK (
        (last_commit_id IS NULL AND last_committed_revision IS NULL)
        OR (last_commit_id IS NOT NULL AND last_committed_revision IS NOT NULL)
    )
);

CREATE TRIGGER update_last_modified_at_for_agent_runtime_runs AFTER
UPDATE ON agent_runtime_runs FOR EACH ROW WHEN NEW.last_modified_at IS OLD.last_modified_at BEGIN
UPDATE agent_runtime_runs
SET
    last_modified_at = CURRENT_TIMESTAMP
WHERE
    id = OLD.id;

END;

CREATE TABLE agent_tool_execution_records (
    id INTEGER PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    tool_call_id TEXT NOT NULL,
    request_fingerprint BLOB NOT NULL CHECK (length(request_fingerprint) = 32),
    state TEXT NOT NULL CHECK (state IN ('pending', 'executing', 'completed')),
    complete_outcome_encoding_version INTEGER CHECK (complete_outcome_encoding_version > 0),
    complete_outcome BLOB,
    tool_result_projection_encoding_version INTEGER CHECK (
        tool_result_projection_encoding_version > 0
    ),
    tool_result_projection BLOB,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (conversation_id, run_id)
        REFERENCES agent_runtime_runs (conversation_id, run_id),
    UNIQUE (conversation_id, run_id, tool_call_id),
    CHECK (
        (
            state IN ('pending', 'executing')
            AND complete_outcome_encoding_version IS NULL
            AND complete_outcome IS NULL
            AND tool_result_projection_encoding_version IS NULL
            AND tool_result_projection IS NULL
        )
        OR (
            state = 'completed'
            AND complete_outcome_encoding_version IS NOT NULL
            AND complete_outcome IS NOT NULL
            AND tool_result_projection_encoding_version IS NOT NULL
            AND tool_result_projection IS NOT NULL
        )
    )
);

CREATE TRIGGER update_last_modified_at_for_agent_tool_execution_records AFTER
UPDATE ON agent_tool_execution_records FOR EACH ROW
WHEN NEW.last_modified_at IS OLD.last_modified_at BEGIN
UPDATE agent_tool_execution_records
SET
    last_modified_at = CURRENT_TIMESTAMP
WHERE
    id = OLD.id;

END;
