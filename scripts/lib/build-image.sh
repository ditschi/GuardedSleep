#!/usr/bin/env bash
# Shared helper: makes sure the guardedsleep-android-native-build image exists and is current.
# Source it, then call gs_ensure_build_image (sets GS_BUILD_IMAGE to the image tag):
#   source "$ROOT/scripts/lib/build-image.sh"
#   gs_ensure_build_image
# The image carries a label "gs.toolchain" with a hash of rust-toolchain.toml and the Dockerfile, so it
# is rebuilt whenever either changes. The repo root is passed as the named build context "repo"
# (the Dockerfile copies rust-toolchain.toml from it).
# Named build contexts: https://docs.docker.com/reference/cli/docker/buildx/build/#build-context

gs_ensure_build_image() {
  local root hash
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  GS_BUILD_IMAGE="${CORE_BUILD_IMAGE:-guardedsleep-android-native-build:local}"
  hash="$(cat "$root/rust-toolchain.toml" "$root/docker/android-native-build/Dockerfile" | sha256sum | cut -c1-16)"
  if [[ "$(docker image inspect -f '{{ index .Config.Labels "gs.toolchain" }}' "$GS_BUILD_IMAGE" 2>/dev/null)" != "$hash" ]]; then
    docker build --build-context repo="$root" --label "gs.toolchain=$hash" -t "$GS_BUILD_IMAGE" \
      "$root/docker/android-native-build" >&2
  fi
}
