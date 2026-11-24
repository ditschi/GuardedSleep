#!/usr/bin/env bash
# Code health with ai-health (openspec/changes/add-code-health, design §0; docs/contributing/code-health.md).
#
#   scripts/lib/code-health.sh setup                     venv + grammars (idempotent, stamped by pin and lock)
#   scripts/lib/code-health.sh scan [--component C] [--json]
#   scripts/lib/code-health.sh changed [BASE] [--check]  delta since the merge base with BASE, incl. uncommitted
#   scripts/lib/code-health.sh staged [--check]          delta of the index (lefthook)
#   scripts/lib/code-health.sh check [BASE]              the gate: changed --check + score comparison + suppressions
#   scripts/lib/code-health.sh cli [...]                 argument vocabulary of `just code-health`
#   scripts/lib/code-health.sh suppressions              every `disable=` marker needs a reason of >= 10 chars
#
# Components: rust (crates), web (apps, packages). Android is not scored until ai-health has a Kotlin
# adapter. Every call passes the profile with --config: without it ai-health silently uses
# <path>/.aihealth.toml or its defaults.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
# shellcheck source=require.sh
source scripts/lib/require.sh

ROOT="$PWD"
CH_DIR="$ROOT/target/code-health"
VENV="$CH_DIR/venv"
STAMP="$CH_DIR/venv.stamp"
AIHEALTH="$VENV/bin/aihealth"
PY="$VENV/bin/python"
# Drop of a component score between merge base and head that still passes (owner decision, design §0).
SCORE_TOLERANCE="0.01"
# Grammars ai-health needs; tree-sitter-language-pack downloads them once (SHA-256 checked) and runs
# offline afterwards. https://github.com/xberg-io/tree-sitter-language-pack
GRAMMARS="'rust','typescript','tsx','javascript'"

# Scanned paths per component (word-split on purpose).
paths_of() {
  case "$1" in
    rust) echo "crates" ;;
    web) echo "apps packages" ;;
    *) echo "unknown component: $1" >&2; exit 2 ;;
  esac
}
COMPONENTS="rust web"
# Source extensions each component scores (a staged commit without them skips the hook run).
ext_re_of() { case "$1" in rust) echo '\.rs$' ;; web) echo '\.(ts|tsx|js|mjs)$' ;; esac; }

pin_hash() { sha256sum tools/code-health/aihealth.in tools/code-health/aihealth.lock.txt | sha256sum | cut -d' ' -f1; }

venv_current() { [ -x "$AIHEALTH" ] && [ -f "$STAMP" ] && [ "$(cat "$STAMP")" = "$(pin_hash)" ]; }

cmd_setup() {
  gs_mise_env
  if venv_current; then return 0; fi
  if ! command -v uv >/dev/null 2>&1; then gs_unmet code-health uv; fi
  mkdir -p "$CH_DIR"
  rm -rf "$VENV" "$STAMP"
  # The lock holds every PyPI dependency with hashes; the ai-health git requirement cannot carry a hash,
  # so it is installed on its own, pinned to a commit, without dependencies.
  # https://docs.astral.sh/uv/pip/compile/#including-hashes
  uv venv --python 3.12 "$VENV"
  uv pip sync --python "$PY" --require-hashes tools/code-health/aihealth.lock.txt
  local rev
  rev="$(sed -n 's/.*@\([0-9a-f]\{40\}\).*/\1/p' tools/code-health/aihealth.in | head -n 1)"
  uv pip install --python "$PY" --no-deps "git+https://github.com/epatel/ai-health@$rev"
  "$PY" -c "from tree_sitter_language_pack import download; download([$GRAMMARS])"
  pin_hash >"$STAMP"
}

# Gate and report calls: set up on demand (CI has no `just setup`), skip or fail when that is impossible.
ensure() {
  cmd_setup
  if ! venv_current; then gs_unmet code-health "the venv (run 'just setup')"; fi
}

# `aihealth` against a given root: aihealth_in <root> <component> <subcommand-args...>
profile_of() { echo "$ROOT/tools/code-health/$1.toml"; }

# LoC-weighted score of a component under <root>, from the JSON of one scan per path.
component_score() {
  local root="$1" comp="$2" out="" p
  for p in $(paths_of "$comp"); do
    [ -d "$root/$p" ] || continue
    out="$out $("$AIHEALTH" scan "$root/$p" --config "$(profile_of "$comp")" --format json | "$PY" -c \
      'import json,sys; d=json.load(sys.stdin); print(d["score"], d["loc"])')"
  done
  # shellcheck disable=SC2086 # word splitting of "score loc score loc" is the point
  "$PY" -c 'import sys; a=sys.argv[1:]; w=sum(float(a[i+1]) for i in range(0,len(a),2)); print(f"{sum(float(a[i])*float(a[i+1]) for i in range(0,len(a),2))/w:.2f}" if w else "10.00")' $out
}

cmd_scan() {
  local comp_filter="" json=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --component) comp_filter="$2"; shift 2 ;;
      --json) json=1; shift ;;
      *) echo "scan: unknown argument $1" >&2; exit 2 ;;
    esac
  done
  ensure
  local comp p fmt=text
  [ "$json" = 1 ] && fmt=json
  local first=1
  [ "$json" = 1 ] && echo '['
  for comp in $COMPONENTS; do
    [ -z "$comp_filter" ] || [ "$comp_filter" = "$comp" ] || continue
    if [ "$json" = 1 ]; then
      for p in $(paths_of "$comp"); do
        [ "$first" = 1 ] || echo ','
        first=0
        "$AIHEALTH" scan "$p" --config "$(profile_of "$comp")" --format json |
          "$PY" -c 'import json,sys; d=json.load(sys.stdin); d["component"]=sys.argv[1]; print(json.dumps(d))' "$comp"
      done
    else
      echo "== $comp ($(paths_of "$comp")): score $(component_score "$ROOT" "$comp") =="
      for p in $(paths_of "$comp"); do
        "$AIHEALTH" scan "$p" --config "$(profile_of "$comp")" --format "$fmt" --top 5
      done
    fi
  done
  if [ "$json" = 1 ]; then
    echo ']'
  elif [ -z "$comp_filter" ] || [ "$comp_filter" = android ]; then
    echo "android: not yet scored (Kotlin adapter pending)"
  fi
}

