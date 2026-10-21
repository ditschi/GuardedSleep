#!/usr/bin/env bash
# Shared helper: makes sure the guardedsleep-android-native-build image exists and is current.
# Source it, then call gs_ensure_build_image (sets GS_BUILD_IMAGE to the image tag):
#   source "$ROOT/scripts/lib/build-image.sh"
#   gs_ensure_build_image
# The image carries a label "gs.toolchain" with a hash of rust-toolchain.toml and the Dockerfile, so it
# is rebuilt whenever either changes. The repo root is passed as the named build context "repo"
# (the Dockerfile copies rust-toolchain.toml from it).
# It also sets GS_DOCKER_USER_ARGS, the array to pass to `docker run`: rootful Docker runs as the caller's
# uid so files written to target/ stay owned by the caller (host cargo shares that directory); rootless
# Docker already maps container root to the caller, so nothing is added there. HOME points at a
# writable directory under target/ because the uid has no passwd entry in the image.
# Named build contexts: https://docs.docker.com/reference/cli/docker/buildx/build/#build-context
# shellcheck disable=SC2034 # GS_BUILD_IMAGE and GS_DOCKER_USER_ARGS are read by the scripts that source this file

gs_ensure_build_image() {
  local root hash
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  GS_BUILD_IMAGE="${CORE_BUILD_IMAGE:-guardedsleep-android-native-build:local}"
  hash="$(cat "$root/rust-toolchain.toml" "$root/docker/android-native-build/Dockerfile" | sha256sum | cut -c1-16)"
  if [[ "$(docker image inspect -f '{{ index .Config.Labels "gs.toolchain" }}' "$GS_BUILD_IMAGE" 2>/dev/null)" != "$hash" ]]; then
    docker build --build-context repo="$root" --label "gs.toolchain=$hash" -t "$GS_BUILD_IMAGE" \
      "$root/docker/android-native-build" >&2
  fi
  GS_DOCKER_USER_ARGS=()
  if ! docker info -f '{{.SecurityOptions}}' 2>/dev/null | grep -q rootless; then
    mkdir -p "$root/target/.home"
    GS_DOCKER_USER_ARGS=(--user "$(id -u):$(id -g)" -e HOME=/workspace/target/.home)
  fi
}
