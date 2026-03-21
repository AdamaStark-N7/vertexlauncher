#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CONTAINER_IMAGE="${CONTAINER_IMAGE:-docker.io/library/centos:7}"
WORK_ROOT="${REPO_ROOT}/.cache/appimage-x86_64-container"
CARGO_REGISTRY_DIR="${WORK_ROOT}/cargo-registry"
CARGO_GIT_DIR="${WORK_ROOT}/cargo-git"

mkdir -p "${WORK_ROOT}" "${CARGO_REGISTRY_DIR}" "${CARGO_GIT_DIR}"

declare -A MOUNTED_DIRS=()
PODMAN_ARGS=(
  run
  --rm
  --arch=amd64
  -v "${REPO_ROOT}:/workspace"
  -v "${WORK_ROOT}:/cache"
  -v "${CARGO_REGISTRY_DIR}:/usr/local/cargo/registry"
  -v "${CARGO_GIT_DIR}:/usr/local/cargo/git"
  -w /workspace
  -e VERTEX_APPIMAGE_ARCH=x86_64
  -e VERTEX_APPIMAGE_TARGET=x86_64-unknown-linux-gnu
  -e VERTEX_IN_APPIMAGE_CONTAINER=1
)

mount_external_tool() {
  local env_name="$1"
  local tool_path="${!env_name:-}"
  local tool_dir

  if [[ -z "${tool_path}" ]]; then
    return 0
  fi
  if [[ ! -f "${tool_path}" ]]; then
    echo "Configured ${env_name} path does not exist: ${tool_path}" >&2
    exit 1
  fi

  tool_dir="$(dirname -- "${tool_path}")"
  if [[ -z "${MOUNTED_DIRS[${tool_dir}]:-}" ]]; then
    PODMAN_ARGS+=(-v "${tool_dir}:${tool_dir}:ro")
    MOUNTED_DIRS["${tool_dir}"]=1
  fi
  PODMAN_ARGS+=(-e "${env_name}=${tool_path}")
}

mount_external_tool VERTEX_LINUXDEPLOY
mount_external_tool VERTEX_APPIMAGETOOL
mount_external_tool VERTEX_LINUXDEPLOY_GTK_PLUGIN

podman "${PODMAN_ARGS[@]}" \
  "${CONTAINER_IMAGE}" \
  bash -lc '
    set -euo pipefail
    export DEBIAN_FRONTEND=noninteractive
    export PATH="/usr/local/cargo/bin:${PATH}"
    export HOME=/cache/home
    export XDG_CACHE_HOME=/cache/xdg-cache
    export XDG_DATA_HOME=/cache/xdg-data
    mkdir -p "${HOME}" "${XDG_CACHE_HOME}" "${XDG_DATA_HOME}"

    configure_centos_vault() {
      rm -f /etc/yum.repos.d/*.repo
      cat >/etc/yum.repos.d/CentOS-Vault.repo <<EOF
[base]
name=CentOS-7 - Base
baseurl=http://vault.centos.org/7.9.2009/os/\$basearch/
gpgcheck=0
enabled=1
[updates]
name=CentOS-7 - Updates
baseurl=http://vault.centos.org/7.9.2009/updates/\$basearch/
gpgcheck=0
enabled=1
[extras]
name=CentOS-7 - Extras
baseurl=http://vault.centos.org/7.9.2009/extras/\$basearch/
gpgcheck=0
enabled=1
EOF
    }

    echo "[appimage-x86_64] installing native packaging dependencies..."
    configure_centos_vault
    yum -y install \
      ca-certificates \
      curl \
      pkgconfig \
      patchelf \
      file \
      desktop-file-utils \
      glib2-devel \
      gtk3-devel \
      gdk-pixbuf2-devel \
      pango-devel \
      atk-devel \
      cairo-devel \
      libsoup-devel \
      webkitgtk4-devel \
      webkitgtk4-jsc-devel >/dev/null

    if [[ ! -f /workspace/target/x86_64-unknown-linux-gnu/release/vertexlauncher ]]; then
      echo "[appimage-x86_64] ensuring Rust toolchain..."
      if ! command -v rustup >/dev/null 2>&1; then
        echo "[appimage-x86_64] bootstrapping rustup..."
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

      bash /workspace/scripts/patch-wry-source.sh

      echo "[appimage-x86_64] building native x86_64 binary..."
      cargo build --release --target x86_64-unknown-linux-gnu -p vertexlauncher
    fi

    echo "[appimage-x86_64] packaging AppImage inside CentOS 7 container..."
    bash /workspace/scripts/build-appimage.sh
  '
