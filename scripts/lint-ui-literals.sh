#!/usr/bin/env bash
# Fails when Compose UI code passes a string literal to Text( / text = / contentDescription = / label =.
# Empty strings and Modifier.testTag("...") literals are allowed. All copy goes through locales/*.json.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/apps/android/app/src/main/kotlin"
[ -d "$dir" ] || exit 0

# Text("x"  |  text = "x"  |  contentDescription = "x"  |  label = "x"; skips "" and testTag(...)
pattern='(\bText\(\s*|\b(text|contentDescription|label)\s*=\s*)"[^"]'
status=0
while IFS= read -r -d '' f; do
  if out=$(grep -nE "$pattern" "$f" | grep -vE 'testTag\(' || true); [ -n "$out" ]; then
    while IFS= read -r line; do
      echo "ui-literal: ${f#"$root"/}:$line" >&2
    done <<<"$out"
    status=1
  fi
done < <(find "$dir" -name '*.kt' -print0)

if [ "$status" -ne 0 ]; then
  echo "lint-ui-literals: move user-visible strings into locales/*.json (native.*) and use stringResource()." >&2
fi
exit "$status"
