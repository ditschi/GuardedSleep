#!/usr/bin/env bash
# Run a command in the same Docker Compose service as the Dev Container (mount: repo → /workspace).
# Usage (from repository root): ./scripts/devcontainer-run.sh pnpm run lint
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/dev-compose-env.sh
source "$ROOT/scripts/lib/dev-compose-env.sh"
dev_compose__prepare_user_for_compose "$ROOT"
exec docker compose --project-directory "$ROOT/.devcontainer" -f "$ROOT/.devcontainer/docker-compose.yml" run --rm dev "$@"
