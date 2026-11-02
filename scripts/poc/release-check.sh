#!/usr/bin/env bash
# ADR-016: test-only surfaces must not ship. Fails if the release APK still contains the debug launch activity, the audio
# injection receiver class or its intent action (manifest or dex), or if the merged manifest names a permission that is
# not in scripts/poc/release-permissions.txt (a dependency update must not add one silently).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APK="${RELEASE_APK:-$ROOT/apps/android/app/build/outputs/apk/release/app-release-unsigned.apk}"
FORBIDDEN=("DebugLaunchActivity" "DebugInjectReceiver" "org.guardedsleep.app.alpha.debug.INJECT_AUDIO")

[[ -f "$APK" ]] || {
	echo "release-check: $APK not found (run :app:assembleRelease)" >&2
	exit 2
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
unzip -q -o "$APK" 'classes*.dex' AndroidManifest.xml -d "$TMP"
ALLOWED="$ROOT/scripts/poc/release-permissions.txt"

found=0
for needle in "${FORBIDDEN[@]}"; do
	# Dex stores UTF-8 (MUTF-8); the binary manifest stores UTF-16LE.
	utf16="$(printf '%s' "$needle" | iconv -f UTF-8 -t UTF-16LE | od -An -tx1 | tr -d ' \n')"
	if grep -aqF "$needle" "$TMP"/classes*.dex || od -An -tx1 "$TMP/AndroidManifest.xml" | tr -d ' \n' | grep -q "$utf16"; then
		echo "FAIL release-check — '$needle' present in release APK"
		found=1
	fi
done

# Binary manifest strings are UTF-16LE; every permission name contains ".permission." or ends in "_PERMISSION".
actual="$(strings -el -n 6 "$TMP/AndroidManifest.xml" | grep -oE '[a-z][a-z0-9_.]*\.permission\.[A-Z0-9_]+|[a-z][a-z0-9_.]*\.[A-Z0-9_]+_PERMISSION' | sort -u)"
allowed="$(grep -vE '^\s*(#|$)' "$ALLOWED" | sort -u)"
extra="$(comm -23 <(printf '%s\n' "$actual") <(printf '%s\n' "$allowed"))"
if [[ -n "$extra" ]]; then
	echo "FAIL release-check — permissions not in release-permissions.txt:"
	printf '  %s\n' $extra
	found=1
fi
[[ $found -eq 0 ]] && echo "PASS release-check — no test-only surfaces, permissions match the allow-list"
exit $found
