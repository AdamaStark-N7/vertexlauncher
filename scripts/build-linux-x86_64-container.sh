#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
MAX_GLIBC_VERSION="${VERTEX_MAX_GLIBC_VERSION:-2.35}"
WORK_ROOT="${REPO_ROOT}/.cache/linux-x86_64-container"
TOOLCHAIN_CACHE_ROOT="${REPO_ROOT}/.cache/linux-x86_64-toolchain"
CARGO_HOME_DIR="${TOOLCHAIN_CACHE_ROOT}/cargo-home"
RUSTUP_HOME_DIR="${TOOLCHAIN_CACHE_ROOT}/rustup"
CONTAINER_DIR="${REPO_ROOT}/containers"

source "${REPO_ROOT}/scripts/lib/portable-linux-common.sh"

CONTAINER_IMAGE="${CONTAINER_IMAGE:-$(ensure_podman_image \
  centos7-webkit \
  x86_64 \
  "${CONTAINER_DIR}/vertexlauncher-centos7-webkit.Dockerfile" \
  "${CONTAINER_DIR}")}"

mkdir -p "${WORK_ROOT}" "${CARGO_HOME_DIR}" "${RUSTUP_HOME_DIR}"

podman run --rm \
  --arch=amd64 \
  -v "${REPO_ROOT}:/workspace" \
  -v "${WORK_ROOT}:/cache" \
  -v "${CARGO_HOME_DIR}:/usr/local/cargo" \
  -v "${RUSTUP_HOME_DIR}:/usr/local/rustup" \
  -w /workspace \
  -e MAX_GLIBC_VERSION="${MAX_GLIBC_VERSION}" \
  -e PKG_CONFIG_PATH="/usr/lib64/pkgconfig:/usr/share/pkgconfig" \
  -e PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1 \
  -e PKG_CONFIG_ALLOW_SYSTEM_LIBS=1 \
  "${CONTAINER_IMAGE}" \
  bash -lc '
    set -euo pipefail
    export PATH="/usr/local/cargo/bin:${PATH}"
    export CARGO_HOME=/usr/local/cargo
    export RUSTUP_HOME=/usr/local/rustup
    export HOME=/root
    export XDG_CACHE_HOME=/cache/xdg-cache
    export XDG_DATA_HOME=/cache/xdg-data
    mkdir -p "${CARGO_HOME}" "${RUSTUP_HOME}" "${XDG_CACHE_HOME}" "${XDG_DATA_HOME}"

    echo "[linux-x86_64] ensuring Rust toolchain..."
    if ! command -v rustup >/dev/null 2>&1; then
      echo "[linux-x86_64] bootstrapping rustup..."
      curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
      if [ -f "${HOME}/.cargo/env" ]; then
        # shellcheck disable=SC1091
        . "${HOME}/.cargo/env"
      elif [ -f /usr/local/cargo/env ]; then
        # shellcheck disable=SC1091
        . /usr/local/cargo/env
      fi
    fi
    if ! rustup toolchain list | grep -Eq "^stable($|-)"; then
      rustup toolchain install stable --profile minimal >/dev/null
    fi
    rustup default stable >/dev/null
    rustup target add x86_64-unknown-linux-gnu >/dev/null

    # Export pkg‑config hints within the container as well.  Without these
    # variables the `soup2‑sys` crate sometimes fails to locate the libsoup
    # 2.4 development files even though `libsoup-devel` is installed.  Explicitly
    # populating PKG_CONFIG_PATH and allowing system CFLAGS/LIBS resolves the
    # build failure by pointing pkg‑config at CentOSʼ default search paths and
    # permitting the use of system includes and libraries during the build.
    export PKG_CONFIG_PATH="${PKG_CONFIG_PATH:-/usr/lib64/pkgconfig:/usr/share/pkgconfig}"
    export PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1
    export PKG_CONFIG_ALLOW_SYSTEM_LIBS=1

    bash /workspace/scripts/patch-wry-source.sh

    # Purge any stale build artifacts that may have been compiled against a
    # newer glibc version on the host. Without cleaning the `target` directory
    # the build script executables might be reused across runs and require
    # GLIBC_2.18+ when executed inside this CentOS 7 container (which only
    # provides glibc 2.17).  A clean build ensures that all Rust build
    # scripts are compiled within the container and link against the 2.17
    # runtime, preserving the desired glibc floor.
    echo "[linux-x86_64] cleaning stale build artifacts..."
    cargo clean --package vertexlauncher || true

    echo "[linux-x86_64] building release artifact..."
    cargo build --release --target x86_64-unknown-linux-gnu -p vertexlauncher

    echo "[linux-x86_64] inspecting glibc symbol floor..."
    glibc_floor="$(bash /workspace/scripts/report-linux-glibc-floor.sh /workspace/target/x86_64-unknown-linux-gnu/release/vertexlauncher)"
    echo "[linux-x86_64] highest required glibc: ${glibc_floor}"

    if [ -n "${MAX_GLIBC_VERSION}" ]; then
      normalized_max_glibc="${MAX_GLIBC_VERSION#GLIBC_}"
      normalized_glibc_floor="${glibc_floor#GLIBC_}"

      if [ "$(printf "%s\n%s\n" "${normalized_max_glibc}" "${normalized_glibc_floor}" | sort -V | tail -n 1)" != "${normalized_max_glibc}" ]; then
        echo "[linux-x86_64] glibc floor ${glibc_floor} exceeds allowed maximum ${MAX_GLIBC_VERSION}" >&2
        exit 1
      fi
    fi
  '
