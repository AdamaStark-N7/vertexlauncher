#!/usr/bin/env fish

set -g script_dir (path dirname (status filename))
set -g repo_root (path resolve $script_dir/..)

set -g package vertexlauncher
set -g release_dir $repo_root/target/release
set -g windows_targets x86_64-pc-windows-msvc aarch64-pc-windows-msvc
set -g linux_targets x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
set -g macos_targets aarch64-apple-darwin
set -g flatpak_app_id io.github.SturdyFool10.VertexLauncher
set -g flatpak_branch stable
set -g flatpak_artifact_arches
set -g appimage_artifact_arches
set -g build_failures
set -g staged_artifacts \
    vertexlauncher-windowsx86-64.exe \
    vertexlauncher-windowsarm64.exe \
    vertexlauncher-linuxx86-64 \
    vertexlauncher-linuxarm64 \
    vertexlauncher-linuxx86-64.AppImage \
    vertexlauncher-linuxarm64.AppImage \
    vertexlauncher-macosarm64 \
    vertexlauncher-windows-x86-64.exe \
    vertexlauncher-windows-arm64.exe \
    vertexlauncher-linux-arm64 \
    vertexlauncher-linux-arm64.AppImage \
    vertexlauncher-macos-aarch64 \
    vertexlauncher-windows-x86_64.exe \
    vertexlauncher-linux-x86_64 \
    vertexlauncher-linux-x86_64.AppImage \
    vertexlauncher-macos-x86_64 \
    $flatpak_app_id-x86_64.flatpak \
    $flatpak_app_id-aarch64.flatpak

function require_command
    set -l command_name $argv[1]
    set -l install_hint $argv[2]
    if not command -sq $command_name
        echo "Missing $command_name. $install_hint" >&2
        exit 1
    end
end

function artifact_name
    set -l platform $argv[1]
    set -l arch $argv[2]
    set -l ext $argv[3]
    printf "%s/vertexlauncher-%s%s%s\n" $release_dir $platform $arch $ext
end

function normalize_packaging_arch
    set -l requested_arch $argv[1]
    switch $requested_arch
        case x86_64 amd64 x86-64
            echo x86_64
            return 0
        case aarch64 arm64
            echo aarch64
            return 0
    end

    return 1
end

function default_release_linux_arches
    if test (uname -s) != Linux
        return 1
    end

    switch (uname -m)
        case x86_64 amd64
            echo x86_64
            echo aarch64
            return 0
        case aarch64 arm64
            echo aarch64
            return 0
    end

    return 1
end

function copy_artifact
    set -l source_path $argv[1]
    set -l staged_path $argv[2]
    if not test -f $source_path
        echo "Missing built artifact: $source_path" >&2
        return 1
    end
    cp -f $source_path $staged_path
    or return $status
end

function note_failure
    set -g build_failures $build_failures $argv[1]
end

function clear_staged_artifacts
    for artifact in $staged_artifacts
        rm -f $release_dir/$artifact
    end
end

function has_cross_pkg_config
    if set -q PKG_CONFIG
        return 0
    end
    if set -q PKG_CONFIG_ALLOW_CROSS
        if set -q PKG_CONFIG_SYSROOT_DIR; or set -q PKG_CONFIG_LIBDIR; or set -q PKG_CONFIG_PATH
            return 0
        end
    end
    return 1
end

function has_macos_sdk
    if test -n (resolve_macos_sdk)
        return 0
    end
    return 1
end

function resolve_macos_sdk
    if set -q SDKROOT
        test -d $SDKROOT
        and echo $SDKROOT
        and return 0
    end
    if set -q DEVELOPER_DIR
        test -d $DEVELOPER_DIR
        and echo $DEVELOPER_DIR/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk
        and return 0
    end
    if command -sq xcrun
        set -l xcrun_sdk (xcrun --sdk macosx --show-sdk-path 2>/dev/null)
        if test $status -eq 0 -a -n "$xcrun_sdk"
            echo $xcrun_sdk
            return 0
        end
    end

    for candidate in $HOME/.local/share/macos-sdk/MacOSX.sdk $HOME/.local/share/macos-sdk/MacOSX*.sdk
        if test -d $candidate
            echo $candidate
            return 0
        end
    end

    return 1
