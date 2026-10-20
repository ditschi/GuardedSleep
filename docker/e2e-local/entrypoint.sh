#!/usr/bin/env bash
set -euo pipefail

cd /workspace

corepack enable
corepack prepare pnpm@9.15.4 --activate

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
