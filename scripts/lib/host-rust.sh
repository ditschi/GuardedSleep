#!/usr/bin/env bash
# Single predicate for "run Rust on the host" (docs/contributing/development.md, "Rust execution mode").
# Source it, then call gs_host_rust. It checks for rustup, not cargo: a distro cargo would ignore
# rust-toolchain.toml and build with the wrong compiler. Side-effect free.

gs_host_rust() { command -v rustup >/dev/null 2>&1; }