end

function build_windows_target
    set -l target $argv[1]
    set -l arch $argv[2]
    set -l source_path $repo_root/target/$target/release/$package.exe
    set -l staged_path (artifact_name windows $arch .exe)

    echo "Building Windows $arch release binary..."
    env -u CFLAGS -u CXXFLAGS -u LDFLAGS -u CC -u CXX -u AR -u RANLIB -u RUSTFLAGS -u CARGO_BUILD_RUSTFLAGS \
        cargo xwin build --release --target $target --cross-compiler clang -p $package
    or return $status

    copy_artifact $source_path $staged_path
    echo "  Staged: $staged_path"
end

function build_linux_target
    set -l target $argv[1]
    set -l arch $argv[2]
    set -l source_path $repo_root/target/$target/release/$package
    set -l staged_path (artifact_name linux $arch "")

    echo "Building Linux $arch release binary..."
    if test $target = x86_64-unknown-linux-gnu
        if test -x $repo_root/scripts/build-linux-x86_64-container.sh
            if not command -sq podman
                echo "Skipping Linux $arch: podman is required for the containerized release helper." >&2
                return 2
            end

            bash $repo_root/scripts/build-linux-x86_64-container.sh
            or return $status

            copy_artifact $source_path $staged_path
            or return $status
            echo "  Staged: $staged_path"
            return 0
        end
    end

    if test $target = aarch64-unknown-linux-gnu
        if test -x $repo_root/scripts/build-linux-arm64-container.sh
            if not command -sq podman
                echo "Skipping Linux $arch: podman is required for the containerized cross-build helper." >&2
                return 2
            end

            bash $repo_root/scripts/build-linux-arm64-container.sh
            or return $status

            copy_artifact $source_path $staged_path
            or return $status
            echo "  Staged: $staged_path"
            return 0
        end

        if not has_cross_pkg_config
            echo "Skipping Linux $arch: configure pkg-config for cross-compilation first." >&2
            echo "  Required: PKG_CONFIG_ALLOW_CROSS=1 plus either PKG_CONFIG=<wrapper> or PKG_CONFIG_SYSROOT_DIR with PKG_CONFIG_PATH/PKG_CONFIG_LIBDIR." >&2
            return 2
        end
    end

    env -u CFLAGS -u CXXFLAGS -u LDFLAGS -u CC -u CXX -u AR -u RANLIB -u RUSTFLAGS -u CARGO_BUILD_RUSTFLAGS \
        cargo zigbuild --release --target $target -p $package
    or return $status

    copy_artifact $source_path $staged_path
    echo "  Staged: $staged_path"
end

function build_macos_target
    set -l target $argv[1]
    set -l arch $argv[2]
    set -l source_path $repo_root/target/$target/release/$package
    set -l staged_path (artifact_name macos $arch "")

    echo "Building macOS $arch release binary..."
    if not has_macos_sdk
        echo "Skipping macOS $arch: no Apple SDK found." >&2
        echo "  Required: SDKROOT=<MacOSX.sdk path>, DEVELOPER_DIR=<Xcode path>, xcrun on PATH, or ~/.local/share/macos-sdk/MacOSX*.sdk." >&2
        return 2
    end

    set -l sdk_root (resolve_macos_sdk)
    env -u CFLAGS -u CXXFLAGS -u LDFLAGS -u CC -u CXX -u AR -u RANLIB -u RUSTFLAGS -u CARGO_BUILD_RUSTFLAGS \
        SDKROOT=$sdk_root cargo zigbuild --release --target $target -p $package
    or return $status

    copy_artifact $source_path $staged_path
    echo "  Staged: $staged_path"
end

