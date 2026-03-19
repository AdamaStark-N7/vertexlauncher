#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <elf-binary>" >&2
  exit 2
fi

binary_path="$1"

if [[ ! -f "${binary_path}" ]]; then
  echo "missing binary: ${binary_path}" >&2
  exit 2
fi

if ! command -v objdump >/dev/null 2>&1; then
  echo "objdump is required to inspect glibc symbol versions." >&2
  exit 2
fi

versions="$(
  objdump -T "${binary_path}" \
    | grep -oE 'GLIBC_[0-9]+\.[0-9]+' \
    | sort -Vu
)"

if [[ -z "${versions}" ]]; then
  echo "no GLIBC symbol versions found in ${binary_path}" >&2
  exit 1
fi

max_version="$(printf '%s\n' "${versions}" | tail -n 1)"
printf '%s\n' "${versions}" >&2
printf '%s\n' "${max_version}"
