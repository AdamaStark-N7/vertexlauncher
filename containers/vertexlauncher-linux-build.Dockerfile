# Build image for the portable Linux binaries: x86_64 natively, aarch64 cross compiled (no emulation while compiling)
# against an arm64 sysroot at /opt/sysroot-aarch64.
#
# Ubuntu 24.04 keeps the glibc floor at 2.39. The engine (SturdyEngine5, C++26) needs a newer toolchain than the
# distribution ships, so clang comes from apt.llvm.org and CMake from Kitware; libstdc++ 14 (std::ranges::to and
# friends) is in the archive. The engine links libstdc++ statically.

ARG LLVM_VERSION=21

# Development libraries, shared by both architectures: the launcher's own (GTK/WebKit for the sign-in webview, udev for
# gamepads) and the engine's window system (SDL3 builds against the X11/Wayland headers and loads them at runtime).
ARG DEV_PACKAGES="g++-14 libglib2.0-dev libgtk-3-dev libgdk-pixbuf-2.0-dev libpango1.0-dev libatk1.0-dev libcairo2-dev \
    libdbus-1-dev libsoup-3.0-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libudev-dev libx11-dev libxext-dev \
    libxrandr-dev libxcursor-dev libxi-dev libxinerama-dev libxss-dev libxfixes-dev libx11-xcb-dev libxkbcommon-dev libwayland-dev \
    libdecor-0-dev libegl-dev libdrm-dev libgbm-dev libasound2-dev libpulse-dev"

# The arm64 sysroot: the same packages installed in a real arm64 Ubuntu (run under qemu; installing only), copied out
# below. The -dev packages of the two architectures cannot be co-installed through multiarch (WebKit conflicts).
FROM --platform=linux/arm64 ubuntu:24.04 AS sysroot-aarch64
ARG DEV_PACKAGES
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends ${DEV_PACKAGES} && rm -rf /var/lib/apt/lists/*

FROM --platform=linux/amd64 ubuntu:24.04
ARG LLVM_VERSION
ARG DEV_PACKAGES
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl gnupg \
 && curl -fsSL https://apt.llvm.org/llvm-snapshot.gpg.key | gpg --dearmor -o /usr/share/keyrings/llvm.gpg \
 && echo "deb [signed-by=/usr/share/keyrings/llvm.gpg] http://apt.llvm.org/noble/ llvm-toolchain-noble-${LLVM_VERSION} main" \
      > /etc/apt/sources.list.d/llvm.list \
 && curl -fsSL https://apt.kitware.com/keys/kitware-archive-latest.asc | gpg --dearmor -o /usr/share/keyrings/kitware.gpg \
 && echo "deb [signed-by=/usr/share/keyrings/kitware.gpg] https://apt.kitware.com/ubuntu/ noble main" \
      > /etc/apt/sources.list.d/kitware.list \
 && apt-get update && apt-get install -y --no-install-recommends \
      build-essential git python3 pkg-config ninja-build cmake patchelf file desktop-file-utils binutils \
      clang-${LLVM_VERSION} lld-${LLVM_VERSION} llvm-${LLVM_VERSION} \
      wayland-protocols libwayland-bin ${DEV_PACKAGES} \
 && for tool in clang clang++ ld.lld lld llvm-ar llvm-ranlib llvm-strip; do \
      ln -sf "/usr/bin/${tool}-${LLVM_VERSION}" "/usr/local/bin/${tool}"; \
    done \
 && rm -rf /var/lib/apt/lists/*

# Ubuntu is merged-/usr (/lib -> usr/lib), so /usr carries everything a sysroot needs; keep the top-level links.
COPY --from=sysroot-aarch64 /usr /opt/sysroot-aarch64/usr
RUN cd /opt/sysroot-aarch64 && ln -s usr/lib lib && ln -s usr/bin bin && ln -s usr/sbin sbin
