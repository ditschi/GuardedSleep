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
  local bin
  bin="$(command -v cargo-deny || true)"
  [ -n "$bin" ] || { echo "cargo-deny not found: run 'just setup' (mise install)" >&2; exit 1; }
  # shellcheck source=build-image.sh
  source scripts/lib/build-image.sh
  gs_ensure_build_image
  mkdir -p target/.cargo-home
  # The build image ships a VCS client, so cargo-deny fetches the advisory database and the crates it
  # needs for `cargo metadata` itself, into the shared CARGO_HOME (needs network).
  docker run --rm "${GS_DOCKER_USER_ARGS[@]}" -e HOME=/workspace/target/.cargo-home \
    -e CARGO_HOME=/workspace/target/.cargo-home \
    -e CARGO_TARGET_DIR=/workspace/target \
    -v "$ROOT":/workspace -v "$bin":/usr/local/bin/cargo-deny:ro -w /workspace \
    "$GS_BUILD_IMAGE" cargo-deny "$@"
}

deny check "${CHECKS[@]}"

# The bans and the dependency count look at the default-feature graph of the core. Today the `uniffi`
# feature (bindgen-time tempfile -> getrandom) lives in guardedsleep-core and would trip the ban, so the
# shipped binding graphs are not checked here. Wave 2 moves uniffi into guardedsleep-ffi; the core graph
# is then checked with --all-features (docs/ci-gates.md).
for manifest in crates/guardedsleep-core/Cargo.toml crates/guardedsleep-relay/Cargo.toml; do
  [ -f "$manifest" ] || continue
  deny --manifest-path "$manifest" --config deny.core.toml check bans
done

scripts/lib/core-dep-count.sh
