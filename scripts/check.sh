#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

echo "Harness check: running ./script/presubmit"
./script/presubmit
