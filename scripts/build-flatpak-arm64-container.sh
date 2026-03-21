#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CONTAINER_IMAGE="${CONTAINER_IMAGE:-docker.io/library/debian:bookworm}"
WORK_ROOT="${REPO_ROOT}/.cache/flatpak-arm64-container"
APT_CACHE_DIR="${WORK_ROOT}/apt-cache"
APT_LISTS_DIR="${WORK_ROOT}/apt-lists"
APPDIR_BINARY="${REPO_ROOT}/target/appimage/aarch64/AppDir/usr/bin/vertexlauncher"
MAX_GLIBC_VERSION="${VERTEX_MAX_GLIBC_VERSION:-2.17}"

mkdir -p "${WORK_ROOT}" "${APT_CACHE_DIR}" "${APT_LISTS_DIR}"

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
  bash "${REPO_ROOT}/scripts/build-appimage-arm64-container.sh"
fi

podman run --rm \
  --arch=arm64 \
  -v "${REPO_ROOT}:/workspace" \
  -v "${WORK_ROOT}:/cache" \
  -v "${APT_CACHE_DIR}:/var/cache/apt" \
  -v "${APT_LISTS_DIR}:/var/lib/apt/lists" \
  -w /workspace \
  -e VERTEX_FLATPAK_BRANCH="${VERTEX_FLATPAK_BRANCH:-stable}" \
  -e VERTEX_FLATPAK_ARCHES=aarch64 \
  -e VERTEX_IN_ARM64_CONTAINER=1 \
  "${CONTAINER_IMAGE}" \
  bash -lc '
    set -euo pipefail
    export DEBIAN_FRONTEND=noninteractive
    export HOME=/cache/home
    export XDG_CACHE_HOME=/cache/xdg-cache
    export XDG_DATA_HOME=/cache/xdg-data

    mkdir -p "${HOME}" "${XDG_CACHE_HOME}" "${XDG_DATA_HOME}"

    echo "[flatpak-arm64] installing packaging dependencies..."
    apt-get update >/dev/null
    apt-get install -y --no-install-recommends \
      binutils \
      ca-certificates \
      flatpak \
      flatpak-builder \
      imagemagick >/dev/null

    echo "[flatpak-arm64] exporting aarch64 flatpak under emulation..."
    bash /workspace/scripts/build-flatpak.sh
  '
