#!/usr/bin/env bash
# AC-6 (ADR-015): the Rust core may add at most CORE_MAX_BYTES (15 MB) to the debug APK, counted
# as the stripped libguardedsleep_core.so for every packaged ABI.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
JNI="$ROOT/apps/android/core/src/main/jniLibs"
APK="${APK:-$ROOT/apps/android/app/build/outputs/apk/debug/app-debug.apk}"
CORE_MAX_BYTES="${CORE_MAX_BYTES:-15728640}"

total=0
for so in "$JNI"/*/libguardedsleep_core.so; do
	size=$(stat -c %s "$so")
	echo "$(basename "$(dirname "$so")"): $size bytes"
	total=$((total + size))
done
echo "core total: $total bytes (limit $CORE_MAX_BYTES)"
[[ -f "$APK" ]] && echo "debug APK: $(stat -c %s "$APK") bytes"
if [[ $total -gt $CORE_MAX_BYTES ]]; then
	echo "FAIL AC-6 — core adds $total bytes"
	exit 1
fi
echo "PASS AC-6 — core adds $total bytes"
