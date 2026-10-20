#!/usr/bin/env bash
# ADR-016: test-only surfaces must not ship. Fails if the release APK still contains the debug launch activity, the audio
# injection receiver class or its intent action (manifest or dex).
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

found=0
for needle in "${FORBIDDEN[@]}"; do
	# Dex stores UTF-8 (MUTF-8); the binary manifest stores UTF-16LE.
	utf16="$(printf '%s' "$needle" | iconv -f UTF-8 -t UTF-16LE | od -An -tx1 | tr -d ' \n')"
	if grep -aqF "$needle" "$TMP"/classes*.dex || od -An -tx1 "$TMP/AndroidManifest.xml" | tr -d ' \n' | grep -q "$utf16"; then
		echo "FAIL release-check — '$needle' present in release APK"
		found=1
	fi
done
[[ $found -eq 0 ]] && echo "PASS release-check — no test-only surfaces in release APK"
exit $found
