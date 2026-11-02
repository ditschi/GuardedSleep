#!/usr/bin/env bash
# Claude Code Stop/SubagentStop hook (.claude/settings.json): lists leftovers of the turn with
# scripts/lib/reap.sh (AGENTS.md section 10.2) and shows them to the user as a systemMessage.
# Never blocks the stop and never ends anything: stopping stays `just reap --stop`.
# Hook output format: https://docs.anthropic.com/en/docs/claude-code/hooks#common-json-fields
set -uo pipefail
cat >/dev/null # hook input (JSON on stdin) is not needed

out="$(timeout 20 "$(dirname "$0")/reap.sh" 2>&1)" || exit 0
[ "$out" = "nothing to reap" ] && exit 0

# JSON-escape backslashes, quotes and newlines; reap.sh prints no other control characters.
msg="just reap found leftovers:"$'\n'"$out"
msg="${msg//\\/\\\\}"
msg="${msg//\"/\\\"}"
msg="${msg//$'\n'/\\n}"
printf '{"systemMessage":"%s"}\n' "$msg"
