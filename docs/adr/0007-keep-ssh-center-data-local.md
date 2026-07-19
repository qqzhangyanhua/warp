---
status: accepted
---

# Keep SSH Center data local

SSH Center keeps Remote Host Shortcuts and their credential references on the current device and does not sync them through Warp cloud services. Hostnames, IP addresses, usernames, commands, and related connection metadata reveal sensitive infrastructure, so cross-device convenience does not justify extending the cloud-sync and permissions boundary in the first version. These fields must not appear in telemetry, logs, Sentry breadcrumbs, or error reports. Because the feature is entirely local, it remains available without Account Sign-in and in Anonymous-only Mode and Local-only Mode.