function flatpak_artifact_name
    set -l arch $argv[1]
    printf "%s/%s-%s.flatpak\n" $release_dir $flatpak_app_id $arch
end

function appimage_artifact_name
    set -l arch $argv[1]
    printf "%s/vertexlauncher-linux%s.AppImage\n" $release_dir $arch
end

function build_flatpak_artifacts
    set -l helper $repo_root/scripts/build-flatpak.sh

    if not test -x $helper
        echo "Skipping Flatpak: missing helper script $helper" >&2
        return 2
    end

    if not command -sq bash
        echo "Skipping Flatpak: bash is required." >&2
        return 2
    end

    if not command -sq flatpak
        echo "Skipping Flatpak: flatpak is required." >&2
        return 2
    end

    set -l raw_requested_arches
    if set -q VERTEX_RELEASE_FLATPAK_ARCHES
        set raw_requested_arches (string split , -- $VERTEX_RELEASE_FLATPAK_ARCHES)
    else if set -q VERTEX_FLATPAK_ARCHES
        set raw_requested_arches (string split , -- $VERTEX_FLATPAK_ARCHES)
    else
        set raw_requested_arches (default_release_linux_arches)
        if test $status -ne 0 -o (count $raw_requested_arches) -eq 0
            set raw_requested_arches (flatpak --default-arch)
        end
    end

    set -l requested_arches
    for arch in $raw_requested_arches
        if test -z "$arch"
            continue
        end

        set -l normalized_arch (normalize_packaging_arch $arch)
        if test $status -ne 0
            echo "Skipping Flatpak: unsupported architecture $arch." >&2
            return 2
        end

        if not contains -- $normalized_arch $requested_arches
            set -a requested_arches $normalized_arch
        end
    end

    echo "Building Flatpak release bundle..."
    set -l build_env \
        VERTEX_FLATPAK_BRANCH=$flatpak_branch \
        VERTEX_FLATPAK_ARCHES=(string join , $requested_arches)
    if contains -- aarch64 $requested_arches
        set -a build_env VERTEX_ENABLE_ARM64_EMULATION=1
    end

    env $build_env bash $helper
    or return $status

    set -g flatpak_artifact_arches
    for arch in $requested_arches
        if test -z "$arch"
            continue
        end

        set -l artifact_path (flatpak_artifact_name $arch)
        if not test -f $artifact_path
            echo "Missing built Flatpak artifact: $artifact_path" >&2
            return 1
        end

        set -ga flatpak_artifact_arches $arch
        echo "  Staged: $artifact_path"
    end
end

function current_linux_appimage_arch
    set -l kernel_name (uname -s)
    if test "$kernel_name" != Linux
        return 1
    end

    switch (uname -m)
        case x86_64 amd64
            echo x86_64
            return 0
        case aarch64 arm64
            echo aarch64
            return 0
    end

    return 1
end

