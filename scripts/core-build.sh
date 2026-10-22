#!/usr/bin/env bash
# Builds and tests the Rust core (ADR-015) inside the guardedsleep-android-native-build image, so the
# host needs only Docker. Subcommands can be combined: test wasm host android
#   scripts/core-build.sh test            # cargo fmt/clippy/test (incl. golden vectors)
#   scripts/core-build.sh wasm            # target/wasm-pkg/node + target/wasm-pkg/web
#   scripts/core-build.sh host            # host library + Kotlin bindings (what the Android JVM unit tests load; no ABIs)
#   scripts/core-build.sh android         # host + jniLibs (arm64-v8a, x86_64) for apps/android
#   scripts/core-build.sh swift           # Swift bindings from the host ffi library into target/swift-bindings (iOS smoke, no Xcode)
#   scripts/core-build.sh determinism     # generate the Kotlin bindings and the wasm package twice, fail on differing hashes
# Inside CI or the image itself, set CORE_BUILD_NATIVE=1 to run without Docker.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_BINDGEN_VERSION="$("$ROOT/scripts/lib/wasm-bindgen-version.sh")"

if [[ "${CORE_BUILD_NATIVE:-0}" != "1" ]]; then
	# shellcheck source=lib/build-image.sh
	source "$ROOT/scripts/lib/build-image.sh"
	gs_ensure_build_image
	exec docker run --rm \
		"${GS_DOCKER_USER_ARGS[@]}" "${GS_DOCKER_CACHE_ARGS[@]}" \
		-e CORE_BUILD_NATIVE=1 \
		-e CARGO_TARGET_DIR=/workspace/target \
		-v "$ROOT":/workspace -w /workspace \
		"$GS_BUILD_IMAGE" bash scripts/core-build.sh "$@"
fi

cd "$ROOT"
export CARGO_HOME="${CARGO_HOME_OVERRIDE:-$ROOT/target/.cargo-home}"
export PATH="$CARGO_HOME/bin:/opt/cargo/bin:$PATH"
mkdir -p "$CARGO_HOME"

run_test() {
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace
}

ensure_wasm_bindgen() {
	if [[ "$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]]; then
		cargo install --locked wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION"
	fi
}

run_wasm() {
	ensure_wasm_bindgen
	cargo build -p guardedsleep-wasm --release --target wasm32-unknown-unknown
	local wasm_file=target/wasm32-unknown-unknown/release/guardedsleep_wasm.wasm
	wasm-bindgen --target nodejs --out-dir target/wasm-pkg/node "$wasm_file"
	wasm-bindgen --target web --out-dir target/wasm-pkg/web "$wasm_file"
	ls -l target/wasm-pkg/web/*.wasm
}

# Unstripped host build of guardedsleep-ffi: the Kotlin bindings are generated from its UniFFI metadata (the release profile
# strips those symbols), and the Android JVM unit tests load it through JNA (jna.library.path in
# apps/android/{core,app}/build.gradle.kts points at target/debug).
run_host() {
	local gen=apps/android/core/src/main/kotlin
	cargo build -p guardedsleep-ffi
	rm -rf "$gen/uniffi"
	cargo run -q -p uniffi-bindgen -- generate --library target/debug/libguardedsleep_ffi.so \
		--language kotlin --out-dir "$gen" --no-format
}

# Same NDK as docker/android-native-build/Dockerfile and the setup-android action. Runners ship other
# NDK versions and export their path as the NDK home, so the pinned one wins when it is installed.
PINNED_NDK=30.0.16248370

run_android() {
	local jni=apps/android/core/src/main/jniLibs
	if [[ -d "${ANDROID_HOME:-}/ndk/$PINNED_NDK" ]]; then
		export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$PINNED_NDK"
	fi
	cargo ndk -t arm64-v8a -t x86_64 -P 26 -o "$jni" build -p guardedsleep-ffi --release
	run_host
	ls -l "$jni"/*/libguardedsleep_ffi.so
}

# The Swift smoke check (weekly workflow): generation must keep working on Linux although no iOS client exists yet.
run_swift() {
	local out=target/swift-bindings
	cargo build -p guardedsleep-ffi
	rm -rf "$out"
	cargo run -q -p uniffi-bindgen -- generate --library target/debug/libguardedsleep_ffi.so \
		--language swift --out-dir "$out" --no-format
	ls -l "$out"
	[[ -n "$(find "$out" -name '*.swift' -print -quit)" ]] || { echo "swift: no .swift file generated" >&2; return 1; }
}

# Generate-twice check (ADR-020 section 3): the generated Kotlin bindings and the wasm package must be
# byte-identical across two generations from the same build. Only the generators run twice; the two builds
# are reused (cargo is incremental, so a prior `host` / `wasm` run makes this fast).
run_determinism() {
	local out=target/determinism wasm_file=target/wasm32-unknown-unknown/release/guardedsleep_wasm.wasm
	ensure_wasm_bindgen
	cargo build -p guardedsleep-ffi
	cargo build -p guardedsleep-wasm --release --target wasm32-unknown-unknown
	rm -rf "$out"
	for i in 1 2; do
		cargo run -q -p uniffi-bindgen -- generate --library target/debug/libguardedsleep_ffi.so \
			--language kotlin --out-dir "$out/kotlin-$i" --no-format
		wasm-bindgen --target nodejs --out-dir "$out/wasm-node-$i" "$wasm_file"
		wasm-bindgen --target web --out-dir "$out/wasm-web-$i" "$wasm_file"
	done
	local status=0 kind
	for kind in kotlin wasm-node wasm-web; do
		local h1 h2
		h1="$(cd "$out/$kind-1" && find . -type f -print0 | sort -z | xargs -0 sha256sum | sha256sum)"
		h2="$(cd "$out/$kind-2" && find . -type f -print0 | sort -z | xargs -0 sha256sum | sha256sum)"
		if [[ "$h1" != "$h2" ]]; then
			echo "determinism: $kind output differs between two generations" >&2
			diff -r "$out/$kind-1" "$out/$kind-2" | head -20 >&2 || true
			status=1
		else
			echo "determinism: $kind ${h1%% *} (identical)"
		fi
	done
	return "$status"
}

for cmd in "${@:-test}"; do
	case "$cmd" in
	test) run_test ;;
	wasm) run_wasm ;;
	host) run_host ;;
	android) run_android ;;
	determinism) run_determinism ;;
	swift) run_swift ;;
	*)
		echo "unknown subcommand: $cmd" >&2
		exit 2
		;;
	esac
done
