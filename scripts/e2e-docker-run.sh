#!/usr/bin/env bash
# Run the host-independent E2E suite in docker/e2e-local (Linux + Docker only).
# Usage: pnpm run e2e:docker          # full auto matrix
#        pnpm run e2e:docker browser # browser baseline only
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$ROOT/docker/e2e-local/compose.yaml"

if ! command -v docker >/dev/null 2>&1; then
	echo "ERROR: docker not found on PATH." >&2
	exit 1
fi

if [[ "$(uname -s)" != "Linux" ]]; then
	echo "ERROR: docker/e2e-local requires Linux (uses network_mode: host for WebRTC)." >&2
	exit 1
fi

cd "$ROOT"
docker compose -f "$COMPOSE_FILE" build
docker compose -f "$COMPOSE_FILE" run --rm e2e "${1:-auto}"
