#!/usr/bin/env bash
# Runs cargo on the host when available, otherwise inside the
# guardedsleep-android-native-build image (host needs only Docker).
# Usage: scripts/cargo.sh run -q -p i18n-gen -- check
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if command -v cargo >/dev/null 2>&1; then
	cd "$ROOT"
	exec cargo "$@"
fi
# shellcheck source=lib/build-image.sh
source "$ROOT/scripts/lib/build-image.sh"
gs_ensure_build_image
exec docker run --rm \
	-e CARGO_HOME=/workspace/target/.cargo-home \
	-e CARGO_TARGET_DIR=/workspace/target \
	-v "$ROOT":/workspace -w /workspace \
	"$GS_BUILD_IMAGE" cargo "$@"
