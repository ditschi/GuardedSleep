#!/usr/bin/env bash
# Prerequisite handling for the just recipes (ADR-020 §1).
#
# Source it from a recipe or script:
#   source scripts/lib/require.sh
#   require gate kvm docker     # <task> <requirement>...
#
# All requirements met  -> returns 0.
# One is missing        -> prints "SKIPPED: <task> needs <x>" and exits 0, so the caller stops here.
# With GS_STRICT=1 (CI) -> prints "FAILED: <task> needs <x>" and exits 1 instead.
# A skip is also appended to $GS_SKIP_LOG when set, so `just ci` can list them at the end (skip-log.sh).
# Requirements: docker, kvm, android-sdk, cargo, cargo-fuzz, network.
#
# Sourcing also puts the mise-managed tools on PATH when mise is installed (see gs_mise_env).

# shellcheck source=skip-log.sh
source "$(dirname "${BASH_SOURCE[0]}")/skip-log.sh"

gs_mise_env() {
  if command -v mise >/dev/null 2>&1; then
    # `mise env` prints `export PATH=...` for the tools pinned in mise.toml.
    # https://mise.jdx.dev/cli/env.html
    eval "$(mise env -s bash 2>/dev/null)" || true
  elif [ -z "${GS_MISE_HINT_SHOWN:-}" ]; then
    echo "hint: mise not found, using tools from PATH. Install mise (https://mise.jdx.dev) and run 'just setup' for the pinned versions." >&2
    export GS_MISE_HINT_SHOWN=1
  fi
}

gs_missing() {
  case "$1" in
    docker) docker info >/dev/null 2>&1 || echo "docker" ;;
    kvm) { [ -r /dev/kvm ] && [ -w /dev/kvm ]; } || echo "kvm" ;;
    android-sdk)
      # The JVM tests run in the build image unless the SDK is already on the host or we are in it.
      [ -d "${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/nonexistent}}" ] || [ "${CORE_BUILD_NATIVE:-0}" = "1" ] \
        || docker info >/dev/null 2>&1 || echo "android-sdk"
      ;;
    cargo)
      # scripts/cargo.sh falls back to the build image, so docker is an acceptable substitute.
      command -v cargo >/dev/null 2>&1 || docker info >/dev/null 2>&1 || echo "cargo"
      ;;
    # cargo-fuzz runs on a rustup-managed nightly (tools/nightly), so both must be on the host.
    cargo-fuzz) { command -v cargo-fuzz >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1; } || echo "cargo-fuzz (and rustup)" ;;
    # The relay tools talk to public hosts; a name that does not resolve means no usable network.
    network) timeout 5 getent hosts github.com >/dev/null 2>&1 || echo "network" ;;
    *) echo "$1 (unknown requirement)" ;;
  esac
}

require() {
  local task="$1" missing="" m req
  shift
  for req in "$@"; do
    m="$(gs_missing "$req")"
    [ -n "$m" ] && missing="${missing:+$missing, }$m"
  done
  [ -z "$missing" ] && return 0
  if [ "${GS_STRICT:-0}" = "1" ]; then
    echo "FAILED: $task needs $missing (GS_STRICT=1)" >&2
    exit 1
  fi
  gs_skip "$task needs $missing"
  exit 0
}

gs_mise_env

# Executed directly instead of sourced: scripts/lib/require.sh <task> <requirement>...
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  [ $# -ge 2 ] || { echo "usage: $0 <task> <requirement>..." >&2; exit 2; }
  require "$@"
fi
