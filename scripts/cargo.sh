#!/usr/bin/env bash
# Runs cargo on the host when rustup is on PATH (so rust-toolchain.toml applies; a distro cargo would
# ignore it), otherwise inside the guardedsleep-android-native-build image (host needs only Docker).
# The image fallback writes target/, not target/container: without host rustup there is no host build
# whose fingerprints it could invalidate, and scripts/dev.sh, scripts/poc/env-up.sh and the web dev
# server read target/debug. docs/contributing/development.md, "Rust execution mode".
# Usage: scripts/cargo.sh run -q -p i18n-gen -- check
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=lib/host-rust.sh
source "$ROOT/scripts/lib/host-rust.sh"
if gs_host_rust; then
	cd "$ROOT"
	exec cargo "$@"
fi
# shellcheck source=lib/build-image.sh
source "$ROOT/scripts/lib/build-image.sh"
gs_ensure_build_image
exec docker run --rm \
	"${GS_DOCKER_USER_ARGS[@]}" "${GS_DOCKER_CACHE_ARGS[@]}" \
	-e CARGO_TARGET_DIR=/workspace/target \
	-v "$ROOT":/workspace -w /workspace \
	"$GS_BUILD_IMAGE" cargo "$@"
