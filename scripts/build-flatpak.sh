#!/usr/bin/env bash
#
# Rebuilt Flatpak packaging script for VertexLauncher.
#
# This script builds the VertexLauncher binary for x86_64 Linux using the
# provided container build script to ensure it links against glibc ≤2.17.
# It then packages the binary into a Flatpak using the GNOME runtime,
# granting host filesystem and network access.

set -euo pipefail

# Determine repository and script locations
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

# Build a release binary for x86_64 Linux with a glibc ≤2.17.  The existing
# portable build script (build-linux-x86_64-container.sh) compiles inside a
# CentOS 7 container.  If the container script is unavailable, fall back to
# building natively with cargo.
echo "[flatpak] Building x86_64 Linux binary (glibc ≤ 2.17)…"
BIN_TARGET_DIR="${REPO_ROOT}/target/x86_64-unknown-linux-gnu/release"
BIN_PATH="${BIN_TARGET_DIR}/vertexlauncher"

if [[ -x "${REPO_ROOT}/scripts/build-linux-x86_64-container.sh" ]]; then
  # Use the container build script to ensure the glibc floor.  If it fails, the
  # script will exit due to `set -e`.
  bash "${REPO_ROOT}/scripts/build-linux-x86_64-container.sh"
else
  echo "Warning: container build script not found; compiling natively." >&2
  # Ensure the Rust target is installed and compile the binary.
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo is not installed. Please install Rust or provide the container build script." >&2
    exit 1
  fi
  cargo build --release --target x86_64-unknown-linux-gnu --manifest-path "${REPO_ROOT}/Cargo.toml" -p vertexlauncher
fi

# Verify that the binary now exists.
if [[ ! -f "${BIN_PATH}" ]]; then
  echo "Error: expected binary not found at ${BIN_PATH}" >&2
  exit 1
fi

# Set up Flatpak build directories
WORK_DIR="${REPO_ROOT}/flatpak_build"
BUILD_DIR="${WORK_DIR}/build"
REPO_DIR="${WORK_DIR}/repo"
MANIFEST="${WORK_DIR}/io.github.SturdyFool10.VertexLauncher.yml"
RUNTIME_VERSION="${VERTEX_FLATPAK_RUNTIME_BRANCH:-45}"

rm -rf "${WORK_DIR}"
mkdir -p "${BUILD_DIR}" "${REPO_DIR}"

# Generate Flatpak manifest.  Use GNOME runtime and SDK to provide WebKitGTK
# and glib-networking.  Grant host filesystem and network access via
# finish-args.  The binary is copied into /app/bin.
cat > "${MANIFEST}" <<EOF
id: io.github.SturdyFool10.VertexLauncher
runtime: org.gnome.Platform
runtime-version: '${RUNTIME_VERSION}'
sdk: org.gnome.Sdk
command: vertexlauncher
finish-args:
  - --share=network
  - --socket=x11
  - --socket=wayland
  - --device=all
  - --filesystem=host
modules:
  - name: vertexlauncher
    buildsystem: simple
    build-commands:
      - install -D vertexlauncher /app/bin/vertexlauncher
    sources:
      - type: file
        path: ${BIN_PATH}
        dest: vertexlauncher
EOF

# Build the Flatpak.  This will download the GNOME runtime and SDK if they
# are not already installed.  The resulting repository is written to
# ${REPO_DIR}.
echo "[flatpak] Building Flatpak with runtime ${RUNTIME_VERSION}…"
flatpak-builder --force-clean --repo="${REPO_DIR}" "${BUILD_DIR}" "${MANIFEST}"

# Bundle the Flatpak into a single file that can be installed.  The bundle
# includes the application and references the selected runtime.
OUTPUT_BUNDLE="${REPO_ROOT}/io.github.SturdyFool10.VertexLauncher.flatpak"
flatpak build-bundle "${REPO_DIR}" "${OUTPUT_BUNDLE}" io.github.SturdyFool10.VertexLauncher "${RUNTIME_VERSION}"

echo "[flatpak] Build complete: ${OUTPUT_BUNDLE}"