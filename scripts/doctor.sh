#!/usr/bin/env bash
# `just doctor`: which prerequisites of the just recipes this machine has, with a one-line fix for each
# missing one. Informational: it always exits 0 and runs no builds. Checks come from the same helpers the
# recipes use (scripts/lib/require.sh, playwright-ready.sh, hooks-check.sh). A missing optional item only
# means the matching recipe prints SKIPPED (a skip is not a pass; CI sets GS_STRICT=1).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
# shellcheck source=lib/require.sh
source scripts/lib/require.sh
# shellcheck source=lib/playwright-ready.sh
source scripts/lib/playwright-ready.sh

ok() { printf 'ok       %-14s %s\n' "$1" "$2"; }
missing() { printf 'MISSING  %-14s %s\n         fix: %s\n' "$1" "$2" "$3"; }
note() { printf 'note     %-14s %s\n' "$1" "$2"; }
have() { command -v "$1" >/dev/null 2>&1; }

echo "GuardedSleep prerequisites (informational, exit status is always 0)"
echo ""

# mise pins every tool (node, pnpm, just, lefthook, ...). `mise ls --missing` lists pinned tools that are not installed.
if have mise; then
  gone="$(timeout 20 mise ls --missing 2>/dev/null | awk 'NF { print $1 }' | paste -sd, -)"
  if [ -z "$gone" ]; then
    ok mise "$(timeout 10 mise --version 2>/dev/null | head -1), all tools of mise.toml installed"
  else
    missing mise "tools not installed: $gone" "mise install   (or: just setup)"
  fi
else
  missing mise "not found; the pinned tools (node, pnpm, just, ...) come from it" "https://mise.jdx.dev/getting-started.html, then: just setup"
fi

if have rustup && have cargo; then
  ok rustup "$(timeout 10 cargo --version 2>/dev/null)"
elif have cargo; then
  note rustup "cargo found without rustup: rust-toolchain.toml (pinned stable, targets) is not applied"
else
  missing rustup "no host cargo (scripts/cargo.sh falls back to the Docker build image)" "https://rustup.rs, then: just setup"
fi

if [ -z "$(gs_missing docker)" ]; then
  if timeout 10 docker info -f '{{.SecurityOptions}}' 2>/dev/null | grep -q rootless; then
    ok docker "reachable, rootless"
    note docker "Dev Container / compose runs need DEV_UID=0 DEV_GID=0 (.devcontainer/.env.example, docs/contributing/development.md)"
  else
    ok docker "reachable, rootful (the build image runs as your uid)"
  fi
else
  missing docker "daemon not reachable (needed for the build image, api-check, gate, stack)" "install Docker, start it, and check 'docker info'"
fi

# reuse (licence lint in `just lint-text`) is a Python tool that mise installs through its pipx backend on the
# uv pinned in mise.toml, so uv must come from mise (no host uv needed).
if have uv; then
  ok uv "$(timeout 10 uv --version 2>/dev/null)"
else
  missing uv "mise-installed uv not found; mise cannot install reuse (pipx:reuse)" "mise install"
fi

if have pnpm; then
  if gs_playwright_ready; then
    ok playwright "Chromium installed (just e2e)"
  else
    missing playwright "Chromium not installed; just e2e prints SKIPPED" "pnpm --filter @guardedsleep/e2e exec playwright install --with-deps chromium"
  fi
else
  missing playwright "pnpm not found (comes from mise), so the browser cannot be checked" "mise install"
fi

if [ -z "$(gs_missing kvm)" ]; then
  ok kvm "/dev/kvm readable and writable (just gate)"
else
  missing kvm "no usable /dev/kvm; just gate prints SKIPPED" "bash scripts/check-kvm.sh explains why (enable VT-x/AMD-V, add yourself to the kvm group)"
fi

sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [ -n "$sdk" ] && [ -d "$sdk" ]; then
  ok android-sdk "$sdk"
elif [ -z "$(gs_missing docker)" ]; then
  note android-sdk "no host SDK; Gradle runs in the build image through Docker"
else
  missing android-sdk "no host SDK and no Docker for the build image" "install Docker, or point ANDROID_HOME at an SDK (see docs/contributing/development.md)"
fi

if [ -z "$(gs_missing adb)" ]; then
  ok adb "$(command -v adb) (just install-android)"
else
  note adb "not found; just install-android prints SKIPPED (install Android platform-tools or add \$ANDROID_HOME/platform-tools to PATH)"
fi

hooks="$(scripts/lib/hooks-check.sh 2>&1 || true)"
if [ -z "$hooks" ]; then
  ok hooks "lefthook pre-commit hook active"
else
  missing hooks "${hooks#warning: }" "just setup"
fi

exit 0
