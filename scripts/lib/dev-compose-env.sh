#!/usr/bin/env bash
# Shared helpers for `docker compose` against `.devcontainer/docker-compose.yml`.
# Keep bind-mount writes working for rootless Docker and optional `.devcontainer/.env`.
#
# shellcheck shell=bash

# True when the Docker daemon is running in rootless mode (host uid maps to container uid 0).
# `docker info` formats differ by version/engine; match several signals.
dev_compose__docker_is_rootless() {
  local info ctx lower
  ctx="$(docker context show 2>/dev/null || true)"
  lower="$(printf '%s' "$ctx" | tr '[:upper:]' '[:lower:]')"
  [[ "$lower" == *rootless* ]] && return 0

  [[ "${DOCKER_HOST:-}" == *rootless* ]] && return 0

  info="$(docker info 2>/dev/null)" || return 1
  lower="$(printf '%s' "$info" | tr '[:upper:]' '[:lower:]')"
  [[ "$lower" == *'rootless: true'* ]] && return 0
  [[ "$lower" == *'running in rootless'* ]] && return 0
  [[ "$lower" == *'rootless mode'* ]] && return 0
  [[ "$lower" == *'context: rootless'* ]] && return 0
  return 1
}

# Load `.devcontainer/.env` (optional), then default DEV_UID/DEV_GID for rootless if unset.
dev_compose__prepare_user_for_compose() {
  local root="$1"
  local env_file="$root/.devcontainer/.env"
  set -a
  if [[ -f "$env_file" ]]; then
    # shellcheck disable=SC1090
    source "$env_file"
  fi
  set +a

  # Host override (wins over .env for this process): e.g. when rootless heuristics miss your engine.
  if [[ -n "${GUARDSLEEP_DEVCONTAINER_UID:-}" ]]; then
    export DEV_UID="$GUARDSLEEP_DEVCONTAINER_UID"
    export DEV_GID="${GUARDSLEEP_DEVCONTAINER_GID:-$GUARDSLEEP_DEVCONTAINER_UID}"
    return 0
  fi

  if [[ -n "${DEV_UID-}" || -n "${DEV_GID-}" ]]; then
    return 0
  fi

  if dev_compose__docker_is_rootless; then
    export DEV_UID=0
    export DEV_GID=0
  fi
}
