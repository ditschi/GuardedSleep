#!/usr/bin/env bash
# Runs cargo on the host when available, otherwise inside the guardedsleep-android-native-build image
# (host needs only Docker). Usage: scripts/cargo.sh run -q -p i18n-gen -- check
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if command -v cargo >/dev/null 2>&1; then
	cd "$ROOT"
	exec cargo "$@"
fi
IMAGE="${CORE_BUILD_IMAGE:-guardedsleep-android-native-build:local}"
TOOLCHAIN_HASH="$(sha256sum "$ROOT/rust-toolchain.toml" | cut -c1-16)"
# (Re)builds the image when it is missing or was built from a different rust-toolchain.toml.
if [[ "$(docker image inspect -f '{{ index .Config.Labels "gs.toolchain" }}' "$IMAGE" 2>/dev/null)" != "$TOOLCHAIN_HASH" ]]; then
	docker build --build-context repo="$ROOT" --label "gs.toolchain=$TOOLCHAIN_HASH" -t "$IMAGE" "$ROOT/docker/android-native-build"
fi
exec docker run --rm \
	-e CARGO_HOME=/workspace/target/.cargo-home \
	-e CARGO_TARGET_DIR=/workspace/target \
	-v "$ROOT":/workspace -w /workspace \
	"$IMAGE" cargo "$@"
