#!/usr/bin/env bash
# cargo-deny for the workspace (deny.toml) and the per-crate sans-IO bans (deny.core.toml), plus the
# dependency-count guard for guardedsleep-core. cargo-deny needs `cargo metadata`, so without a host
# cargo it runs inside the build image, with the mise-installed cargo-deny binary mounted in.
#   scripts/lib/deny.sh            # everything
#   scripts/lib/deny.sh advisories # only the given cargo-deny checks for deny.toml
# Format reference: https://embarkstudios.github.io/cargo-deny/
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
# shellcheck source=require.sh
source scripts/lib/require.sh
require deny cargo

CHECKS=("$@")
[ ${#CHECKS[@]} -gt 0 ] || CHECKS=(advisories bans licenses sources)

deny() {
  if command -v cargo >/dev/null 2>&1; then
    cargo deny "$@"
    return
  fi
  local bin image="${CORE_BUILD_IMAGE:-guardedsleep-android-native-build:local}"
  bin="$(command -v cargo-deny || true)"
  [ -n "$bin" ] || { echo "cargo-deny not found: run 'just setup' (mise install)" >&2; exit 1; }
  docker image inspect "$image" >/dev/null 2>&1 || docker build -t "$image" docker/android-native-build
  mkdir -p target/.cargo-home
  # The image has no VCS client, so the advisory database is fetched on the host into the shared CARGO_HOME.
  CARGO_HOME="$ROOT/target/.cargo-home" cargo-deny fetch db
  docker run --rm -e HOME=/workspace/target/.cargo-home \
    -e CARGO_HOME=/workspace/target/.cargo-home \
    -e CARGO_TARGET_DIR=/workspace/target \
    -v "$ROOT":/workspace -v "$bin":/usr/local/bin/cargo-deny:ro -w /workspace \
    "$image" cargo-deny "$@"
}

OFFLINE=()
command -v cargo >/dev/null 2>&1 || OFFLINE=(--offline)

deny "${OFFLINE[@]}" check "${CHECKS[@]}"

for manifest in crates/guardedsleep-core/Cargo.toml crates/guardedsleep-relay/Cargo.toml; do
  [ -f "$manifest" ] || continue
  deny "${OFFLINE[@]}" --manifest-path "$manifest" --config deny.core.toml check bans
done

scripts/lib/core-dep-count.sh
