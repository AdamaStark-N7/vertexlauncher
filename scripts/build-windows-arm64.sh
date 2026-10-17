#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET=aarch64-pc-windows-msvc
STAGED="${REPO_ROOT}/target/release/vertexlauncher-windows-arm64.exe"

# The engine's third-party sources, downloaded once and shared by every target (sources only; .cache is ignored).
export STURDY_RS_DEPS_DIR="${STURDY_RS_DEPS_DIR:-${REPO_ROOT}/.cache/sturdy-deps}"

# clang-cl uses xwin's splatted MSVC CRT/SDK, and cargo-xwin exports a matching CMake toolchain file
# (CMAKE_TOOLCHAIN_FILE_<target>) that the engine's build script uses to cross compile its C++ for Windows.
#
# One wrinkle on arm64: ring builds its C/assembly with GNU-style `clang` for this target, but cargo-xwin's C flags are
# clang-cl spellings (`/imsvc <dir>`), which `clang` reads as file names. So take cargo-xwin's environment, drop the
# include flags from the C flags, and hand both drivers the same headers through INCLUDE, which each of them honours.
unset CFLAGS CXXFLAGS LDFLAGS CC CXX AR RANLIB RUSTFLAGS CARGO_BUILD_RUSTFLAGS
eval "$(cargo xwin env --target "${TARGET}" --cross-compiler clang-cl)"
XWIN_DIR="${XWIN_CACHE_DIR:-${XDG_CACHE_HOME:-${HOME}/.cache}/cargo-xwin}/xwin"
export INCLUDE="${XWIN_DIR}/crt/include;${XWIN_DIR}/sdk/include/ucrt;${XWIN_DIR}/sdk/include/um;${XWIN_DIR}/sdk/include/shared;${XWIN_DIR}/sdk/include/winrt"
export CFLAGS_aarch64_pc_windows_msvc="--target=${TARGET} -Wno-unused-command-line-argument"
cargo build --release --target "${TARGET}" -p vertexlauncher

mkdir -p "${REPO_ROOT}/target/release"
cp "${REPO_ROOT}/target/${TARGET}/release/vertexlauncher.exe" "${STAGED}"
echo "Staged: ${STAGED}"
