# Environment Variable Collections (removed)

Environment Variable Collections (EVCs) are **removed** from the ZYH local
product.

## Product contract

- Menus, commands, Drive create paths, command palette results, and Agent
  context must not expose EVC.
- Stale session restore of EVC panes fails closed without constructing
  `CloudModel` or `UpdateManager`. See `product_removal.rs`.
- Ordinary shell environment variables, project `.env` files, system
  environment variables, and user-selected Secret Manager CLIs remain
  unchanged.
- No ZYH-owned plaintext JSON or SQLite secret store replaces EVC.

## Remaining code

Legacy cloud model types and UI modules may still exist for compile continuity
while other surfaces finish detangling. They are not product entry points:

- Open/create paths call `may_open_or_create_evc()` / show `EVC_REMOVED_GUIDANCE`.
- `EnvVarCollectionPane::restore` returns an unsupported error.
- Palette/Drive listing gates on `may_expose_evc_in_ui()`.

## Guidance for users

Use shell configuration, a project `.env` file, system environment variables,
or your Secret Manager CLI.
