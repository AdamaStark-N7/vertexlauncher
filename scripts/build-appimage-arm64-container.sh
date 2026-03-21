#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CONTAINER_IMAGE="${CONTAINER_IMAGE:-docker.io/library/centos:7}"
WORK_ROOT="${REPO_ROOT}/.cache/appimage-arm64-container"
SOURCE_BINARY="${REPO_ROOT}/target/aarch64-unknown-linux-gnu/release/vertexlauncher"

mkdir -p "${WORK_ROOT}"

if [[ ! -f "${SOURCE_BINARY}" ]]; then
  if [[ -f "${REPO_ROOT}/scripts/build-linux-arm64-container.sh" ]]; then
    echo "[appimage-arm64] building missing aarch64 Linux binary first..."
    bash "${REPO_ROOT}/scripts/build-linux-arm64-container.sh"
  else
    echo "Missing built Linux binary: ${SOURCE_BINARY}" >&2
    exit 1
  fi
fi

declare -A MOUNTED_DIRS=()
PODMAN_ARGS=(
  run
  --rm
  --arch=arm64
  -v "${REPO_ROOT}:/workspace"
  -v "${WORK_ROOT}:/cache"
  -w /workspace
  -e VERTEX_APPIMAGE_ARCH=aarch64
  -e VERTEX_APPIMAGE_TARGET=aarch64-unknown-linux-gnu
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

mount_external_binary() {
  local env_name="$1"
  local container_path="$2"
  local tool_path="${!env_name:-}"

  if [[ -z "${tool_path}" ]]; then
    return 0
  fi
  if [[ ! -f "${tool_path}" ]]; then
    echo "Configured ${env_name} path does not exist: ${tool_path}" >&2
    exit 1
  fi

  PODMAN_ARGS+=(-v "${tool_path}:${container_path}:ro")
  PODMAN_ARGS+=(-e "${env_name}=${container_path}")
}

mount_external_tool VERTEX_LINUXDEPLOY
mount_external_tool VERTEX_APPIMAGETOOL
mount_external_tool VERTEX_LINUXDEPLOY_GTK_PLUGIN

if [[ -z "${VERTEX_APPIMAGE_TOOL_RUNNER:-}" ]] && command -v qemu-aarch64-static >/dev/null 2>&1; then
  VERTEX_APPIMAGE_TOOL_RUNNER="$(command -v qemu-aarch64-static)"
fi
mount_external_binary VERTEX_APPIMAGE_TOOL_RUNNER /tmp/vertex-qemu-aarch64-static

podman "${PODMAN_ARGS[@]}" \
  "${CONTAINER_IMAGE}" \
  bash -lc '
    set -euo pipefail
    export HOME=/cache/home
    export XDG_CACHE_HOME=/cache/xdg-cache
    export XDG_DATA_HOME=/cache/xdg-data
    mkdir -p "${HOME}" "${XDG_CACHE_HOME}" "${XDG_DATA_HOME}"

    configure_centos_vault() {
      rm -f /etc/yum.repos.d/*.repo
      cat >/etc/yum.repos.d/CentOS-Vault.repo <<EOF
[base]
name=CentOS-7 - Base
baseurl=http://vault.centos.org/altarch/7.9.2009/os/\$basearch/
gpgcheck=0
enabled=1
[updates]
name=CentOS-7 - Updates
baseurl=http://vault.centos.org/altarch/7.9.2009/updates/\$basearch/
gpgcheck=0
enabled=1
[extras]
name=CentOS-7 - Extras
baseurl=http://vault.centos.org/altarch/7.9.2009/extras/\$basearch/
gpgcheck=0
enabled=1
EOF
    }

    echo "[appimage-arm64] installing packaging dependencies..."
    configure_centos_vault
    yum -y --setopt=cachedir=/cache/yum install \
      ca-certificates \
      curl \
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

    echo "[appimage-arm64] packaging AppImage inside CentOS 7 container..."
    bash /workspace/scripts/build-appimage.sh
  '
