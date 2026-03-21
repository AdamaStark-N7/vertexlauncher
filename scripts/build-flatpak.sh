#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
APP_ID="io.github.SturdyFool10.VertexLauncher"
RUNTIME_ID="${VERTEX_FLATPAK_RUNTIME_ID:-org.freedesktop.Platform}"
SDK_ID="${VERTEX_FLATPAK_SDK_ID:-org.freedesktop.Sdk}"
RUNTIME_BRANCH="${VERTEX_FLATPAK_RUNTIME_BRANCH:-24.08}"
BUILD_ROOT="${REPO_ROOT}/flatpak/build-dir"
REPO_ROOT_DIR="${REPO_ROOT}/flatpak/repo"
GENERATED_ROOT="${REPO_ROOT}/flatpak/generated"
SOURCE_ICON="${REPO_ROOT}/crates/launcher_ui/src/assets/vertex.webp"
BRANCH="${VERTEX_FLATPAK_BRANCH:-stable}"
TARGET_ARCHES_RAW="${VERTEX_FLATPAK_ARCHES:-}"
MAX_GLIBC_VERSION="${VERTEX_MAX_GLIBC_VERSION:-2.17}"

require_command() {
  local command_name="$1"
  local install_hint="$2"
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    echo "Missing ${command_name}. ${install_hint}" >&2
    exit 1
  fi
}

find_icon_generator() {
  if command -v magick >/dev/null 2>&1; then
    printf '%s\n' "magick"
    return 0
  fi
  if command -v convert >/dev/null 2>&1; then
    printf '%s\n' "convert"
    return 0
  fi

  return 1
}

ensure_flathub_remote() {
  if flatpak remotes --user --columns=name 2>/dev/null | grep -Fxq "flathub"; then
    return 0
  fi
  if flatpak remotes --system --columns=name 2>/dev/null | grep -Fxq "flathub"; then
    return 0
  fi

  if flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo >/dev/null 2>&1; then
    return 0
  fi

  if flatpak remote-add --system --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo >/dev/null 2>&1; then
    return 0
  fi

  echo "Failed to configure the Flathub remote." >&2
  exit 1
}

ensure_flatpak_ref() {
  local requested_arch="$1"
  local ref="$2"

  if flatpak info --user "${ref}" >/dev/null 2>&1 || flatpak info --system "${ref}" >/dev/null 2>&1; then
    return 0
  fi

  echo "[flatpak] installing missing ref ${ref} for ${requested_arch}..."
  flatpak install \
    --user \
    -y \
    --noninteractive \
    --or-update \
    --arch="${requested_arch}" \
    flathub \
    "${ref}" >/dev/null
}

ensure_runtime_refs() {
  local requested_arch="$1"

  ensure_flathub_remote
  ensure_flatpak_ref "${requested_arch}" "${RUNTIME_ID}//${RUNTIME_BRANCH}"
  ensure_flatpak_ref "${requested_arch}" "${SDK_ID}//${RUNTIME_BRANCH}"
}

normalize_glibc_version() {
  local value="$1"
  value="${value#GLIBC_}"
  printf '%s\n' "${value}"
}

glibc_floor_exceeds_limit() {
  local glibc_floor="$1"
  local max_glibc="$2"
  local normalized_floor
  local normalized_max

  normalized_floor="$(normalize_glibc_version "${glibc_floor}")"
  normalized_max="$(normalize_glibc_version "${max_glibc}")"

  [[ "$(printf "%s\n%s\n" "${normalized_max}" "${normalized_floor}" | sort -V | tail -n 1)" != "${normalized_max}" ]]
}

generate_flatpak_icon() {
  local output_path="${GENERATED_ROOT}/${APP_ID}.png"
  local icon_tool

  icon_tool="$(find_icon_generator)" || {
    echo "Missing ImageMagick. Install 'magick' or 'convert' to generate the Flatpak icon." >&2
    exit 1
  }

  mkdir -p "${GENERATED_ROOT}"
  echo "[flatpak] generating export-safe PNG icon..."
  if [[ "${icon_tool}" == "magick" ]]; then
    magick "${SOURCE_ICON}" -resize 256x256 "PNG32:${output_path}"
  else
    convert "${SOURCE_ICON}" -resize 256x256 "PNG32:${output_path}"
  fi
}

