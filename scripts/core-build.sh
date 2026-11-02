#!/usr/bin/env bash
# Builds and tests the Rust core (ADR-015). Subcommands can be combined: test wasm host android
#   scripts/core-build.sh test            # cargo fmt/clippy/test (incl. golden vectors)
#   scripts/core-build.sh wasm            # target/wasm-pkg/node + target/wasm-pkg/web
#   scripts/core-build.sh host            # host library + Kotlin bindings (what the Android JVM unit tests load; no ABIs)
#   scripts/core-build.sh android         # host + jniLibs (arm64-v8a, x86_64) for apps/android
#   scripts/core-build.sh determinism     # build and generate the Kotlin bindings and the wasm package twice (second time after a clean of the workspace crates), fail on differing hashes
#
# Where each subcommand runs (docs/contributing/development.md, "Rust execution mode"), decided per subcommand:
#   test, wasm                  natively when rustup is on PATH (a distro cargo would ignore rust-toolchain.toml);
#                               otherwise in the guardedsleep-android-native-build image, writing target/ like
#                               scripts/cargo.sh does (no host build exists whose output could be invalidated).
#   host, android, determinism  in the image, writing target/container: the Android chain must build, test and
#                               link in one environment (the image's glibc is older than most hosts'), and the
#                               image's absolute paths and CARGO_HOME would otherwise invalidate the host's
#                               target/ fingerprints. Determinism also gets a reproducible environment.
# CORE_BUILD_NATIVE=1 (CI, devcontainer, inside the image) runs everything natively with the current environment.
# Only cargo outputs follow CARGO_TARGET_DIR; target/wasm-pkg (read by packages/core-wasm/scripts/copy-pkg.mjs,
# crates/guardedsleep-core/js/conformance.test.mjs and scripts/lib/api-snapshot.sh) and target/determinism stay fixed.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_BINDGEN_VERSION="$("$ROOT/scripts/lib/wasm-bindgen-version.sh")"

if [[ "${CORE_BUILD_NATIVE:-0}" != "1" ]]; then
	# shellcheck source=lib/build-image.sh
	source "$ROOT/scripts/lib/build-image.sh"
	for cmd in "${@:-test}"; do
		case "$cmd" in
		test | wasm)
			if command -v rustup >/dev/null 2>&1; then
				CORE_BUILD_NATIVE=1 bash "$ROOT/scripts/core-build.sh" "$cmd"
				continue
			fi
			target_dir=/workspace/target
			;;
		host | android | determinism) target_dir=/workspace/target/container ;;
		*)
			echo "unknown subcommand: $cmd" >&2
			exit 2
			;;
		esac
		gs_ensure_build_image
		docker run --rm \
			"${GS_DOCKER_USER_ARGS[@]}" "${GS_DOCKER_CACHE_ARGS[@]}" \
			-e CORE_BUILD_NATIVE=1 \
			-e CARGO_TARGET_DIR="$target_dir" \
			-v "$ROOT":/workspace -w /workspace \
			"$GS_BUILD_IMAGE" bash scripts/core-build.sh "$cmd"
	done
	exit 0
fi

cd "$ROOT"
# Cargo outputs (debug/, wasm32-unknown-unknown/release/) of this run; absolute inside the image.
TARGET="${CARGO_TARGET_DIR:-target}"
# CARGO_HOME is the environment's; build-image.sh sets /cache/cargo for image runs.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:/opt/cargo/bin:$PATH"

run_test() {
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace
}

# Uses a wasm-bindgen from PATH when its version matches the pin (CI caches ~/.cargo/bin); only on a miss
# it is installed under $TARGET/tools (never over a contributor's global copy) and put first on PATH.
# $TARGET/tools is target/container/tools in the image, so a host-built binary never runs in the older glibc.
wasm_bindgen_ok() {
	[[ "$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')" == "$WASM_BINDGEN_VERSION" ]]
}

