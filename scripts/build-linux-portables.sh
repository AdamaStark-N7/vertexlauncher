#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

source "${REPO_ROOT}/scripts/lib/portable-linux-common.sh"

usage() {
  cat <<'EOF'
Usage: bash scripts/build-linux-portables.sh [options]

Options:
  --arches <list>           Comma-separated arches: x86_64,aarch64
  --formats <list>          Comma-separated formats: appimage,flatpak
  --skip-binary-build       Reuse existing target/<triple>/release/vertexlauncher binaries
  --help                    Show this help

Environment overrides:
  VERTEX_PORTABLE_ARCHES
  VERTEX_PORTABLE_FORMATS
  VERTEX_PORTABLE_SKIP_BINARY_BUILD=1
EOF
}

append_unique() {
  local candidate="$1"
  shift || true
  local existing

  for existing in "$@"; do
    if [[ "${existing}" == "${candidate}" ]]; then
      return 1
    fi
  done

  return 0
}

contains_value() {
  local needle="$1"
  shift || true
  local value

  for value in "$@"; do
    if [[ "${value}" == "${needle}" ]]; then
      return 0
    fi
  done

  return 1
}

normalize_format() {
  case "${1:-}" in
    appimage|AppImage)
      printf 'appimage\n'
      ;;
    flatpak|Flatpak)
      printf 'flatpak\n'
      ;;
    *)
      return 1
      ;;
  esac
}

target_for_arch() {
  local requested_arch

  requested_arch="$(normalize_arch "${1:-}")" || return 1
  case "${requested_arch}" in
    x86_64)
      printf 'x86_64-unknown-linux-gnu\n'
      ;;
    aarch64)
      printf 'aarch64-unknown-linux-gnu\n'
      ;;
  esac
}

requested_arches_raw="${VERTEX_PORTABLE_ARCHES:-}"
requested_formats_raw="${VERTEX_PORTABLE_FORMATS:-appimage,flatpak}"
skip_binary_build=0

case "${VERTEX_PORTABLE_SKIP_BINARY_BUILD:-}" in
  1|true|TRUE|True|yes|YES)
    skip_binary_build=1
    ;;
esac

while (( $# > 0 )); do
  case "$1" in
    --arches)
      requested_arches_raw="${2:-}"
      shift 2
      ;;
    --arches=*)
      requested_arches_raw="${1#*=}"
      shift
      ;;
    --formats)
      requested_formats_raw="${2:-}"
      shift 2
      ;;
    --formats=*)
      requested_formats_raw="${1#*=}"
      shift
      ;;
    --skip-binary-build)
      skip_binary_build=1
      shift
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Portable Linux artifacts can only be built on Linux hosts." >&2
  exit 2
fi

if [[ -z "${requested_arches_raw}" ]]; then
  requested_arches_raw="$(default_linux_portable_arches | paste -sd, -)"
fi

IFS=',' read -r -a raw_arches <<< "${requested_arches_raw}"
IFS=',' read -r -a raw_formats <<< "${requested_formats_raw}"

declare -a requested_arches=()
declare -a requested_formats=()

for arch in "${raw_arches[@]}"; do
  [[ -n "${arch}" ]] || continue
  normalized_arch="$(normalize_arch "${arch}")" || {
    echo "Unsupported architecture: ${arch}" >&2
    exit 1
  }
  if append_unique "${normalized_arch}" "${requested_arches[@]}"; then
    requested_arches+=("${normalized_arch}")
  fi
done

for format_name in "${raw_formats[@]}"; do
  [[ -n "${format_name}" ]] || continue
  normalized_format="$(normalize_format "${format_name}")" || {
    echo "Unsupported portable format: ${format_name}" >&2
    exit 1
  }
  if append_unique "${normalized_format}" "${requested_formats[@]}"; then
    requested_formats+=("${normalized_format}")
  fi
done

if (( ${#requested_arches[@]} == 0 )); then
  echo "No portable Linux architectures were requested." >&2
  exit 1
fi

if (( ${#requested_formats[@]} == 0 )); then
  echo "No portable Linux formats were requested." >&2
  exit 1
fi

if (( ! skip_binary_build )); then
  for requested_arch in "${requested_arches[@]}"; do
    case "${requested_arch}" in
      x86_64)
        bash "${REPO_ROOT}/scripts/build-linux-x86_64-container.sh"
        ;;
      aarch64)
        bash "${REPO_ROOT}/scripts/build-linux-arm64-container.sh"
        ;;
    esac
  done
fi

if contains_value appimage "${requested_formats[@]}"; then
  for requested_arch in "${requested_arches[@]}"; do
    build_env=(
      "VERTEX_APPIMAGE_ARCH=${requested_arch}"
      "VERTEX_APPIMAGE_TARGET=$(target_for_arch "${requested_arch}")"
    )
    if [[ "${requested_arch}" == "aarch64" ]]; then
      build_env+=("VERTEX_ENABLE_ARM64_EMULATION=1")
    fi

    env "${build_env[@]}" bash "${REPO_ROOT}/scripts/build-appimage.sh"
  done
fi

if contains_value flatpak "${requested_formats[@]}"; then
  flatpak_env=(
    "VERTEX_FLATPAK_ARCHES=$(IFS=,; printf '%s' "${requested_arches[*]}")"
  )
  if contains_value aarch64 "${requested_arches[@]}"; then
    flatpak_env+=("VERTEX_ENABLE_ARM64_EMULATION=1")
  fi

  env "${flatpak_env[@]}" bash "${REPO_ROOT}/scripts/build-flatpak.sh"
fi

echo
echo "Portable Linux artifacts ready:"
for requested_arch in "${requested_arches[@]}"; do
  staged_arch="$(stage_arch_name "${requested_arch}")"
  if contains_value appimage "${requested_formats[@]}"; then
    echo "  ${REPO_ROOT}/target/release/vertexlauncher-linux${staged_arch}.AppImage"
  fi
  if contains_value flatpak "${requested_formats[@]}"; then
    echo "  ${REPO_ROOT}/target/release/io.github.SturdyFool10.VertexLauncher-${requested_arch}.flatpak"
  fi
done
