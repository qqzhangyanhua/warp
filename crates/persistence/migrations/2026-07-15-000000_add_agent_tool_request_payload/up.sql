ALTER TABLE agent_tool_execution_records
ADD COLUMN request_encoding_version INTEGER NOT NULL DEFAULT 1;

ALTER TABLE agent_tool_execution_records
ADD COLUMN request_payload BLOB NOT NULL DEFAULT X'';
