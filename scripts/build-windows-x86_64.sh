#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET=x86_64-pc-windows-msvc
STAGED="${REPO_ROOT}/target/release/vertexlauncher-windows-x86_64.exe"

# The engine's third-party sources, downloaded once and shared by every target (sources only; .cache is ignored).
export STURDY_RS_DEPS_DIR="${STURDY_RS_DEPS_DIR:-${REPO_ROOT}/.cache/sturdy-deps}"

# clang-cl uses xwin's splatted MSVC CRT/SDK, and cargo-xwin exports a matching CMake toolchain file
# (CMAKE_TOOLCHAIN_FILE_<target>) that the engine's build script uses to cross compile its C++ for Windows.
env -u CFLAGS -u CXXFLAGS -u LDFLAGS -u CC -u CXX -u AR -u RANLIB -u RUSTFLAGS -u CARGO_BUILD_RUSTFLAGS \
    cargo xwin build --release --target "${TARGET}" --cross-compiler clang-cl -p vertexlauncher

mkdir -p "${REPO_ROOT}/target/release"
cp "${REPO_ROOT}/target/${TARGET}/release/vertexlauncher.exe" "${STAGED}"
echo "Staged: ${STAGED}"
