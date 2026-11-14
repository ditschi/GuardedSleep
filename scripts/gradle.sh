#!/usr/bin/env bash
# Runs ./gradlew for apps/android inside the guardedsleep-android-native-build image (host needs only
# Docker). Two flocks serialise container runs, taken in this order, each bounded by GS_SLOT_WAIT
# (default 3600 s, then exit 75 like scripts/lib/slot.sh):
# - target/.gradle.lock (per worktree): parallel Gradle runs on one project dir corrupt its caches.
# - <shared cache>/gradle/.gs-container.lock (machine-wide): the Gradle user home is shared between
#   worktrees (build-image.sh), and Gradle's own cross-process cache locks do not work across
#   containers. A waiting Gradle asks the lock owner to release over a loopback UDP port and names it
#   by PID; every container has its own network and PID namespace, so the owner never hears it and the
#   waiter fails after 60 s with "Timeout waiting to lock journal cache". Container runs that share the
#   home therefore take turns. https://docs.gradle.org/current/userguide/directory_layout.html
# The container builds the core into target/container (as scripts/core-build.sh host|android does), so the
# JVM tests find the library built in the same environment and the host's target/ stays untouched:
# jna.library.path in apps/android/{core-bindings,platform}/build.gradle.kts follows CARGO_TARGET_DIR.
# docs/contributing/development.md, "Rust execution mode".
#   scripts/gradle.sh :platform:testDebugUnitTest :app:assembleDebug
# Inside CI or the image itself, set CORE_BUILD_NATIVE=1 to run without Docker.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$ROOT/target"

# Commit identity for BuildConfig (Settings -> About). Computed on the host: in a git worktree .git is
# a file pointing outside the /workspace mount, so git inside the container is unreliable.
if [[ -z "${GS_GIT_COMMIT:-}" ]]; then
	GS_GIT_COMMIT="$(git -C "$ROOT" rev-parse --short=8 HEAD 2>/dev/null || true)"
fi
if [[ -z "${GS_GIT_COMMIT_TIME:-}" ]]; then
	GS_GIT_COMMIT_TIME="$(TZ=UTC git -C "$ROOT" log -1 --format=%cd --date=format-local:'%Y-%m-%d %H:%M UTC' 2>/dev/null || true)"
fi
export GS_GIT_COMMIT GS_GIT_COMMIT_TIME

if [[ "${CORE_BUILD_NATIVE:-0}" != "1" ]]; then
	# shellcheck source=lib/build-image.sh
	source "$ROOT/scripts/lib/build-image.sh"
	gs_ensure_build_image
	gs_lock() {
		if ! flock -n "$1"; then
			echo "waiting for $2" >&2
			if ! flock -w "${GS_SLOT_WAIT:-3600}" "$1"; then
				echo "error: $2 stayed busy for ${GS_SLOT_WAIT:-3600}s (GS_SLOT_WAIT); is a job stuck? try 'just reap'" >&2
				exit 75
			fi
		fi
	}
	exec 7>"$ROOT/target/.gradle.lock" 8>"${GS_SHARED_CACHE:-$HOME/.cache/guardedsleep}/gradle/.gs-container.lock"
	gs_lock 7 "another Gradle run in this worktree"
	gs_lock 8 "the shared Gradle home (a container Gradle run, possibly in another worktree, holds it)"
	exec docker run --rm \
		"${GS_DOCKER_USER_ARGS[@]}" "${GS_DOCKER_CACHE_ARGS[@]}" \
		-e CORE_BUILD_NATIVE=1 \
		-e GS_GIT_COMMIT -e GS_GIT_COMMIT_TIME \
		-e CARGO_TARGET_DIR=/workspace/target/container \
		-v "$ROOT":/workspace -w /workspace \
		"$GS_BUILD_IMAGE" bash scripts/gradle.sh "$@"
fi

cd "$ROOT/apps/android"
exec ./gradlew --no-daemon "$@"
