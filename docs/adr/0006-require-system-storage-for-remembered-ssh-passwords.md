---
status: accepted
---

# Require system storage for remembered SSH passwords

SSH Center stores shortcut metadata separately from Remembered SSH Passwords. Passwords are stored only in operating-system-backed secret storage, are never read back into the UI, and are supplied through a dedicated SSH authentication channel; terminal prompt detection and file-based fallback storage are prohibited because either could expose a password to untrusted terminal output or local file access. When system secret storage is unavailable, SSH Center disables password remembering instead of degrading security.
