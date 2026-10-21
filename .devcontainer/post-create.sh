#!/usr/bin/env bash
# Dev Container setup: pinned tools (mise.toml), git hooks (lefthook) and JS dependencies.
set -euo pipefail

mise trust
mise install
just setup
pnpm install --frozen-lockfile
