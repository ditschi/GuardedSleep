#!/usr/bin/env bash
# Runs ./gradlew for apps/android inside the guardedsleep-android-native-build image (host needs only
# Docker). Concurrent invocations are serialised with flock on target/.gradle.lock because parallel
# Gradle runs on one project dir corrupt each other's caches.
#   scripts/gradle.sh :core:testDebugUnitTest :app:assembleDebug
# Inside CI or the image itself, set CORE_BUILD_NATIVE=1 to run without Docker.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="${CORE_BUILD_IMAGE:-guardedsleep-android-native-build:local}"
mkdir -p "$ROOT/target"

if [[ "${CORE_BUILD_NATIVE:-0}" != "1" ]]; then
	if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
		docker build -t "$IMAGE" "$ROOT/docker/android-native-build"
	fi
	exec flock "$ROOT/target/.gradle.lock" docker run --rm \
		-e CORE_BUILD_NATIVE=1 \
		-e GRADLE_USER_HOME=/workspace/target/.gradle-home \
		-v "$ROOT":/workspace -w /workspace \
		"$IMAGE" bash scripts/gradle.sh "$@"
fi

cd "$ROOT/apps/android"
exec ./gradlew --no-daemon "$@"
