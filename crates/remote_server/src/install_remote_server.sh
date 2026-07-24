#!/usr/bin/env bash
# Installs the ZYH remote server binary on a remote host, plus the
# artifact's `resources/` tree (bundled skills, settings schema) at a
# global, version-independent location:
#
#   {install_dir}/
#   ├── {binary_name}{version_suffix}   ← the executable
#   └── bundled_resources/              ← the artifact's resources tree
#
# Resources are deliberately decoupled from the binary version: the last
# install wins. An older daemon that is still running parsed its skills at
# startup, so a slightly newer resources tree underneath it is accepted.
#
# The client always uploads a verified local tarball over SSH/SCP before
# this script runs. There is no HTTP/CDN download path.
#
# Placeholders (substituted at runtime by setup.rs):
#   {install_dir}               — e.g. ~/.zyh/remote-server
#   {binary_name}               — channel binary name
#   {version_suffix}            — e.g. -v0.2026... (empty when unversioned)
#   {bundled_resources_dir_name} — global resources directory name
#   {staging_tarball_path}      — path to the pre-uploaded tarball (required)
set -e

arch=$(uname -m)
case "$arch" in
  x86_64|amd64)  arch_name=x86_64 ;;
  aarch64|arm64) arch_name=aarch64 ;;
  *) echo "unsupported arch: $arch" >&2; exit 2 ;;
esac

os_kernel=$(uname -s)
case "$os_kernel" in
  Darwin) os_name=macos ;;
  Linux)  os_name=linux ;;
  *) echo "unsupported OS: $os_kernel" >&2; exit 2 ;;
esac

# Record platform for diagnostics; install selection happens on the client.
echo "zyh-remote-daemon platform=${os_name}-${arch_name}" >&2

install_dir="{install_dir}"
# Avoid `${var/pattern/replacement}` for tilde expansion. Two
# interpreter quirks make it dangerous in this script:
#   1. bash 3.2 (macOS /bin/bash) keeps inner double-quotes around the
#      replacement literal, so `"$HOME"` ends up as 6 literal
#      characters and the install lands under a directory tree
#      literally named `"`.
#   2. bash 5.2+ enables `patsub_replacement` by default, which makes
#      `&` in the replacement expand to the matched pattern, so a
#      `$HOME` containing `&` resolves to a `~`-substituted path.
# Use `case` + `${var#\~}` instead — works on bash 3.2 and bash 5.2+
# without surprises.
case "$install_dir" in
  "~"|"~/"*) install_dir="${HOME}${install_dir#\~}" ;;
esac
mkdir -p "$install_dir"

tmpdir=$(mktemp -d "$install_dir/.install.XXXXXX")
# Best-effort cleanup of the staging directory. A failure here (e.g.
# EBUSY or "Directory not empty" races on some filesystems/mounts)
# must not fail the install: by the time this fires the binary has
# either already been moved into its final location, or the script
# has already failed for an unrelated reason that we want to surface
# instead of clobbering with the cleanup's exit code.
cleanup() {
  rm -rf "$tmpdir" 2>/dev/null || true
}
trap cleanup EXIT

staging_tarball_path="{staging_tarball_path}"
if [ -z "$staging_tarball_path" ]; then
  echo "error: staging tarball path is required; HTTP download is not supported" >&2
  exit 1
fi

# Same tilde-expansion caveat as install_dir above.
case "$staging_tarball_path" in
  "~"|"~/"*) staging_tarball_path="${HOME}${staging_tarball_path#\~}" ;;
esac
if [ ! -f "$staging_tarball_path" ]; then
  echo "error: staging tarball not found: $staging_tarball_path" >&2
  exit 1
fi
mv "$staging_tarball_path" "$tmpdir/zyh-remote-daemon.tar.gz"

tar -xzf "$tmpdir/zyh-remote-daemon.tar.gz" -C "$tmpdir"

# The executable and its resources are siblings in the artifact. Exclude the
# resources tree from the search: bundled skills may ship companion files
# whose names also start with the product prefix.
bin=$(find "$tmpdir" -type f \( -name 'oz*' -o -name 'zyh*' \) ! -name '*.tar.gz' ! -path '*/resources/*' | head -n1)
if [ -z "$bin" ]; then echo "no binary found in tarball" >&2; exit 1; fi
chmod +x "$bin"

# Install the resources tree at the global, version-independent location
# the daemon reads. `$tmpdir` lives inside `$install_dir`, so the `mv` is a
# same-filesystem rename. Installed before the binary so an interrupted
# install never leaves a new binary without its resources — the binary miss
# re-triggers this script. A tarball without resources is not an error: the
# daemon simply has no bundled skills.
resources="$(dirname "$bin")/resources"
if [ -d "$resources" ]; then
  rm -rf "$install_dir/{bundled_resources_dir_name}"
  mv "$resources" "$install_dir/{bundled_resources_dir_name}"
fi

mv "$bin" "$install_dir/{binary_name}{version_suffix}"
