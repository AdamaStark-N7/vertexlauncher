#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CONTAINER_IMAGE="${CONTAINER_IMAGE:-docker.io/library/rust:1-bookworm}"
MAX_GLIBC_VERSION="${VERTEX_MAX_GLIBC_VERSION:-2.17}"
WORK_ROOT="${REPO_ROOT}/.cache/linux-arm64-container"
CARGO_REGISTRY_DIR="${WORK_ROOT}/cargo-registry"
CARGO_GIT_DIR="${WORK_ROOT}/cargo-git"
APT_CACHE_DIR="${WORK_ROOT}/apt-cache"
APT_LISTS_DIR="${WORK_ROOT}/apt-lists"
SYSROOT_DIR="${WORK_ROOT}/sysroot"
RPMS_DIR="${WORK_ROOT}/rpms"
PACKAGE_STAMP="${WORK_ROOT}/sysroot-packages.txt"

mkdir -p "${WORK_ROOT}" "${CARGO_REGISTRY_DIR}" "${CARGO_GIT_DIR}" "${APT_CACHE_DIR}" "${APT_LISTS_DIR}" "${RPMS_DIR}"

podman run --rm \
  -v "${REPO_ROOT}:/workspace" \
  -v "${WORK_ROOT}:/cache" \
  -v "${CARGO_REGISTRY_DIR}:/usr/local/cargo/registry" \
  -v "${CARGO_GIT_DIR}:/usr/local/cargo/git" \
  -v "${APT_CACHE_DIR}:/var/cache/apt" \
  -v "${APT_LISTS_DIR}:/var/lib/apt/lists" \
  -w /workspace \
  -e MAX_GLIBC_VERSION="${MAX_GLIBC_VERSION}" \
  "${CONTAINER_IMAGE}" \
  bash -s -- <<'EOF'
set -euo pipefail

export PATH="/usr/local/cargo/bin:${PATH}"
export DEBIAN_FRONTEND=noninteractive
export CARGO_HOME=/usr/local/cargo
export HOME=/cache/home
export XDG_CACHE_HOME=/cache/xdg-cache
export XDG_DATA_HOME=/cache/xdg-data
mkdir -p "${HOME}" "${XDG_CACHE_HOME}" "${XDG_DATA_HOME}"

SYSROOT_DIR=/cache/sysroot
RPMS_DIR=/cache/rpms
PACKAGE_STAMP=/cache/sysroot-packages.txt

PACKAGE_ROOTS=(
  glib2-devel
  gtk3-devel
  gdk-pixbuf2-devel
  pango-devel
  atk-devel
  cairo-devel
  libsoup-devel
  webkitgtk4-devel
  webkitgtk4-jsc-devel
)

configure_centos_vault_repo() {
  mkdir -p /etc/yum.repos.d
  cat >/etc/yum.repos.d/CentOS-Altarch.repo <<'REPO'
[base]
name=CentOS-7 - Base - aarch64
baseurl=http://vault.centos.org/altarch/7.9.2009/os/aarch64/
enabled=1
gpgcheck=0
[updates]
name=CentOS-7 - Updates - aarch64
baseurl=http://vault.centos.org/altarch/7.9.2009/updates/aarch64/
enabled=1
gpgcheck=0
[extras]
name=CentOS-7 - Extras - aarch64
baseurl=http://vault.centos.org/altarch/7.9.2009/extras/aarch64/
enabled=1
gpgcheck=0
REPO
}

normalize_glibc_version() {
  local value="$1"
  value="${value#GLIBC_}"
  printf '%s\n' "${value}"
}

install_cross_build_host_tools() {
  echo "[linux-arm64] installing cross-build host tools..."
  apt-get update >/dev/null
  apt-get install -y --no-install-recommends \
    ca-certificates \
    cpio \
    dnf \
    dnf-plugins-core \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    make \
    pkg-config \
    rpm2cpio >/dev/null
}

