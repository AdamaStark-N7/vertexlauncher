#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
MAX_GLIBC_VERSION="${VERTEX_MAX_GLIBC_VERSION:-2.17}"
TARGET_TRIPLE="aarch64-unknown-linux-gnu"
WORK_ROOT="${REPO_ROOT}/.cache/linux-arm64-container"
SYSROOT_DIR="${WORK_ROOT}/sysroot"
RPMS_DIR="${WORK_ROOT}/rpms"
PACKAGE_STAMP="${WORK_ROOT}/sysroot-packages.txt"
TOOLCHAIN_DIR="${WORK_ROOT}/toolchain"
ARM64_CARGO_JOBS="${VERTEX_ARM64_CARGO_JOBS:-$(nproc)}"
CONTAINER_DIR="${REPO_ROOT}/containers"

PACKAGE_ROOTS=(
  glibc-devel
  glib2-devel
  gtk3-devel
  gdk-pixbuf2-devel
  pango-devel
  atk-devel
  cairo-devel
  dbus-devel
  libsoup-devel
  webkitgtk4-devel
  webkitgtk4-jsc-devel
)

source "${REPO_ROOT}/scripts/lib/portable-linux-common.sh"

CONTAINER_IMAGE="${CONTAINER_IMAGE:-$(ensure_podman_image \
  debian-tooling \
  "$(normalize_arch "$(uname -m)")" \
  "${CONTAINER_DIR}/vertexlauncher-debian-tooling.Dockerfile" \
  "${CONTAINER_DIR}")}"

mkdir -p "${WORK_ROOT}" "${TOOLCHAIN_DIR}"

sysroot_is_usable() {
  [[ -f "${PACKAGE_STAMP}" ]] \
    && cmp -s <(printf '%s\n' "${PACKAGE_ROOTS[@]}") "${PACKAGE_STAMP}" \
    && [[ -f "${SYSROOT_DIR}/usr/lib64/libc.so" ]] \
    && compgen -G "${SYSROOT_DIR}/usr/lib64/libwebkit2gtk-4.0.so.*" >/dev/null \
    && [[ -f "${SYSROOT_DIR}/usr/lib64/pkgconfig/webkit2gtk-4.0.pc" ]] \
    && [[ -f "${SYSROOT_DIR}/usr/lib64/pkgconfig/gtk+-3.0.pc" ]] \
    && [[ -f "${SYSROOT_DIR}/usr/lib64/pkgconfig/libsoup-2.4.pc" ]]
}

refresh_sysroot_if_needed() {
  if sysroot_is_usable; then
    echo "[linux-arm64] reusing cached CentOS 7 aarch64 sysroot..."
    return 0
  fi

  require_command podman "Install Podman so the ARM64 CentOS 7 sysroot can be refreshed on cache miss."

  echo "[linux-arm64] refreshing CentOS 7 aarch64 sysroot cache..."
  mkdir -p "${RPMS_DIR}"

  podman run --rm -i \
    -v "${WORK_ROOT}:/cache" \
    "${CONTAINER_IMAGE}" \
    bash -s -- <<'EOF'
set -euo pipefail

SYSROOT_DIR=/cache/sysroot
RPMS_DIR=/cache/rpms
PACKAGE_STAMP=/cache/sysroot-packages.txt

PACKAGE_ROOTS=(
  glibc-devel
  glib2-devel
  gtk3-devel
  gdk-pixbuf2-devel
  pango-devel
  atk-devel
  cairo-devel
  dbus-devel
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
EOF
}

prepare_toolchain_wrappers() {
  mkdir -p "${TOOLCHAIN_DIR}"

  cat >"${TOOLCHAIN_DIR}/zig-aarch64-common" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

filtered_args=()
skip_next=0
for arg in "$@"; do
  if (( skip_next )); then
    skip_next=0
    continue
  fi

  case "${arg}" in
    --target)
      skip_next=1
      ;;
    --target=*|--sysroot=*)
      ;;
    *)
      filtered_args+=("${arg}")
      ;;
  esac
done

printf '%s\0' "${filtered_args[@]}"
EOF

  cat >"${TOOLCHAIN_DIR}/arm64-pkg-config" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

: "${VERTEX_ARM64_SYSROOT:?}"

export PKG_CONFIG_DIR=
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR="${VERTEX_ARM64_SYSROOT}"
export PKG_CONFIG_LIBDIR="${VERTEX_ARM64_SYSROOT}/usr/lib64/pkgconfig:${VERTEX_ARM64_SYSROOT}/usr/share/pkgconfig:${VERTEX_ARM64_SYSROOT}/usr/lib/pkgconfig"

exec pkg-config "$@"
EOF

  cat >"${TOOLCHAIN_DIR}/zig-aarch64-cc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

: "${VERTEX_ARM64_SYSROOT:?}"

mapfile -d '' -t filtered_args < <("$(dirname -- "$0")/zig-aarch64-common" "$@")

exec zig cc \
  -target aarch64-linux-gnu \
  --sysroot "${VERTEX_ARM64_SYSROOT}" \
  -D__ARM_ARCH=8 \
  "${filtered_args[@]}"
EOF

  cat >"${TOOLCHAIN_DIR}/zig-aarch64-cxx" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

: "${VERTEX_ARM64_SYSROOT:?}"

mapfile -d '' -t filtered_args < <("$(dirname -- "$0")/zig-aarch64-common" "$@")

exec zig c++ \
  -target aarch64-linux-gnu \
  --sysroot "${VERTEX_ARM64_SYSROOT}" \
  -D__ARM_ARCH=8 \
  "${filtered_args[@]}"
