CREATE TABLE personal_memory_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    record_id TEXT NOT NULL UNIQUE,
    fact_text TEXT NOT NULL CHECK (length(fact_text) BETWEEN 1 AND 4096),
    value_text TEXT NOT NULL CHECK (length(value_text) BETWEEN 1 AND 2048),
    topic TEXT NOT NULL CHECK (length(topic) BETWEEN 1 AND 512),
    normalized_topic TEXT NOT NULL CHECK (length(normalized_topic) BETWEEN 1 AND 512),
    labels_json TEXT NOT NULL DEFAULT '[]',
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    index_state TEXT NOT NULL DEFAULT 'unconfigured'
        CHECK (index_state IN ('unconfigured', 'pending', 'ready', 'unavailable')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX personal_memory_records_normalized_topic
    ON personal_memory_records (normalized_topic);
CREATE INDEX personal_memory_records_updated_at
    ON personal_memory_records (updated_at DESC);

CREATE TRIGGER personal_memory_records_update_timestamp
AFTER UPDATE ON personal_memory_records
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE personal_memory_records
    SET updated_at = CURRENT_TIMESTAMP
    WHERE id = NEW.id;
END;

CREATE TRIGGER personal_memory_records_capacity
BEFORE INSERT ON personal_memory_records
FOR EACH ROW
WHEN (SELECT COUNT(*) FROM personal_memory_records) >= 5000
BEGIN
    SELECT RAISE(ABORT, 'personal_memory_capacity');
END;
