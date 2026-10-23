#!/usr/bin/env bash
# Fails when UI code carries user-visible literals. All copy goes through locales/*.json.
#  - Compose (apps/android/app/src/main/kotlin): a string literal passed to Text( / text = /
#    contentDescription = / label =. Empty strings and Modifier.testTag("...") are allowed.
#  - Svelte (apps/web/src, *.svelte): text between tags and literal accessibility attributes
#    (aria-label, aria-description, aria-valuetext, title, placeholder, alt). Text inside {...}
#    mustaches, <script>, <style> and comments is code, not copy; test files are not scanned.
set -euo pipefail
# UI_LITERALS_ROOT lets scripts/lib/ui-literals.test.mjs lint a temporary tree.
root="${UI_LITERALS_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
status=0

lint_compose() {
  local dir="$root/apps/android/app/src/main/kotlin"
  [ -d "$dir" ] || return 0
  # Text("x"  |  text = "x"  |  contentDescription = "x"  |  label = "x"; skips "" and testTag(...)
  local pattern='(\bText\(\s*|\b(text|contentDescription|label)\s*=\s*)"[^"]'
  local f out line
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
}

# Prints "<file>: <literal>" lines for one Svelte file; needs perl (in the build image and CI).
svelte_literals() {
  perl -0777 -ne '
    my $src = $_;
    $src =~ s/<script\b.*?<\/script>//gs;
    $src =~ s/<style\b.*?<\/style>//gs;
    $src =~ s/<!--.*?-->//gs;
    # Mustaches may contain < and >; drop them (innermost first) before looking at tags.
    1 while $src =~ s/\{[^{}]*\}//gs;
    my @found;
    while ($src =~ />([^<>]*)</gs) {
      my $text = $1;
      push @found, $text if $text =~ /\p{L}{2,}/;
    }
    while ($src =~ /\b(?:aria-label|aria-description|aria-valuetext|title|placeholder|alt)\s*=\s*"([^"]*)"/g) {
      my $value = $1;
      push @found, $value if $value =~ /\p{L}{2,}/;
    }
    for my $item (@found) {
      $item =~ s/\s+/ /g;
      $item =~ s/^ | $//g;
      print "$ARGV: $item\n";
    }
  ' "$1"
}

lint_svelte() {
  local dir="$root/apps/web/src"
  [ -d "$dir" ] || return 0
  local f out line
  while IFS= read -r -d '' f; do
    out=$(svelte_literals "$f")
    if [ -n "$out" ]; then
      while IFS= read -r line; do
        echo "ui-literal: ${line#"$root"/}" >&2
      done <<<"$out"
      status=1
    fi
  done < <(find "$dir" -name '*.svelte' -not -name '*.test.*' -print0)
}

lint_compose
compose_status=$status
lint_svelte
if [ "$status" -ne 0 ] && [ "$compose_status" -eq 0 ]; then
  echo "lint-ui-literals: move user-visible strings into locales/*.json (webShell.* and the web groups) and use \$t()." >&2
fi
exit "$status"
