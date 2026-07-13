# Warp Bridge Protocol

`core-v1.schema.json` is the authoritative Core Protocol schema. The TypeScript Bridge and Warp
Rust implementation must both accept every fixture under `fixtures/valid` and reject every fixture
under `fixtures/invalid`.

The Core Protocol schema hash is the lowercase SHA-256 digest of the schema file's exact bytes,
prefixed with `sha256:`. Changing the schema changes the hash; an incompatible Core change also
requires a protocol major-version change.

The current Core identity is
`sha256:afb439d8518d3ae8f2fb0f314845036f0c673c0c96eb7e849e0e71bdfd87600e`.
`src/protocol-identity.ts`, the valid Bridge hello fixture, and the fake Bridge must match these
exact bytes. The Bridge test suite verifies that identity on every run.

Bridge stdout is reserved for one JSON object per line. Warp accepts `bridge_hello` first and sends
no Run Configuration, Provider API Key, Transcript, Tool Catalog, or Resource Catalog until the
handshake succeeds. Negotiated frame and Transcript Sync byte limits apply before parsing or
buffering content.
