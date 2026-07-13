# Warp Agent Runtime Bridge

This Warp-owned package implements the local Bridge Protocol and will host the standalone Pi Agent
Runtime adapter. `protocol/core-v1.schema.json` and its fixtures are authoritative; vendored Pi code
cannot add or alter protocol messages.

## Development

Use pnpm only:

```sh
pnpm install
pnpm test
pnpm typecheck
```

Pi dependencies resolve exclusively from `../../third_party/pi/packages`. The pnpm configuration
allows only the pinned `bun` package's install script; transitive package install scripts remain
disabled.

## Standalone toolchain

`bun@1.3.14` is pinned as the standalone compiler. The accepted target names are:

- `bun-darwin-arm64`
- `bun-darwin-x64`
- `bun-linux-arm64`
- `bun-linux-x64`
- `bun-windows-arm64`
- `bun-windows-x64`

Run `pnpm run build:fake:matrix` to compile the spawnable fake Bridge for all six targets under
`dist/fake/`. This smoke build proves compiler and target availability without creating a release
artifact. The host artifact must also pass the fake Bridge handshake test.

Production packaging remains fail-closed until the real Bridge entry point, artifact manifest,
digest/size verification, and platform signing integration are present. Nothing under `dist/` is
checked in or downloaded by the installed application.
