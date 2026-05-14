#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
cspell_bin="$root/node_modules/.bin/cspell"
if [ ! -x "$cspell_bin" ]; then
  echo "cspell not found in node_modules/.bin (run: pnpm install)" >&2
  exit 1
fi
"$cspell_bin" lint --no-progress --config cspell-i18n-de.json
"$cspell_bin" lint --no-progress --config cspell-i18n-fr.json
"$cspell_bin" lint --no-progress --config cspell-i18n-es.json
