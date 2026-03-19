#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
APP_ID="io.github.SturdyFool10.VertexLauncher"
MANIFEST_PATH="${REPO_ROOT}/flatpak/${APP_ID}.yaml"
BUILD_ROOT="${REPO_ROOT}/flatpak/build-dir"
REPO_ROOT_DIR="${REPO_ROOT}/flatpak/repo"
VENDOR_DIR="${REPO_ROOT}/flatpak/vendor"
STATE_ROOT="${REPO_ROOT}/.flatpak-builder"
BRANCH="${VERTEX_FLATPAK_BRANCH:-stable}"
TARGET_ARCHES_RAW="${VERTEX_FLATPAK_ARCHES:-}"

require_command() {
  local command_name="$1"
  local install_hint="$2"
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    echo "Missing ${command_name}. ${install_hint}" >&2
    exit 1
  fi
}

require_command cargo "Install Rust/Cargo first."
require_command flatpak "Install Flatpak first."
require_command flatpak-builder "Install flatpak-builder first."

mapfile -t SUPPORTED_ARCHES < <(flatpak --supported-arches)
DEFAULT_ARCH="$(flatpak --default-arch)"

if [[ -z "${TARGET_ARCHES_RAW}" ]]; then
  TARGET_ARCHES_RAW="${DEFAULT_ARCH}"
fi

IFS=',' read -r -a TARGET_ARCHES <<< "${TARGET_ARCHES_RAW}"

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

mkdir -p "${REPO_ROOT}/target/release"
requested_arch=""
for requested_arch in "${TARGET_ARCHES[@]}"; do
  if [[ -z "${requested_arch}" ]]; then
    continue
  fi

  if ! arch_is_supported "${requested_arch}"; then
    echo "Flatpak host does not support architecture '${requested_arch}'." >&2
    echo "Supported arches on this machine: ${SUPPORTED_ARCHES[*]}" >&2
    echo "Run this script on a host that supports '${requested_arch}', or set VERTEX_FLATPAK_ARCHES to one of the supported arches above." >&2
    exit 1
  fi
done

rm -rf "${VENDOR_DIR}"

echo "[flatpak] ensuring Flathub remote exists..."
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo >/dev/null 2>&1 || true

echo "[flatpak] vendoring Cargo dependencies..."
cargo vendor --locked "${VENDOR_DIR}" >/dev/null

declare -a BUNDLE_PATHS=()
for requested_arch in "${TARGET_ARCHES[@]}"; do
  if [[ -z "${requested_arch}" ]]; then
    continue
  fi

  build_dir="${BUILD_ROOT}/${requested_arch}"
  repo_dir="${REPO_ROOT_DIR}/${requested_arch}"
  state_dir="${STATE_ROOT}/${requested_arch}"
  bundle_path="${REPO_ROOT}/target/release/${APP_ID}-${requested_arch}.flatpak"

  rm -rf "${build_dir}" "${repo_dir}" "${state_dir}"

  echo "[flatpak] building sandboxed application for ${requested_arch}..."
  flatpak-builder \
    --user \
    --arch="${requested_arch}" \
    --force-clean \
    --default-branch="${BRANCH}" \
    --install-deps-from=flathub \
    --repo="${repo_dir}" \
    --state-dir="${state_dir}" \
    "${build_dir}" \
    "${MANIFEST_PATH}"

  echo "[flatpak] generating repository metadata for ${requested_arch}..."
  flatpak build-update-repo --generate-static-deltas "${repo_dir}"

  echo "[flatpak] bundling portable flatpak for ${requested_arch}..."
  rm -f "${bundle_path}"
  flatpak build-bundle --arch="${requested_arch}" "${repo_dir}" "${bundle_path}" "${APP_ID}" "${BRANCH}"
  BUNDLE_PATHS+=("${bundle_path}")
done

echo
echo "Flatpak artifacts ready:"
for bundle_path in "${BUNDLE_PATHS[@]}"; do
  echo "  ${bundle_path}"
done
