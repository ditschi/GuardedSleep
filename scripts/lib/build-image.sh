#!/usr/bin/env bash
# Shared helper: makes sure the guardedsleep-android-native-build image exists and is current.
# Source it, then call gs_ensure_build_image (sets GS_BUILD_IMAGE to the image tag):
#   source "$ROOT/scripts/lib/build-image.sh"
#   gs_ensure_build_image
# The image carries a label "gs.toolchain" with a hash of rust-toolchain.toml, tools/android-versions.env and the Dockerfile, so it
# is rebuilt whenever either changes. The repo root is passed as the named build context "repo"
# (the Dockerfile copies rust-toolchain.toml and tools/android-versions.env from it).
# It also sets GS_DOCKER_USER_ARGS, the array to pass to `docker run`: rootful Docker runs as the caller's
# uid so files written to target/ stay owned by the caller (host cargo shares that directory); rootless
# Docker already maps container root to the caller, so nothing is added there. HOME points at a
# writable directory under target/ because the uid has no passwd entry in the image.
# GS_DOCKER_CACHE_ARGS mounts the shared Cargo home (registry, git DBs, installed tools) and Gradle user
# home from ${GS_SHARED_CACHE:-$HOME/.cache/guardedsleep}/{cargo,gradle} at /cache and sets CARGO_HOME,
# CARGO_HOME_OVERRIDE (read by core-build.sh) and GRADLE_USER_HOME, so worktrees download dependencies
# once. Cargo's package-cache lock and Gradle's cross-process cache locks make concurrent runs safe;
# target/ and the Gradle project cache stay per worktree. AGENTS.md section 10.1.
# Unless GS_SCCACHE=0 or CI is set, it also mounts a shared sccache directory ($cache/sccache, capped by
# GS_SCCACHE_SIZE, default 10G) and sets RUSTC_WRAPPER=sccache, so worktrees reuse compiled dependencies
# (every worktree compiles at /workspace, so cache keys match). Off in CI, and core-build.sh determinism
# empties RUSTC_WRAPPER: a cache hit would make its generate-twice check compare a build with its own copy.
# Named build contexts: https://docs.docker.com/reference/cli/docker/buildx/build/#build-context
# shellcheck disable=SC2034 # GS_BUILD_IMAGE, GS_DOCKER_USER_ARGS and GS_DOCKER_CACHE_ARGS are read by the scripts that source this file

gs_ensure_build_image() {
  local root hash
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  GS_BUILD_IMAGE="${CORE_BUILD_IMAGE:-guardedsleep-android-native-build:local}"
  hash="$(cat "$root/rust-toolchain.toml" "$root/tools/android-versions.env" "$root/docker/android-native-build/Dockerfile" | sha256sum | cut -c1-16)"
  if [[ "$(docker image inspect -f '{{ index .Config.Labels "gs.toolchain" }}' "$GS_BUILD_IMAGE" 2>/dev/null)" != "$hash" ]]; then
    docker build --build-context repo="$root" --label "gs.toolchain=$hash" -t "$GS_BUILD_IMAGE" \
      "$root/docker/android-native-build" >&2
  fi
  GS_DOCKER_USER_ARGS=()
  if ! docker info -f '{{.SecurityOptions}}' 2>/dev/null | grep -q rootless; then
    mkdir -p "$root/target/.home"
    GS_DOCKER_USER_ARGS=(--user "$(id -u):$(id -g)" -e HOME=/workspace/target/.home)
  fi
  local cache="${GS_SHARED_CACHE:-$HOME/.cache/guardedsleep}"
  mkdir -p "$cache/cargo" "$cache/gradle"
  GS_DOCKER_CACHE_ARGS=(
    -v "$cache/cargo:/cache/cargo" -v "$cache/gradle:/cache/gradle"
    -e CARGO_HOME=/cache/cargo -e CARGO_HOME_OVERRIDE=/cache/cargo -e GRADLE_USER_HOME=/cache/gradle
  )
  if [[ "${GS_SCCACHE:-1}" != "0" && -z "${CI:-}" ]]; then
    mkdir -p "$cache/sccache"
    GS_DOCKER_CACHE_ARGS+=(
      -v "$cache/sccache:/cache/sccache"
      -e RUSTC_WRAPPER=sccache -e SCCACHE_DIR=/cache/sccache -e SCCACHE_CACHE_SIZE="${GS_SCCACHE_SIZE:-10G}"
    )
  fi
}
