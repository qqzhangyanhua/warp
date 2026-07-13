# Vendored Pi Packages

Warp vendors the exact published Pi packages needed by `tools/warp-bridge`. The Bridge must use
these archives through local `file:` dependencies; it must not resolve Pi from the package registry,
a user installation, or the adjacent Pi checkout.

The authoritative package versions, source tag and commit, archive sizes, registry integrity values,
and SHA-256 digests are recorded in `provenance.json`. `SHA256SUMS` is provided for standard archive
verification. The upstream MIT license is preserved in `LICENSE` because the published archives do
not contain a license file.

## Updating

1. Choose an upstream release tag and resolve its immutable commit.
2. Download the exact published packages with pnpm into a temporary directory.
3. Confirm each archive's embedded `package.json` version, repository, and license.
4. Replace the archives under `packages/` and update `provenance.json` and `SHA256SUMS`.
5. Update the exact local `file:` dependencies and pnpm lockfile in `tools/warp-bridge`.
6. Run the Bridge conformance tests and rebuild every supported standalone artifact.
7. Update and verify the Bridge Artifact Manifest before packaging.

Never copy a dirty or post-release Pi checkout into this directory. Version changes require review
of the upstream diff and redistribution obligations.
