#!/usr/bin/env bash
# Runs ./gradlew for apps/android inside the guardedsleep-android-native-build image (host needs only
# Docker). Concurrent invocations are serialised with flock on target/.gradle.lock because parallel
# Gradle runs on one project dir corrupt each other's caches. The Gradle user home is shared between
# worktrees (build-image.sh): Gradle locks it across processes, so only the project dir needs the flock.
# The container builds the core into target/container (as scripts/core-build.sh host|android does), so the
# JVM tests find the library built in the same environment and the host's target/ stays untouched:
# jna.library.path in apps/android/{core-bindings,platform}/build.gradle.kts follows CARGO_TARGET_DIR.
# docs/contributing/development.md, "Rust execution mode".
#   scripts/gradle.sh :platform:testDebugUnitTest :app:assembleDebug
# Inside CI or the image itself, set CORE_BUILD_NATIVE=1 to run without Docker.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$ROOT/target"

if [[ "${CORE_BUILD_NATIVE:-0}" != "1" ]]; then
	# shellcheck source=lib/build-image.sh
	source "$ROOT/scripts/lib/build-image.sh"
	gs_ensure_build_image
	exec flock "$ROOT/target/.gradle.lock" docker run --rm \
		"${GS_DOCKER_USER_ARGS[@]}" "${GS_DOCKER_CACHE_ARGS[@]}" \
		-e CORE_BUILD_NATIVE=1 \
		-e CARGO_TARGET_DIR=/workspace/target/container \
		-v "$ROOT":/workspace -w /workspace \
		"$GS_BUILD_IMAGE" bash scripts/gradle.sh "$@"
fi

cd "$ROOT/apps/android"
exec ./gradlew --no-daemon "$@"