ensure_wasm_bindgen() {
	wasm_bindgen_ok && return 0
	export PATH="$TARGET/tools/bin:$PATH"
	wasm_bindgen_ok && return 0
	cargo install --locked --root "$TARGET/tools" wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION"
}

# The node package runs the conformance vectors, so it is built with the test-only `conformance` feature
# (fixed-nonce sealing and key export of the envelope). The web package, which ships, is built without it.
run_wasm() {
	ensure_wasm_bindgen
	local wasm_file="$TARGET/wasm32-unknown-unknown/release/guardedsleep_wasm.wasm"
	cargo build -p guardedsleep-wasm --release --target wasm32-unknown-unknown --features conformance
	wasm-bindgen --target nodejs --out-dir target/wasm-pkg/node "$wasm_file"
	cargo build -p guardedsleep-wasm --release --target wasm32-unknown-unknown
	wasm-bindgen --target web --out-dir target/wasm-pkg/web "$wasm_file"
	ls -l target/wasm-pkg/web/*.wasm
}

# Unstripped host build of guardedsleep-ffi: the Kotlin bindings are generated from its UniFFI metadata (the release profile
# strips those symbols), and the Android JVM unit tests load it through JNA (jna.library.path in
# apps/android/{core-bindings,platform}/build.gradle.kts points at $CARGO_TARGET_DIR/debug). It is built with the
# test-only `conformance` feature so the Kotlin conformance test can run the envelope vectors; the
# cross-compiled release libraries (run_android) are built without it.
run_host() {
	local gen=apps/android/core-bindings/src/main/kotlin
	cargo build -p guardedsleep-ffi --features conformance
	rm -rf "$gen/uniffi"
	cargo run -q -p uniffi-bindgen -- generate --library "$TARGET/debug/libguardedsleep_ffi.so" \
		--language kotlin --out-dir "$gen" --no-format
}

# Same NDK as docker/android-native-build/Dockerfile and the setup-android action. Runners ship other
# NDK versions and export their path as the NDK home, so the pinned one wins when it is installed.
PINNED_NDK=30.0.16248370

run_android() {
	local jni=apps/android/core-bindings/src/main/jniLibs
	if [[ -d "${ANDROID_HOME:-}/ndk/$PINNED_NDK" ]]; then
		export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$PINNED_NDK"
	fi
	cargo ndk -t arm64-v8a -t x86_64 -P 26 -o "$jni" build -p guardedsleep-ffi --release
	run_host
	ls -l "$jni"/*/libguardedsleep_ffi.so
}

# Generate-twice check (ADR-020 section 3): the generated Kotlin bindings and the wasm package must be
# byte-identical across two independent builds. The second round cleans and recompiles the three workspace
# crates (core, ffi, wasm), so non-determinism in macro expansion or metadata emission (uniffi, tsify,
# wasm-bindgen) is caught as well as in the generators. Dependencies stay cached.
run_determinism() {
	local out=target/determinism wasm_file="$TARGET/wasm32-unknown-unknown/release/guardedsleep_wasm.wasm"
	ensure_wasm_bindgen
	rm -rf "$out"
	for i in 1 2; do
		if [[ "$i" == 2 ]]; then
			cargo clean -q -p guardedsleep-core -p guardedsleep-ffi
			cargo clean -q -p guardedsleep-core -p guardedsleep-wasm --release --target wasm32-unknown-unknown
		fi
		cargo build -p guardedsleep-ffi
		cargo build -p guardedsleep-wasm --release --target wasm32-unknown-unknown
		cargo run -q -p uniffi-bindgen -- generate --library "$TARGET/debug/libguardedsleep_ffi.so" \
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
	# RUSTC_WRAPPER is emptied for this call only (works in the image and CORE_BUILD_NATIVE paths): with sccache a
	# cache hit would make the second build a copy of the first, and the generate-twice check could never fail.
	determinism) RUSTC_WRAPPER='' run_determinism ;;
	*)
		echo "unknown subcommand: $cmd" >&2
		exit 2
		;;
	esac
done
