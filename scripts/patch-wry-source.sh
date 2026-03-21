#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
CARGO_HOME_DIR="${CARGO_HOME:-${HOME}/.cargo}"
WRY_VERSION="0.24.11"

find_wry_source_dir() {
  local registry_src="${CARGO_HOME_DIR}/registry/src"

  if [[ ! -d "${registry_src}" ]]; then
    return 0
  fi

  find "${registry_src}" -maxdepth 2 -type d -name "wry-${WRY_VERSION}" | head -n 1
}

wry_source_dir="$(find_wry_source_dir)"

if [[ -z "${wry_source_dir}" ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required to fetch the cached wry source." >&2
    exit 1
  fi

  echo "[wry-patch] wry ${WRY_VERSION} not cached yet; fetching crate sources..."
  cargo fetch --locked --manifest-path "${REPO_ROOT}/crates/vertexlauncher/Cargo.toml" >/dev/null
  wry_source_dir="$(find_wry_source_dir)"
fi

if [[ -z "${wry_source_dir}" ]]; then
  echo "Could not find cached wry source for version ${WRY_VERSION} under ${CARGO_HOME_DIR}/registry/src." >&2
  exit 1
fi

target_file="${wry_source_dir}/src/webview/webkitgtk/mod.rs"
if [[ ! -f "${target_file}" ]]; then
  echo "Missing expected wry source file: ${target_file}" >&2
  exit 1
fi

needle='use webkit2gtk::traits::SettingsExt;'
anchor='use webkit2gtk_sys::{'

if grep -Fqx "${needle}" "${target_file}"; then
  echo "[wry-patch] already patched: ${target_file}"
  exit 0
fi

target_dir="$(dirname -- "${target_file}")"
tmp_file="$(mktemp "${target_dir}/mod.rs.XXXXXX")"
cleanup() {
  rm -f "${tmp_file}"
}
trap cleanup EXIT

if ! awk -v needle="${needle}" -v anchor="${anchor}" '
  $0 == anchor {
    print needle
    patched = 1
  }
  { print }
  END {
    if (!patched) {
      exit 1
    }
  }
' "${target_file}" > "${tmp_file}"; then
  echo "[wry-patch] failed: could not find anchor in ${target_file}" >&2
  exit 1
fi

mv "${tmp_file}" "${target_file}"
trap - EXIT
echo "[wry-patch] patched: ${target_file}"