refresh_sysroot_if_needed() {
  local need_refresh=0
  local package_list
  package_list="$(printf '%s\n' "${PACKAGE_ROOTS[@]}")"

  if [[ ! -f "${PACKAGE_STAMP}" ]]; then
    need_refresh=1
  elif ! cmp -s <(printf '%s\n' "${PACKAGE_ROOTS[@]}") "${PACKAGE_STAMP}"; then
    need_refresh=1
  elif [[ ! -d "${SYSROOT_DIR}/usr/include" || ! -d "${SYSROOT_DIR}/usr/lib64" ]]; then
    need_refresh=1
  fi

  if (( need_refresh == 0 )); then
    echo "[linux-arm64] reusing cached CentOS 7 aarch64 sysroot..."
    return 0
  fi

  echo "[linux-arm64] refreshing CentOS 7 aarch64 sysroot..."
  rm -rf "${SYSROOT_DIR}" "${RPMS_DIR}"
  mkdir -p "${SYSROOT_DIR}" "${RPMS_DIR}"

  configure_centos_vault_repo
  dnf \
    --releasever=7 \
    --forcearch=aarch64 \
    --disablerepo=_dnf_local \
    download \
    --resolve \
    --alldeps \
    --destdir "${RPMS_DIR}" \
    "${PACKAGE_ROOTS[@]}" >/dev/null

  shopt -s nullglob
  for rpm in "${RPMS_DIR}"/*.rpm; do
    rpm2cpio "${rpm}" | (cd "${SYSROOT_DIR}" && cpio -idm --quiet)
  done
  shopt -u nullglob

  printf '%s\n' "${PACKAGE_ROOTS[@]}" > "${PACKAGE_STAMP}"
}

install_cross_build_host_tools
refresh_sysroot_if_needed

echo "[linux-arm64] ensuring Rust target..."
rustup target add aarch64-unknown-linux-gnu >/dev/null

bash /workspace/scripts/patch-wry-source.sh

export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR="${SYSROOT_DIR}"
export PKG_CONFIG_LIBDIR="${SYSROOT_DIR}/usr/lib64/pkgconfig:${SYSROOT_DIR}/usr/share/pkgconfig:${SYSROOT_DIR}/usr/lib/pkgconfig"
export PKG_CONFIG_PATH="${PKG_CONFIG_LIBDIR}"
export CARGO_BUILD_JOBS="$(nproc)"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-Clink-arg=--sysroot=${SYSROOT_DIR} -Clink-arg=-Wl,-rpath-link,${SYSROOT_DIR}/lib64 -Clink-arg=-Wl,-rpath-link,${SYSROOT_DIR}/usr/lib64"
export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
export AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar
export CFLAGS_aarch64_unknown_linux_gnu="--sysroot=${SYSROOT_DIR} -D__ARM_ARCH=8 -march=armv8-a"
export CXXFLAGS_aarch64_unknown_linux_gnu="--sysroot=${SYSROOT_DIR} -D__ARM_ARCH=8 -march=armv8-a"

echo "[linux-arm64] cross-compiling release artifact..."
cargo build --release --target aarch64-unknown-linux-gnu -p vertexlauncher

echo "[linux-arm64] inspecting glibc symbol floor..."
glibc_floor="$(bash /workspace/scripts/report-linux-glibc-floor.sh /workspace/target/aarch64-unknown-linux-gnu/release/vertexlauncher)"
echo "[linux-arm64] highest required glibc: ${glibc_floor}"

if [[ -n "${MAX_GLIBC_VERSION}" ]]; then
  normalized_max_glibc="$(normalize_glibc_version "${MAX_GLIBC_VERSION}")"
  normalized_glibc_floor="$(normalize_glibc_version "${glibc_floor}")"

  if [[ "$(printf '%s\n%s\n' "${normalized_max_glibc}" "${normalized_glibc_floor}" | sort -V | tail -n 1)" != "${normalized_max_glibc}" ]]; then
    echo "[linux-arm64] glibc floor ${glibc_floor} exceeds allowed maximum ${MAX_GLIBC_VERSION}" >&2
    exit 1
  fi
fi
EOF
