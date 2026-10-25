#!/usr/bin/env bash
# No Google STUN host anywhere (owner decision 2026-10-22, openspec add-rust-signaling-server):
# the default is the configured relay's STUN, and a third-party server needs an explicit opt-in.
# Docs that explain the ban, the archive and this script are exempt.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

pattern='stun[0-9]*\.l\.google\.com|stun\.google|[a-z0-9.-]*\.google\.com:19302|stun:[^ "'"'"']*google'
exempt='^(openspec/changes/archive/|openspec/changes/alpha-connection-model/|crates/public-lists/|docs/ci-gates\.md$|openspec/changes/add-stun-server-choice/|openspec/changes/add-rust-signaling-server/|openspec/specs/signaling-relay/|docs/reviews/|docs/technical/analysis\.md$|CHANGELOG\.md$|scripts/lib/no-google-stun\.sh$)'

status=0
while IFS= read -r -d '' f; do
  [[ -f "$f" ]] || continue
  if grep -nEI -- "$pattern" "$f" | sed "s|^|no-google-stun: $f:|" | grep .; then
    status=1
  fi
done < <(git ls-files -z --cached --others --exclude-standard | grep -zEv "$exempt")
[[ $status -eq 0 ]] || echo "no-google-stun: remove the Google STUN host (or explain the ban in an exempt doc)" >&2
exit "$status"
