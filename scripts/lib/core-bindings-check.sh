#!/usr/bin/env bash
# Binding-free core (ADR-020 section 3, AGENTS.md section 5.1): no UniFFI or wasm-bindgen in guardedsleep-core.
#   1. source: no `uniffi::`, `#[uniffi` or `wasm_bindgen` in crates/guardedsleep-core/src
#   2. default-feature graph: no uniffi, wasm-bindgen, js-sys or web-sys (`cargo tree -p` resolves the
#      features of the core alone; cargo-deny cannot, it unifies features across the workspace, where
#      guardedsleep-wasm enables the optional tsify feature)
# The all-features graph is covered by the `uniffi` ban in deny.core.toml; tsify legitimately adds
# wasm-bindgen, js-sys and web-sys there.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
dir="${CORE_SRC_DIR:-crates/guardedsleep-core/src}"
if grep -rnE 'uniffi::|#\[uniffi|wasm_bindgen' "$dir"; then
  echo "core-bindings-check: UniFFI/wasm-bindgen glue belongs in guardedsleep-ffi / guardedsleep-wasm, not in $dir" >&2
  exit 1
fi
tree="$(scripts/cargo.sh tree -p guardedsleep-core -e normal --prefix none)"
if echo "$tree" | grep -E '^(uniffi|uniffi_[a-z_]+|wasm-bindgen|js-sys|web-sys) '; then
  echo "core-bindings-check: guardedsleep-core pulls a binding crate into its default-feature graph (see above)" >&2
  exit 1
fi
echo "core-bindings-check: guardedsleep-core is binding-free (ok)"
