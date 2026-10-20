#!/usr/bin/env bash
# Run adb inside the emulator container. Host repo paths in arguments (e.g. the APK for
# `adb install`) are rewritten to the container's repo mount, which compose.yaml defines once.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
compose="$repo/docker/android-emulator/compose.yaml"

mount="$(docker compose -f "$compose" exec -T emulator printenv GUARDEDSLEEP_REPO_MOUNT | tr -d '\r' || true)"
if [[ -z "$mount" ]]; then
	echo "adb.sh: GUARDEDSLEEP_REPO_MOUNT not set in the emulator container; recreate it (pnpm run emulator:up)." >&2
	exit 1
fi

args=()
for arg in "$@"; do
	if [[ "$arg" == "$repo" || "$arg" == "$repo/"* ]]; then
		arg="$mount${arg#"$repo"}"
	fi
	args+=("$arg")
done

exec docker compose -f "$compose" exec -T emulator adb "${args[@]}"
