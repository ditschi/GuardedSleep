#!/usr/bin/env bash
# Runs one cargo-fuzz target on the pinned nightly (tools/nightly/rust-toolchain.toml, the only nightly pin;
# the stable product toolchain cannot build the sanitizer flags). Extra arguments go to libFuzzer:
#   scripts/lib/fuzz.sh envelope_open -- -max_total_time=60
# Targets are listed in fuzz/Cargo.toml. cargo-fuzz: https://rust-fuzz.github.io/book/cargo-fuzz.html
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

# shellcheck source=require.sh
source scripts/lib/require.sh
require fuzz cargo-fuzz

target="${1:-envelope_open}"
[ $# -gt 0 ] && shift
nightly="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' tools/nightly/rust-toolchain.toml)"
rustup toolchain list | grep -q "^$nightly" || rustup toolchain install "$nightly" --profile minimal
cd fuzz
exec cargo "+$nightly" fuzz run "$target" "$@"
