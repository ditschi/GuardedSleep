#!/usr/bin/env bash
# Builds and tests the Rust core (ADR-015) inside the guardedsleep-android-native-build image, so the
# host needs only Docker. Subcommands can be combined: test wasm android kotlin
#   scripts/core-build.sh test            # cargo fmt/clippy/test (incl. golden vectors)
#   scripts/core-build.sh wasm            # target/wasm-pkg/node + target/wasm-pkg/web
#   scripts/core-build.sh android         # jniLibs (arm64-v8a, x86_64) + Kotlin bindings for apps/android
# Inside CI or the image itself, set CORE_BUILD_NATIVE=1 to run without Docker.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="${CORE_BUILD_IMAGE:-guardedsleep-android-native-build:local}"
WASM_BINDGEN_VERSION="$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/,""); print; exit}' "$ROOT/Cargo.lock")"

if [[ "${CORE_BUILD_NATIVE:-0}" != "1" ]]; then
	TOOLCHAIN_HASH="$(sha256sum "$ROOT/rust-toolchain.toml" | cut -c1-16)"
	# (Re)builds the image when it is missing or was built from a different rust-toolchain.toml.
	if [[ "$(docker image inspect -f '{{ index .Config.Labels "gs.toolchain" }}' "$IMAGE" 2>/dev/null)" != "$TOOLCHAIN_HASH" ]]; then
		docker build --build-context repo="$ROOT" --label "gs.toolchain=$TOOLCHAIN_HASH" -t "$IMAGE" "$ROOT/docker/android-native-build"
	fi
	exec docker run --rm \
		-e CORE_BUILD_NATIVE=1 \
		-e CARGO_TARGET_DIR=/workspace/target \
		-e GRADLE_USER_HOME=/workspace/target/.gradle-home \
		-v "$ROOT":/workspace -w /workspace \
		"$IMAGE" bash scripts/core-build.sh "$@"
fi

cd "$ROOT"
export CARGO_HOME="${CARGO_HOME_OVERRIDE:-$ROOT/target/.cargo-home}"
export PATH="$CARGO_HOME/bin:/opt/cargo/bin:$PATH"
mkdir -p "$CARGO_HOME"

run_test() {
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --features guardedsleep-core/uniffi -- -D warnings
	cargo test -p guardedsleep-core
}

run_wasm() {
	if [[ "$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]]; then
		cargo install --locked wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION"
	fi
	cargo build -p guardedsleep-core --release --target wasm32-unknown-unknown --features wasm
	local wasm_file=target/wasm32-unknown-unknown/release/guardedsleep_core.wasm
	wasm-bindgen --target nodejs --out-dir target/wasm-pkg/node "$wasm_file"
	wasm-bindgen --target web --out-dir target/wasm-pkg/web "$wasm_file"
	ls -l target/wasm-pkg/web/*.wasm
}

run_android() {
	local jni=apps/android/core/src/main/jniLibs
	local gen=apps/android/core/src/main/kotlin
	cargo ndk -t arm64-v8a -t x86_64 -P 26 -o "$jni" build -p guardedsleep-core --release --features uniffi
	# Unstripped host build: bindings are generated from its UniFFI metadata (the release profile
	# strips those symbols), and the JVM conformance test loads it through JNA.
	cargo build -p guardedsleep-core --features uniffi
	rm -rf "$gen/uniffi"
	cargo run -q -p uniffi-bindgen -- generate --library target/debug/libguardedsleep_core.so \
		--language kotlin --out-dir "$gen" --no-format
	ls -l "$jni"/*/libguardedsleep_core.so
}

for cmd in "${@:-test}"; do
	case "$cmd" in
	test) run_test ;;
	wasm) run_wasm ;;
	android) run_android ;;
	*)
		echo "unknown subcommand: $cmd" >&2
		exit 2
		;;
	esac
done
