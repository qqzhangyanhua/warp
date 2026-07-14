ALTER TABLE agent_runtime_runs
ADD COLUMN last_commit_payload_fingerprint BLOB CHECK (
    last_commit_payload_fingerprint IS NULL
    OR length(last_commit_payload_fingerprint) = 32
);
