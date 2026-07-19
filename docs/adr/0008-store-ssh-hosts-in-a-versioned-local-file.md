---
status: accepted
---

# Store SSH hosts in a versioned local file

SSH Center stores its model in a versioned `ssh_hosts.json` under the current channel's local data directory, written with owner-only permissions through temporary-file replacement. The file contains stable shortcut IDs, connection metadata, authentication modes, credential references, and local recency data, but never passwords or private key contents. A dedicated file avoids a database migration and keeps runtime list data out of `settings.toml`, while explicit schema versioning leaves room for later local migrations.

Because shortcut metadata and Remembered SSH Passwords live in separate stores, cross-store changes use a persistent transaction journal. A create, update, authentication-mode change, or delete is reported as successful only after both stores reach the committed state; interrupted operations remain explicit and are replayed idempotently on startup instead of being silently discarded.

Each committed write retains one last-known-good backup. If the primary file cannot be parsed, SSH Center fails closed and does not replace it with an empty model or mutate credentials; the user may restore the backup or explicitly confirm a reset when no recovery is possible.
