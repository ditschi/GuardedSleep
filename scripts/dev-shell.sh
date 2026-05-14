#!/usr/bin/env bash
# Interactive shell in the same Dev Container image as the IDE (repo → /workspace).
# Publishes Vite (5173) and signaling (8787) to the host — no Node/pnpm on the host needed.
#
# Usage (from repository root): ./scripts/dev-shell.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/dev-compose-env.sh
source "$ROOT/scripts/lib/dev-compose-env.sh"
dev_compose__prepare_user_for_compose "$ROOT"
COMPOSE=(
  docker compose
  --project-directory "$ROOT/.devcontainer"
  -f "$ROOT/.devcontainer/docker-compose.yml"
)

"${COMPOSE[@]}" build
"${COMPOSE[@]}" run --rm -it --service-ports dev bash -lc '
set -euo pipefail
sudo corepack enable
corepack prepare pnpm@9.15.4 --activate
cd /workspace
printf "\n%s\n" "GuardedSleep dev shell — repo at /workspace (bind mount)."
printf "%s\n" "  pnpm install        # once (writes node_modules into the mounted repo)"
printf "%s\n" "  pnpm run dev:local"
printf "%s\n\n" "  App http://localhost:5173/  Signaling ws://localhost:8787"
exec bash
'