function build_appimage_artifacts
    set -l helper $repo_root/scripts/build-appimage.sh

    if not test -x $helper
        echo "Skipping AppImage: missing helper script $helper" >&2
        return 2
    end

    if not command -sq bash
        echo "Skipping AppImage: bash is required." >&2
        return 2
    end

    set -l raw_requested_arches
    if set -q VERTEX_RELEASE_APPIMAGE_ARCHES
        set raw_requested_arches (string split , -- $VERTEX_RELEASE_APPIMAGE_ARCHES)
    else if set -q VERTEX_APPIMAGE_ARCHES
        set raw_requested_arches (string split , -- $VERTEX_APPIMAGE_ARCHES)
    else if set -q VERTEX_RELEASE_APPIMAGE_ARCH
        set raw_requested_arches $VERTEX_RELEASE_APPIMAGE_ARCH
    else if set -q VERTEX_APPIMAGE_ARCH
        set raw_requested_arches $VERTEX_APPIMAGE_ARCH
    else
        set raw_requested_arches (default_release_linux_arches)
        if test $status -ne 0 -o (count $raw_requested_arches) -eq 0
            set raw_requested_arches (current_linux_appimage_arch)
        end
    end

    if test (count $raw_requested_arches) -eq 0
        echo "Skipping AppImage: only native Linux hosts are supported." >&2
        return 2
    end

    set -g appimage_artifact_arches
    set -l requested_arches
    for arch in $raw_requested_arches
        if test -z "$arch"
            continue
        end

        set -l requested_arch (normalize_packaging_arch $arch)
        if test $status -ne 0
            echo "Skipping AppImage: unsupported architecture $arch." >&2
            return 2
        end

        if contains -- $requested_arch $requested_arches
            continue
        end
        set -a requested_arches $requested_arch

        set -l target
        set -l staged_arch
        switch $requested_arch
            case x86_64
                set target x86_64-unknown-linux-gnu
                set staged_arch x86-64
            case aarch64
                set target aarch64-unknown-linux-gnu
                set staged_arch arm64
        end

        echo "Building AppImage release bundle for $staged_arch..."
        set -l build_env \
            VERTEX_APPIMAGE_ARCH=$requested_arch \
            VERTEX_APPIMAGE_TARGET=$target \
            VERTEX_APPIMAGE_SOURCE=$repo_root/target/$target/release/$package
        if test "$requested_arch" = aarch64
            set -a build_env VERTEX_ENABLE_ARM64_EMULATION=1
        end

        env $build_env bash $helper
        or return $status

        set -l artifact_path (appimage_artifact_name $staged_arch)
        if not test -f $artifact_path
            echo "Missing built AppImage artifact: $artifact_path" >&2
            return 1
        end

        set -ga appimage_artifact_arches $staged_arch
        echo "  Staged: $artifact_path"
    end
end

cd $repo_root; or exit 1
mkdir -p $release_dir
or exit $status
clear_staged_artifacts

require_command cargo "Install Rust/Cargo first."

if not cargo xwin --version >/dev/null 2>&1
    echo "Missing cargo-xwin. Install it with: cargo install --locked cargo-xwin" >&2
    exit 1
end

if not cargo zigbuild --help >/dev/null 2>&1
    echo "Missing cargo-zigbuild. Install it with: cargo install --locked cargo-zigbuild" >&2
    exit 1
end

for target in $windows_targets
    switch $target
        case x86_64-pc-windows-msvc
            build_windows_target $target x86-64
            or note_failure "Windows x86-64 build failed."
        case aarch64-pc-windows-msvc
            build_windows_target $target arm64
            or note_failure "Windows arm64 build failed."
    end
end

for target in $linux_targets
    switch $target
        case x86_64-unknown-linux-gnu
            build_linux_target $target x86-64
            or note_failure "Linux x86-64 build failed."
        case aarch64-unknown-linux-gnu
            build_linux_target $target arm64
            or note_failure "Linux arm64 build requires a cross pkg-config sysroot/wrapper."
    end
end

for target in $macos_targets
    switch $target
        case aarch64-apple-darwin
            build_macos_target $target arm64
            or note_failure "macOS arm64 build requires an Apple SDK via SDKROOT, DEVELOPER_DIR, or xcrun."
    end
end

build_flatpak_artifacts
or note_failure "Flatpak build failed."

build_appimage_artifacts
or note_failure "AppImage build failed."

echo ""
echo "Artifacts ready:"
echo "  "(artifact_name windows x86-64 .exe)
echo "  "(artifact_name windows arm64 .exe)
echo "  "(artifact_name linux x86-64 "")
echo "  "(artifact_name linux arm64 "")
echo "  "(artifact_name macos arm64 "")
for arch in $flatpak_artifact_arches
    echo "  "(flatpak_artifact_name $arch)
end
for arch in $appimage_artifact_arches
    echo "  "(appimage_artifact_name $arch)
end

if test (count $build_failures) -gt 0
    echo ""
    echo "Build matrix incomplete:" >&2
    for failure in $build_failures
        echo "  - $failure" >&2
    end
    exit 1
end
