#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
WORK_ROOT="${REPO_ROOT}/.cache/flatpak-arm64-container"
APPDIR_BINARY="${REPO_ROOT}/target/appimage/aarch64/AppDir/usr/bin/vertexlauncher"
MAX_GLIBC_VERSION="${VERTEX_MAX_GLIBC_VERSION:-2.17}"
CONTAINER_DIR="${REPO_ROOT}/containers"

source "${REPO_ROOT}/scripts/lib/portable-linux-common.sh"

CONTAINER_IMAGE="${CONTAINER_IMAGE:-$(ensure_podman_image \
  debian-tooling \
  aarch64 \
  "${CONTAINER_DIR}/vertexlauncher-debian-tooling.Dockerfile" \
  "${CONTAINER_DIR}")}"

mkdir -p "${WORK_ROOT}"

normalize_glibc_version() {
  local value="$1"
  value="${value#GLIBC_}"
  printf '%s\n' "${value}"
}

appdir_is_usable() {
  local glibc_floor=""

  if [[ ! -x "${APPDIR_BINARY}" ]]; then
    return 1
  fi

  glibc_floor="$(bash "${REPO_ROOT}/scripts/report-linux-glibc-floor.sh" "${APPDIR_BINARY}")"
  echo "[flatpak-arm64] reusing existing aarch64 AppDir with launcher floor ${glibc_floor}..."

  if [[ -n "${MAX_GLIBC_VERSION}" ]]; then
    local normalized_max_glibc
    local normalized_glibc_floor

    normalized_max_glibc="$(normalize_glibc_version "${MAX_GLIBC_VERSION}")"
    normalized_glibc_floor="$(normalize_glibc_version "${glibc_floor}")"

    if [[ "$(printf '%s\n%s\n' "${normalized_max_glibc}" "${normalized_glibc_floor}" | sort -V | tail -n 1)" != "${normalized_max_glibc}" ]]; then
      echo "[flatpak-arm64] cached AppDir floor ${glibc_floor} exceeds allowed maximum ${MAX_GLIBC_VERSION}; rebuilding..." >&2
      return 1
    fi
  fi

  return 0
}

if ! appdir_is_usable; then
  echo "[flatpak-arm64] preparing aarch64 AppDir from the Linux/AppImage pipeline..."
  VERTEX_APPIMAGE_PREPARE_ONLY=1 bash "${REPO_ROOT}/scripts/build-appimage-arm64-container.sh"
fi

podman run --rm \
  --arch=arm64 \
  -v "${REPO_ROOT}:/workspace" \
  -v "${WORK_ROOT}:/cache" \
  -w /workspace \
  -e VERTEX_FLATPAK_BRANCH="${VERTEX_FLATPAK_BRANCH:-stable}" \
  -e VERTEX_FLATPAK_ARCHES=aarch64 \
  -e VERTEX_IN_ARM64_CONTAINER=1 \
  "${CONTAINER_IMAGE}" \
  bash -lc '
    set -euo pipefail
    export HOME=/cache/home
    export XDG_CACHE_HOME=/cache/xdg-cache
    export XDG_DATA_HOME=/cache/xdg-data

    mkdir -p "${HOME}" "${XDG_CACHE_HOME}" "${XDG_DATA_HOME}"

    echo "[flatpak-arm64] exporting aarch64 flatpak under emulation..."
    bash /workspace/scripts/build-flatpak.sh
  '