generate_wrapper_script() {
  local output_path="${GENERATED_ROOT}/${APP_ID}-wrapper.sh"

  cat > "${output_path}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

cd /app

export LD_LIBRARY_PATH="/app/lib:/app/lib64/webkit2gtk-4.0${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
export XDG_DATA_DIRS="/app/share${XDG_DATA_DIRS:+:${XDG_DATA_DIRS}}"
export GSETTINGS_SCHEMA_DIR="/app/share/glib-2.0/schemas"
export GDK_PIXBUF_MODULEDIR="/app/lib64/gdk-pixbuf-2.0/2.10.0/loaders"
export GDK_PIXBUF_MODULE_FILE="/app/lib64/gdk-pixbuf-2.0/2.10.0/loaders.cache"
export GTK_IM_MODULE_FILE="/app/lib64/gtk-3.0/3.0.0/immodules.cache"
export GTK_EXE_PREFIX="/app"
export GTK_DATA_PREFIX="/app"

exec /app/bin/vertexlauncher "$@"
EOF

  chmod +x "${output_path}"
}

can_delegate_arm64_container_build() {
  local arch

  if [[ "${VERTEX_ENABLE_ARM64_EMULATION:-}" != "1" ]]; then
    return 1
  fi
  if [[ "${VERTEX_IN_ARM64_CONTAINER:-}" == "1" ]]; then
    return 1
  fi
  if [[ "$(uname -s)" != "Linux" ]]; then
    return 1
  fi
  if ! command -v podman >/dev/null 2>&1; then
    return 1
  fi
  if [[ ! -f "${REPO_ROOT}/scripts/build-flatpak-arm64-container.sh" ]]; then
    return 1
  fi

  for arch in "$@"; do
    if [[ -z "${arch}" ]]; then
      continue
    fi
    if [[ "${arch}" != "aarch64" ]]; then
      return 1
    fi
  done

  return 0
}

run_arm64_container_build() {
  echo "[flatpak] host cannot export aarch64 directly; delegating to ARM64 container..."
  bash "${REPO_ROOT}/scripts/build-flatpak-arm64-container.sh"
}

arch_is_supported() {
  local candidate="$1"
  local supported_arch

  for supported_arch in "${SUPPORTED_ARCHES[@]}"; do
    if [[ "${supported_arch}" == "${candidate}" ]]; then
      return 0
    fi
  done

  return 1
}

ensure_prebuilt_appdir() {
  local requested_arch="$1"
  local appdir="${REPO_ROOT}/target/appimage/${requested_arch}/AppDir"
  local binary_path="${appdir}/usr/bin/vertexlauncher"
  local glibc_floor=""

  if [[ ! -x "${binary_path}" ]]; then
    echo "[flatpak] building ${requested_arch} AppDir from the Linux packaging pipeline..."
    VERTEX_APPIMAGE_ARCH="${requested_arch}" bash "${REPO_ROOT}/scripts/build-appimage.sh"
  fi

  if [[ ! -x "${binary_path}" ]]; then
    echo "Missing packaged Linux bundle for ${requested_arch}: ${binary_path}" >&2
    exit 1
  fi

  glibc_floor="$(bash "${REPO_ROOT}/scripts/report-linux-glibc-floor.sh" "${binary_path}")"
  echo "[flatpak] ${requested_arch} launcher binary glibc floor: ${glibc_floor}"

  if glibc_floor_exceeds_limit "${glibc_floor}" "${MAX_GLIBC_VERSION}"; then
    echo "[flatpak] ${requested_arch} launcher binary glibc floor ${glibc_floor} exceeds allowed maximum ${MAX_GLIBC_VERSION}" >&2
    exit 1
  fi
}

