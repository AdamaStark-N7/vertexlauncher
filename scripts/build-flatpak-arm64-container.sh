#!/usr/bin/env bash
set -euo pipefail

# Packages the aarch64 Flatpak from an x86_64 host. The binary itself is cross compiled first, natively fast, by
# build-linux-arm64-container.sh (run here when it is missing); this container only runs flatpak-builder --arch=aarch64,
# whose `install` steps are the only thing that touches QEMU.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CONTAINER_IMAGE="${CONTAINER_IMAGE:-docker.io/library/debian:bookworm}"
WORK_ROOT="${REPO_ROOT}/.cache/flatpak-arm64-container"
BINARY="${REPO_ROOT}/target/aarch64-unknown-linux-gnu/release/vertexlauncher"

if [[ ! -f "${BINARY}" ]]; then
  echo "[flatpak-arm64] no aarch64 binary yet; cross compiling it first..."
  bash "${SCRIPT_DIR}/build-linux-arm64-container.sh"
fi

mkdir -p "${WORK_ROOT}"

podman run --rm \
  --privileged \
  --arch=amd64 \
  -v "${REPO_ROOT}:/workspace" \
  -v "${WORK_ROOT}:/cache" \
  -w /workspace \
  -e VERTEX_FLATPAK_BRANCH="${VERTEX_FLATPAK_BRANCH:-stable}" \
  "${CONTAINER_IMAGE}" \
  bash -lc '
    set -euo pipefail
    export DEBIAN_FRONTEND=noninteractive
    export HOME=/cache/home
    export XDG_CACHE_HOME=/cache/xdg-cache
    export XDG_DATA_HOME=/cache/xdg-data
    mkdir -p "${HOME}" "${XDG_CACHE_HOME}" "${XDG_DATA_HOME}"

    echo "[flatpak-arm64] installing packaging tools..."
    apt-get update -qq
    apt-get install -y --no-install-recommends \
      ca-certificates curl elfutils appstream-compose file \
      flatpak flatpak-builder ostree \
      python3 python3-aiohttp python3-tomlkit \
      qemu-user-static rsync xz-utils zstd \
      librsvg2-2 librsvg2-common librsvg2-bin \
      >/dev/null
    gdk-pixbuf-query-loaders --update-cache 2>/dev/null || true
    if [ -d /proc/sys/fs/binfmt_misc ]; then
      update-binfmts --enable qemu-aarch64 2>/dev/null || true
    fi

    PREBUILT=/workspace/flatpak/generated/prebuilt-aarch64
    mkdir -p "${PREBUILT}"
    cp /workspace/target/aarch64-unknown-linux-gnu/release/vertexlauncher "${PREBUILT}/vertexlauncher"
    echo "[flatpak-arm64] packaging $(file -b ${PREBUILT}/vertexlauncher)"

    export VERTEX_IN_ARM64_CONTAINER=1
    export VERTEX_FLATPAK_ARCHES=aarch64
    export VERTEX_PREBUILT_AARCH64="${PREBUILT}/vertexlauncher"
    bash /workspace/scripts/build-flatpak.sh
  '
