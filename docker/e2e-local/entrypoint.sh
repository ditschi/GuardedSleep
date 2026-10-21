#!/usr/bin/env bash
set -euo pipefail

cd /workspace

# pnpm 11 at the version pinned in mise.toml (corepack is not used, ADR-020 section 5).
PNPM_VERSION="$(sed -n 's/^pnpm *= *"\(.*\)"/\1/p' mise.toml)"
test -n "$PNPM_VERSION"
npm install --global "pnpm@${PNPM_VERSION}"

pnpm install --no-frozen-lockfile

case "${1:-auto}" in
	auto)
		exec bash scripts/e2e-local-automation.sh
		;;
	browser)
		exec pnpm run e2e
		;;
	shell)
		exec bash
		;;
	*)
		exec "$@"
		;;
esac
