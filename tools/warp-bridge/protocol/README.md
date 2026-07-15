# Warp Bridge Protocol

`core-v2.schema.json` is the authoritative Core Protocol schema. The TypeScript Bridge and Warp
Rust implementation must both accept every fixture under `fixtures/valid` and reject every fixture
under `fixtures/invalid`.

The Core Protocol schema hash is the lowercase SHA-256 digest of the schema file's exact bytes,
prefixed with `sha256:`. Changing the Core schema changes the hash and requires a protocol
major-version change.

The current Core identity is
`sha256:7a44caef7fc85b2719d1c3ae7f98bab98f221287a4de6541d6386d1f590c578c`.
`src/protocol-identity.ts`, the valid Bridge hello fixture, and the fake Bridge must match these
exact bytes. The Bridge test suite verifies that identity on every run.

Bridge stdout is reserved for one JSON object per line. Warp accepts `bridge_hello` first and sends
no Run Configuration, Provider API Key, Transcript, Tool Catalog, or Resource Catalog until the
handshake succeeds. Negotiated frame and Transcript Sync byte limits apply before parsing or
buffering content.

`transcript_sync_begin.total_bytes` is the sum of the UTF-8 byte lengths of the exact
`transcript_sync_item` JSON frames in that sync. JSONL newline delimiters are not included.