# Delta per component. $1 = base rev or "--staged"; $2 = "check" to fail on any degradation.
run_delta() {
  local base="$1" mode="$2" comp p rc=0 args
  for comp in $COMPONENTS; do
    if [ "$base" = "--staged" ]; then
      # shellcheck disable=SC2046,SC2086 # word-split paths
      git diff --cached --name-only --diff-filter=ACMR -- $(paths_of "$comp") | grep -qE "$(ext_re_of "$comp")" || continue
      args=(--staged)
    else
      args=(--base "$base")
    fi
    echo "== code health delta: $comp =="
    # shellcheck disable=SC2046,SC2086 # word-split paths
    "$AIHEALTH" delta "${args[@]}" --config "$(profile_of "$comp")" $(paths_of "$comp") || rc=$?
  done
  [ "$mode" = check ] && return "$rc"
  return 0
}

merge_base() { git merge-base "${1:-origin/main}" HEAD; }

cmd_changed() {
  local base="origin/main" mode=report
  while [ $# -gt 0 ]; do
    case "$1" in
      --check) mode=check; shift ;;
      *) base="$1"; shift ;;
    esac
  done
  ensure
  run_delta "$(merge_base "$base")" "$mode"
}

cmd_staged() {
  local mode=report
  [ "${1:-}" = "--check" ] && mode=check
  # A hook before `just setup` must not block commits: skip with a message instead of installing.
  if ! venv_current; then
    echo "code-health skipped: run 'just setup' to install ai-health"
    return 0
  fi
  run_delta --staged "$mode"
}

# Fails when a component's scan score drops by more than $SCORE_TOLERANCE against the merge base.
cmd_compare() {
  local base_sha="$1" comp rc=0 before after
  rm -rf "$CH_DIR/base"
  mkdir -p "$CH_DIR/base"
  trap 'rm -rf "$CH_DIR/base"' RETURN
  git archive "$base_sha" crates apps packages | tar -x -C "$CH_DIR/base"
  for comp in $COMPONENTS; do
    before="$(component_score "$CH_DIR/base" "$comp")"
    after="$(component_score "$ROOT" "$comp")"
    echo "component $comp: $before (merge base ${base_sha:0:8}) -> $after (head)"
    if "$PY" -c 'import sys; sys.exit(0 if float(sys.argv[2]) >= float(sys.argv[1]) - float(sys.argv[3]) else 1)' \
      "$before" "$after" "$SCORE_TOLERANCE"; then :; else
      echo "FAILED: $comp score fell from $before to $after (allowed drop $SCORE_TOLERANCE)" >&2
      rc=1
    fi
  done
  return "$rc"
}

# ai-health ignores a suppression without " -- <reason>" silently, so a bare marker would pass unnoticed.
# Syntax and examples live in docs/contributing/code-health.md, hence the excluded paths.
cmd_suppressions() {
  local hits
  hits="$(git grep -nP 'aihealth:[ \t]*disable=(?![\w, \t]+-- .{10})' -- . \
    ':!openspec' ':!docs' ':!*.md' ':!scripts/lib/code-health*' ':!tools/code-health' ':!.claude' || true)"
  if [ -n "$hits" ]; then
    echo "FAILED: suppression without a reason of at least 10 characters (disable=<smell> -- <reason>):" >&2
    echo "$hits" >&2
    return 1
  fi
}

cmd_check() {
  local base="${1:-origin/main}" sha rc=0
  cmd_suppressions || rc=1
  ensure
  sha="$(merge_base "$base")"
  run_delta "$sha" check || rc=1
  cmd_compare "$sha" || rc=1
  return "$rc"
}

# `just code-health [--component C] [--json] [--changed [BASE]] [--staged] [--check]`
cmd_cli() {
  local changed=0 staged=0 check=0 base="" rest=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --changed) changed=1; shift; case "${1:-}" in "" | --*) ;; *) base="$1"; shift ;; esac ;;
      --staged) staged=1; shift ;;
      --check) check=1; shift ;;
      *) rest+=("$1"); shift ;;
    esac
  done
  if [ "$staged" = 1 ]; then
    if [ "$check" = 1 ]; then cmd_staged --check; else cmd_staged; fi
  elif [ "$changed" = 1 ] && [ "$check" = 1 ]; then
    cmd_check "${base:-origin/main}"
  elif [ "$changed" = 1 ]; then
    cmd_changed "${base:-origin/main}"
  elif [ "$check" = 1 ]; then
    cmd_check origin/main
  else
    cmd_scan "${rest[@]}"
  fi
}

sub="${1:-}"
[ $# -gt 0 ] && shift
case "$sub" in
  cli) cmd_cli "$@" ;;
  setup) cmd_setup ;;
  scan) cmd_scan "$@" ;;
  changed) cmd_changed "$@" ;;
  staged) cmd_staged "$@" ;;
  check) cmd_check "$@" ;;
  suppressions) cmd_suppressions ;;
  *) echo "usage: $0 setup|scan|changed|staged|check|suppressions" >&2; exit 2 ;;
esac