EOF

  cat >"${TOOLCHAIN_DIR}/zig-aarch64-linker" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

: "${VERTEX_ARM64_SYSROOT:?}"

mapfile -d '' -t filtered_args < <("$(dirname -- "$0")/zig-aarch64-common" "$@")
linker_args=()
arg_index=0

normalize_link_search_path() {
  local path_value="$1"

  if [[ "${path_value}" != /* ]]; then
    printf '%s\n' "${path_value}"
    return 0
  fi

  if [[ "${path_value}" == "${VERTEX_ARM64_SYSROOT}/"* ]]; then
    printf '/%s\n' "${path_value#${VERTEX_ARM64_SYSROOT}/}"
    return 0
  fi

  realpath -m --relative-to="${PWD}" "${path_value}"
}

while (( arg_index < ${#filtered_args[@]} )); do
  current_arg="${filtered_args[arg_index]}"

  if [[ "${current_arg}" == "-L" ]] && (( arg_index + 1 < ${#filtered_args[@]} )); then
    next_arg="${filtered_args[arg_index + 1]}"
    linker_args+=("${current_arg}" "$(normalize_link_search_path "${next_arg}")")
    arg_index=$((arg_index + 2))
    continue
  fi

  case "${current_arg}" in
    -L/*)
      linker_args+=("-L$(normalize_link_search_path "${current_arg#-L}")")
      ;;
    *)
      linker_args+=("${current_arg}")
      ;;
  esac

  arg_index=$((arg_index + 1))
done

exec zig cc \
  -target aarch64-linux-gnu.2.17 \
  --sysroot "${VERTEX_ARM64_SYSROOT}" \
  -L/lib64 \
  -L/usr/lib64 \
  "${linker_args[@]}"
EOF

  cat >"${TOOLCHAIN_DIR}/zig-ar" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

exec zig ar "$@"
EOF

  cat >"${TOOLCHAIN_DIR}/zig-ranlib" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

exec zig ranlib "$@"
EOF

  chmod +x \
    "${TOOLCHAIN_DIR}/zig-aarch64-common" \
    "${TOOLCHAIN_DIR}/arm64-pkg-config" \
    "${TOOLCHAIN_DIR}/zig-aarch64-cc" \
    "${TOOLCHAIN_DIR}/zig-aarch64-cxx" \
    "${TOOLCHAIN_DIR}/zig-aarch64-linker" \
    "${TOOLCHAIN_DIR}/zig-ar" \
    "${TOOLCHAIN_DIR}/zig-ranlib"
}

require_command cargo "Install Rust and Cargo."
require_command rustup "Install rustup so the ARM64 target can be added."
require_command zig "Install Zig so the host can cross-link the ARM64 Linux build."
require_command pkg-config "Install pkg-config for ARM64 sysroot resolution."

refresh_sysroot_if_needed
prepare_toolchain_wrappers

echo "[linux-arm64] ensuring Rust target..."
rustup target add "${TARGET_TRIPLE}" >/dev/null

bash "${REPO_ROOT}/scripts/patch-wry-source.sh"

export VERTEX_ARM64_SYSROOT="${SYSROOT_DIR}"
export PKG_CONFIG="${TOOLCHAIN_DIR}/arm64-pkg-config"
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_SYSROOT_DIR="${SYSROOT_DIR}"
export PKG_CONFIG_LIBDIR="${SYSROOT_DIR}/usr/lib64/pkgconfig:${SYSROOT_DIR}/usr/share/pkgconfig:${SYSROOT_DIR}/usr/lib/pkgconfig"
export PKG_CONFIG_PATH="${PKG_CONFIG_LIBDIR}"
export CARGO_BUILD_JOBS="${ARM64_CARGO_JOBS}"
export CC_aarch64_unknown_linux_gnu="${TOOLCHAIN_DIR}/zig-aarch64-cc"
export CXX_aarch64_unknown_linux_gnu="${TOOLCHAIN_DIR}/zig-aarch64-cxx"
export AR_aarch64_unknown_linux_gnu="${TOOLCHAIN_DIR}/zig-ar"
export RANLIB_aarch64_unknown_linux_gnu="${TOOLCHAIN_DIR}/zig-ranlib"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="${TOOLCHAIN_DIR}/zig-aarch64-linker"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_AR="${TOOLCHAIN_DIR}/zig-ar"

echo "[linux-arm64] cross-compiling release artifact on the host..."
env \
  -u CFLAGS \
  -u CXXFLAGS \
  -u CPPFLAGS \
  -u LDFLAGS \
  -u CC \
  -u CXX \
  -u AR \
  -u RANLIB \
  -u RUSTFLAGS \
  -u CARGO_BUILD_RUSTFLAGS \
  cargo build --release --target "${TARGET_TRIPLE}" -p vertexlauncher

echo "[linux-arm64] inspecting glibc symbol floor..."
glibc_floor="$(bash "${REPO_ROOT}/scripts/report-linux-glibc-floor.sh" "${REPO_ROOT}/target/${TARGET_TRIPLE}/release/vertexlauncher")"
echo "[linux-arm64] highest required glibc: ${glibc_floor}"

if [[ -n "${MAX_GLIBC_VERSION}" ]]; then
  if glibc_floor_exceeds_limit "${glibc_floor}" "${MAX_GLIBC_VERSION}"; then
    echo "[linux-arm64] glibc floor ${glibc_floor} exceeds allowed maximum ${MAX_GLIBC_VERSION}" >&2
    exit 1
  fi
fi