prepare_source_bundle() {
  local requested_arch="$1"
  local source_dir="${GENERATED_ROOT}/source-${requested_arch}"

  rm -rf "${source_dir}"
  mkdir -p "${source_dir}/bundle"
  cp -a "${REPO_ROOT}/target/appimage/${requested_arch}/AppDir/usr/." "${source_dir}/bundle/"
  install -Dm755 "${GENERATED_ROOT}/${APP_ID}-wrapper.sh" "${source_dir}/vertexlauncher-flatpak"
  install -Dm644 "${REPO_ROOT}/flatpak/${APP_ID}.metainfo.xml" "${source_dir}/${APP_ID}.metainfo.xml"
  install -Dm644 "${GENERATED_ROOT}/${APP_ID}.png" "${source_dir}/${APP_ID}.png"

  printf '%s\n' "${source_dir}"
}

prepare_manifest() {
  local requested_arch="$1"
  local source_dir="$2"
  local manifest_path="${GENERATED_ROOT}/${APP_ID}-${requested_arch}.yaml"
  local source_dir_name

  source_dir_name="$(basename -- "${source_dir}")"

  cat > "${manifest_path}" <<EOF
app-id: ${APP_ID}
runtime: ${RUNTIME_ID}
runtime-version: "${RUNTIME_BRANCH}"
sdk: ${SDK_ID}
command: vertexlauncher-flatpak
separate-locales: false
finish-args:
  - --share=network
  - --share=ipc
  - --socket=wayland
  - --socket=x11
  - --socket=pulseaudio
  - --device=dri
  - --filesystem=home
  - --talk-name=org.freedesktop.secrets
  - --env=GDK_BACKEND=wayland,x11
modules:
  - name: vertexlauncher-bundle
    buildsystem: simple
    build-commands:
      - mkdir -p /app
      - install -Dm755 vertexlauncher-flatpak /app/bin/vertexlauncher-flatpak
      - install -Dm644 ${APP_ID}.metainfo.xml /app/share/metainfo/${APP_ID}.metainfo.xml
      - install -Dm644 ${APP_ID}.png /app/share/icons/hicolor/256x256/apps/${APP_ID}.png
      - cp -a bundle/. /app/
    sources:
      - type: dir
        path: ${source_dir_name}
EOF

  printf '%s\n' "${manifest_path}"
}

should_use_direct_export() {
  case "${VERTEX_FLATPAK_DIRECT_EXPORT:-}" in
    1|true|TRUE|True|yes|YES)
      return 0
      ;;
  esac

  [[ "${VERTEX_IN_ARM64_CONTAINER:-}" == "1" ]]
}

stage_build_directory() {
  local requested_arch="$1"
  local source_dir="$2"
  local build_dir="$3"

  flatpak build-init \
    --arch="${requested_arch}" \
    "${build_dir}" \
    "${APP_ID}" \
    "${SDK_ID}" \
    "${RUNTIME_ID}" \
    "${RUNTIME_BRANCH}" >/dev/null

  cp -a "${source_dir}/bundle/." "${build_dir}/files/"
  install -Dm755 "${source_dir}/vertexlauncher-flatpak" "${build_dir}/files/bin/vertexlauncher-flatpak"
  install -Dm644 "${source_dir}/${APP_ID}.metainfo.xml" "${build_dir}/files/share/metainfo/${APP_ID}.metainfo.xml"
  install -Dm644 "${source_dir}/${APP_ID}.png" "${build_dir}/files/share/icons/hicolor/256x256/apps/${APP_ID}.png"
  rm -f \
    "${build_dir}/files/share/icons/hicolor/scalable/apps/${APP_ID}.svg" \
    "${build_dir}/files/share/pixmaps/${APP_ID}.svg"

  flatpak build-finish \
    --command=vertexlauncher-flatpak \
    --share=network \
    --share=ipc \
    --socket=wayland \
    --socket=x11 \
    --socket=pulseaudio \
    --device=dri \
    --filesystem=home \
    --talk-name=org.freedesktop.secrets \
    --env=GDK_BACKEND=wayland,x11 \
    "${build_dir}" >/dev/null
}

if [[ -n "${TARGET_ARCHES_RAW}" ]]; then
  IFS=',' read -r -a TARGET_ARCHES <<< "${TARGET_ARCHES_RAW}"
  if can_delegate_arm64_container_build "${TARGET_ARCHES[@]}"; then
    run_arm64_container_build
    exit 0
  fi
