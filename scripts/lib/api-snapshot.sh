#!/usr/bin/env bash
# API-surface snapshots of the two binding facades (ADR-020 section 3):
#   crates/guardedsleep-ffi/api.txt        cargo public-api of guardedsleep-ffi
#   crates/guardedsleep-wasm/api.txt       cargo public-api of guardedsleep-wasm
#   packages/core-wasm/api/core.d.ts       the TypeScript definitions wasm-bindgen + tsify generate
# A change to the surface the platforms see therefore shows up as a reviewed diff.
#   scripts/lib/api-snapshot.sh check     # fail on a diff (part of `just lint`)
#   scripts/lib/api-snapshot.sh update    # rewrite the snapshots (`just api-snapshot`)
#
# cargo-public-api reads rustdoc JSON, which only a nightly compiler emits and whose format changes with
# every nightly, so the nightly is pinned in tools/nightly/rust-toolchain.toml and cargo-public-api in
# the [vars] table of mise.toml (a cargo: backend in [tools] would need a host cargo). Without a host
# toolchain it all runs in a stock rust image (cargo-public-api needs libssl-dev, which the build image
# lacks); the nightly and the binary are cached under target/.api.
# cargo-public-api: https://github.com/cargo-public-api/cargo-public-api (compatibility table in README)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

# Re-entry point: runs where rustup and the pinned nightly are available (the container or the host).
if [ "${API_SNAPSHOT_NATIVE:-0}" = "1" ]; then
  out="$1"
  nightly="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' tools/nightly/rust-toolchain.toml)"
  version="$(sed -n 's/^cargo_public_api *= *"\(.*\)"/\1/p' mise.toml)"
  rustup toolchain install "$nightly" --profile minimal >&2
  # target/.api is inside CI's restored and pruned target/ cache, so a toolchain can come back without its
  # manifest ("missing manifest in toolchain"), which rustup reports as installed. Reinstall it then.
  if ! rustup run "$nightly" rustc --version >/dev/null 2>&1; then
    rustup toolchain uninstall "$nightly" >&2 || rm -rf "${RUSTUP_HOME:-$HOME/.rustup}/toolchains/$nightly-"*
    rustup toolchain install "$nightly" --profile minimal >&2
  fi
  export RUSTUP_TOOLCHAIN="$nightly"
  if ! cargo public-api --version 2>/dev/null | grep -q "$version"; then
    cargo install --locked cargo-public-api --version "$version" >&2
  fi
  mkdir -p "$out"
  for crate in guardedsleep-ffi guardedsleep-wasm; do
    cargo public-api --manifest-path "crates/$crate/Cargo.toml" -sss >"$out/$crate.api.txt"
  done
  exit 0
fi

MODE="${1:-check}"
case "$MODE" in check | update) ;; *)
  echo "usage: $0 check|update" >&2
  exit 2
  ;;
esac

# shellcheck source=require.sh
source scripts/lib/require.sh
require api-check docker

out="$ROOT/target/api-out"
rm -rf "$out"
mkdir -p "$out"

# The generated .d.ts comes from the wasm build (incremental when nothing changed).
scripts/core-build.sh wasm >&2
cp target/wasm-pkg/web/guardedsleep_wasm.d.ts "$out/core.d.ts"

nightly="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' tools/nightly/rust-toolchain.toml)"
if command -v cargo >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1 \
  && rustup toolchain list | grep -q "^$nightly"; then
  API_SNAPSHOT_NATIVE=1 CARGO_TARGET_DIR="$ROOT/target/api" "$0" "$out"
else
  # Same digest as docker/web/Dockerfile (the custom manager in renovate.json keeps this copy in step).
  image="rust:1-bookworm@sha256:665b6e0a9290a7298236c08c6e4f1e754e3f0d9b70421c434daa3dfba888d63e"
  user_args=()
  if ! docker info -f '{{.SecurityOptions}}' 2>/dev/null | grep -q rootless; then
    user_args=(--user "$(id -u):$(id -g)")
  fi
  mkdir -p target/.api/home
  docker run --rm "${user_args[@]}" \
    -e HOME=/workspace/target/.api/home \
    -e RUSTUP_HOME=/workspace/target/.api/rustup \
    -e CARGO_HOME=/workspace/target/.api/cargo \
    -e CARGO_TARGET_DIR=/workspace/target/api \
    -e API_SNAPSHOT_NATIVE=1 \
    -v "$ROOT":/workspace -w /workspace \
    "$image" bash -c 'mkdir -p "$CARGO_HOME/bin" && cp --remove-destination /usr/local/cargo/bin/* "$CARGO_HOME/bin/"; export PATH="$CARGO_HOME/bin:$PATH"; exec scripts/lib/api-snapshot.sh /workspace/target/api-out'
fi

declare -A dest=(
  [guardedsleep-ffi.api.txt]=crates/guardedsleep-ffi/api.txt
  [guardedsleep-wasm.api.txt]=crates/guardedsleep-wasm/api.txt
  [core.d.ts]=packages/core-wasm/api/core.d.ts
)
status=0
for name in "${!dest[@]}"; do
  target="${dest[$name]}"
  if [ "$MODE" = "update" ]; then
    mkdir -p "$(dirname "$target")"
    cp "$out/$name" "$target"
    echo "api-snapshot: wrote $target"
  elif ! diff -u "$target" "$out/$name"; then
    status=1
  fi
done
if [ "$status" -ne 0 ]; then
  echo "api-snapshot: the binding surface changed. Review the diff above, then run: just api-snapshot" >&2
  exit 1
fi
[ "$MODE" = "check" ] && echo "api-snapshot: ok"
exit 0
