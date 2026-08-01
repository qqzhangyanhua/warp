CREATE TABLE personal_memory_vectors (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    record_id TEXT NOT NULL UNIQUE
        REFERENCES personal_memory_records(record_id) ON DELETE CASCADE,
    index_identity TEXT NOT NULL CHECK (length(index_identity) = 64),
    dimensions INTEGER NOT NULL CHECK (dimensions BETWEEN 1 AND 8192),
    vector BLOB NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX personal_memory_vectors_index_identity
    ON personal_memory_vectors (index_identity);