fi

require_command flatpak "Install Flatpak first."
require_command flatpak-builder "Install flatpak-builder first."

if [[ ! -f "${SOURCE_ICON}" ]]; then
  echo "Missing icon source: ${SOURCE_ICON}" >&2
  exit 1
fi

mapfile -t SUPPORTED_ARCHES < <(flatpak --supported-arches)
DEFAULT_ARCH="$(flatpak --default-arch)"

if [[ -z "${TARGET_ARCHES_RAW}" ]]; then
  TARGET_ARCHES_RAW="${DEFAULT_ARCH}"
fi

IFS=',' read -r -a TARGET_ARCHES <<< "${TARGET_ARCHES_RAW}"

mkdir -p "${REPO_ROOT}/target/release" "${BUILD_ROOT}" "${REPO_ROOT_DIR}"
declare -a NATIVE_TARGET_ARCHES=()
declare -a EMULATED_TARGET_ARCHES=()

requested_arch=""
for requested_arch in "${TARGET_ARCHES[@]}"; do
  if [[ -z "${requested_arch}" ]]; then
    continue
  fi

  if ! arch_is_supported "${requested_arch}"; then
    if [[ "${requested_arch}" == "aarch64" ]] && can_delegate_arm64_container_build "${requested_arch}"; then
      EMULATED_TARGET_ARCHES+=("${requested_arch}")
      continue
    fi
    echo "Flatpak host does not support architecture '${requested_arch}'." >&2
    echo "Supported arches on this machine: ${SUPPORTED_ARCHES[*]}" >&2
    exit 1
  fi

  NATIVE_TARGET_ARCHES+=("${requested_arch}")
done

generate_flatpak_icon
generate_wrapper_script

declare -a BUNDLE_PATHS=()
for requested_arch in "${NATIVE_TARGET_ARCHES[@]}"; do
  ensure_prebuilt_appdir "${requested_arch}"
  source_dir="$(prepare_source_bundle "${requested_arch}")"
  build_dir="${BUILD_ROOT}/${requested_arch}"
  repo_dir="${REPO_ROOT_DIR}/${requested_arch}"
  bundle_path="${REPO_ROOT}/target/release/${APP_ID}-${requested_arch}.flatpak"

  ensure_runtime_refs "${requested_arch}"
  rm -rf "${build_dir}" "${repo_dir}"

  if should_use_direct_export; then
    echo "[flatpak] staging ${requested_arch} bundle directly from the prebuilt Linux AppDir..."
    stage_build_directory "${requested_arch}" "${source_dir}" "${build_dir}"
    flatpak build-export \
      --disable-sandbox \
      --arch="${requested_arch}" \
      "${repo_dir}" \
      "${build_dir}" \
      "${BRANCH}" >/dev/null
  else
    local_manifest="$(prepare_manifest "${requested_arch}" "${source_dir}")"
    echo "[flatpak] building ${requested_arch} bundle around the prebuilt Linux AppDir..."
    flatpak-builder \
      --force-clean \
      --disable-rofiles-fuse \
      --arch="${requested_arch}" \
      --repo="${repo_dir}" \
      --default-branch="${BRANCH}" \
      "${build_dir}" \
      "${local_manifest}" >/dev/null
  fi

  echo "[flatpak] bundling portable flatpak for ${requested_arch}..."
  rm -f "${bundle_path}"
  flatpak build-bundle \
    --arch="${requested_arch}" \
    "${repo_dir}" \
    "${bundle_path}" \
    "${APP_ID}" \
    "${BRANCH}" >/dev/null

  BUNDLE_PATHS+=("${bundle_path}")
done

if (( ${#EMULATED_TARGET_ARCHES[@]} > 0 )); then
  run_arm64_container_build
  BUNDLE_PATHS+=("${REPO_ROOT}/target/release/${APP_ID}-aarch64.flatpak")
fi

echo
echo "Flatpak artifacts ready:"
for bundle_path in "${BUNDLE_PATHS[@]}"; do
  echo "  ${bundle_path}"
done
