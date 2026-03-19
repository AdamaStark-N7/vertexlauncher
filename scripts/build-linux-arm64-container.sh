#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CONTAINER_IMAGE="${CONTAINER_IMAGE:-docker.io/library/rust:1-bookworm}"
WORK_ROOT="${REPO_ROOT}/.cache/linux-arm64-container"
SYSROOT_DIR="${WORK_ROOT}/sysroot"
DEBS_DIR="${WORK_ROOT}/debs"
PACKAGE_LIST="${WORK_ROOT}/packages.txt"
RESOLVED_LIST="${WORK_ROOT}/resolved-packages.txt"
PACKAGE_ROOTS=(
  "libglib2.0-dev:arm64"
  "libgtk-3-dev:arm64"
  "libgdk-pixbuf-2.0-dev:arm64"
  "libpango1.0-dev:arm64"
  "libatk1.0-dev:arm64"
  "libcairo2-dev:arm64"
  "libsysprof-4-dev:arm64"
  "libsoup-3.0-dev:arm64"
  "libwebkit2gtk-4.1-dev:arm64"
  "libjavascriptcoregtk-4.1-dev:arm64"
)

mkdir -p "${WORK_ROOT}"

podman run --rm \
  -v "${REPO_ROOT}:/workspace" \
  -v "${WORK_ROOT}:/cache" \
  -w /workspace \
  "${CONTAINER_IMAGE}" \
  bash -lc '
    set -euo pipefail
    export DEBIAN_FRONTEND=noninteractive

    echo "[linux-arm64] installing cross-build host tools..."
    apt-get update >/dev/null
    apt-get install -y --no-install-recommends apt-rdepends gcc-aarch64-linux-gnu g++-aarch64-linux-gnu pkg-config ca-certificates curl >/dev/null

    echo "[linux-arm64] enabling arm64 package metadata..."
    dpkg --add-architecture arm64
    apt-get update >/dev/null

    SYSROOT_DIR=/cache/sysroot
    DEBS_DIR=/cache/debs
    PACKAGE_LIST=/cache/packages.txt
    RESOLVED_LIST=/cache/resolved-packages.txt

    rm -rf "${SYSROOT_DIR}"
    mkdir -p "${SYSROOT_DIR}" "${DEBS_DIR}"

    if ! compgen -G "${DEBS_DIR}/*.deb" > /dev/null; then
      echo "[linux-arm64] resolving dependency graph..."
      for pkg in '"$(printf "%q " "${PACKAGE_ROOTS[@]}")"'; do
        apt-rdepends "${pkg}" 2>/dev/null
      done | grep -E "^[A-Za-z0-9][^ ]*$" | sed "s/:arm64$//" | sort -u > "${PACKAGE_LIST}"

      : > "${RESOLVED_LIST}"
      total="$(wc -l < "${PACKAGE_LIST}")"
      current=0
      cd "${DEBS_DIR}"
      while read -r pkg; do
        current="$((current + 1))"
        if apt-get download "${pkg}:arm64" >/dev/null 2>&1; then
          echo "${pkg}:arm64" >> "${RESOLVED_LIST}"
        elif apt-get download "${pkg}" >/dev/null 2>&1; then
          echo "${pkg}" >> "${RESOLVED_LIST}"
        else
          echo "[linux-arm64] skipping virtual/non-downloadable package: ${pkg}"
        fi
        if (( current % 25 == 0 )) || (( current == total )); then
          echo "[linux-arm64] downloaded ${current}/${total} packages..."
        fi
      done < "${PACKAGE_LIST}"
    else
      echo "[linux-arm64] reusing cached Debian sysroot packages..."
    fi

    echo "[linux-arm64] extracting sysroot..."
    for deb in "${DEBS_DIR}"/*.deb; do
      dpkg-deb -x "${deb}" "${SYSROOT_DIR}"
    done

    echo "[linux-arm64] adding Rust target..."
    if ! command -v rustup >/dev/null 2>&1; then
      echo "[linux-arm64] bootstrapping rustup..."
      curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
      export PATH="${HOME}/.cargo/bin:/usr/local/cargo/bin:${PATH}"
      if [ -f "${HOME}/.cargo/env" ]; then
        # shellcheck disable=SC1091
        . "${HOME}/.cargo/env"
      elif [ -f /usr/local/cargo/env ]; then
        # shellcheck disable=SC1091
        . /usr/local/cargo/env
      fi
    fi
    rustup target add aarch64-unknown-linux-gnu >/dev/null

    export PKG_CONFIG_ALLOW_CROSS=1
    export PKG_CONFIG_SYSROOT_DIR="${SYSROOT_DIR}"
    export PKG_CONFIG_LIBDIR="${SYSROOT_DIR}/usr/lib/aarch64-linux-gnu/pkgconfig:${SYSROOT_DIR}/usr/share/pkgconfig"
    export PKG_CONFIG_PATH="${SYSROOT_DIR}/usr/lib/aarch64-linux-gnu/pkgconfig:${SYSROOT_DIR}/usr/share/pkgconfig"
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-Clink-arg=--sysroot=${SYSROOT_DIR}"
    export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
    export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
    export CFLAGS_aarch64_unknown_linux_gnu="--sysroot=${SYSROOT_DIR}"
    export CXXFLAGS_aarch64_unknown_linux_gnu="--sysroot=${SYSROOT_DIR}"

    echo "[linux-arm64] building release artifact..."
    cargo build --release --target aarch64-unknown-linux-gnu -p vertexlauncher
  '
