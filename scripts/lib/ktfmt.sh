#!/usr/bin/env bash
# Runs the mise-pinned ktfmt jar (mise.toml "http:ktfmt") over the Android Kotlin sources.
#   scripts/lib/ktfmt.sh --dry-run --set-exit-if-changed   # check (lists unformatted files, exit 1)
#   scripts/lib/ktfmt.sh             # format in place
# Options: https://github.com/facebook/ktfmt#command-line
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=require.sh
source scripts/lib/require.sh
dir="$(mise where http:ktfmt 2>/dev/null || true)"
jar="$(compgen -G "$dir/ktfmt-*-with-dependencies.jar" | head -1 || true)"
[ -n "$jar" ] || { echo "ktfmt jar not found: run 'just setup' (mise install)" >&2; exit 1; }
command -v java >/dev/null 2>&1 || { echo "SKIPPED: ktfmt needs java"; exit 0; }
mapfile -d '' files < <(find apps/android -type f \( -name '*.kt' -o -name '*.kts' \) -not -path '*/build/*' -not -path '*/kotlin/uniffi/*' -print0)
[ ${#files[@]} -gt 0 ] || exit 0
exec java -jar "$jar" --kotlinlang-style "$@" "${files[@]}"
