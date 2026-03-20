Come to our Discord: [![Discord](https://img.shields.io/discord/1480105103414530190?label=Discord%20Members&logo=discord)](https://discord.gg/EJGUFeuGrN)

# Vertex Launcher

Native Minecraft launcher written in Rust.

Vertex Launcher is a multi-crate desktop launcher with:

- native desktop UI built on `eframe`/`egui`
- Microsoft and Minecraft account sign-in
- multi-instance management
- runtime/bootstrap setup for Minecraft and Java
- in-app Modrinth and CurseForge browsing
- quick-launch CLI flows for packs, worlds, and servers

## Building

If you build from source, install:

- Rust toolchain
- Cargo
- Git
- a working C/C++ toolchain

Windows release artifacts must use the MSVC targets. `windows-gnu` is not part of the supported release matrix.

## Native Linux Prerequisites

On Linux, native launcher builds require GTK, GLib, Soup, and WebKit development packages.

For Debian/Ubuntu:

```sh
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config \
  libglib2.0-dev \
  libgtk-3-dev \
  libgdk-pixbuf-2.0-dev \
  libpango1.0-dev \
  libatk1.0-dev \
  libcairo2-dev \
  libsoup-3.0-dev \
  libwebkit2gtk-4.1-dev \
  libjavascriptcoregtk-4.1-dev
```

If your distro only ships the `4.0` WebKit packages, use:

- `libwebkit2gtk-4.0-dev`
- `libjavascriptcoregtk-4.0-dev`

Basic native builds:

```sh
cargo build --release
```

Linux release binaries should not be built on a rolling distro with plain `cargo build`, because that will inherit the host glibc baseline and the host GTK/WebKit stack. The release scripts build Linux x86-64 in a Debian-based container so the binary is linked against a stable distro baseline instead of your rolling host.

The current Linux UI stack still depends on distro WebKitGTK/GTK libraries, so the final glibc floor is constrained by those packages. The x86-64 container helper now prints the highest required glibc symbol version after each build and defaults to enforcing `VERTEX_MAX_GLIBC_VERSION=2.42`, which matches the lower end of current Fedora-derived gaming distros such as Bazzite and Nobara. Override `VERTEX_MAX_GLIBC_VERSION=<version>` if you want a stricter or looser ceiling.

Windows MSVC example:

```sh
cargo build --release --target x86_64-pc-windows-msvc
```

## Release Matrix

The current supported release artifact matrix is:

- Windows x86-64: `x86_64-pc-windows-msvc`
- Windows ARM64: `aarch64-pc-windows-msvc`
- Linux x86-64: `x86_64-unknown-linux-gnu`
- Linux ARM64: `aarch64-unknown-linux-gnu`
- macOS ARM64: `aarch64-apple-darwin`

Installed Rust targets intentionally not used for release artifacts:

- `x86_64-pc-windows-gnu`
- `armv7-unknown-linux-gnueabihf`
- `x86_64-apple-darwin`

To build the staged release artifacts:

Linux/macOS:

```sh
fish scripts/build-release-artifacts.fish
```

Windows:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-release-artifacts.ps1
```

## Flatpak

Linux users who need the broadest cross-distro compatibility should use the Flatpak package instead of the raw host-linked binary. The Flatpak package carries its own GNOME runtime, so it does not rely on the host distro's glibc, GTK, Soup, or WebKitGTK stack. That is the packaging path intended for current gaming-oriented distros such as Bazzite, Nobara, CachyOS, and similar systems that already support Flatpak.

The Flatpak manifest grants home-directory filesystem access so the launcher can detect and import existing Minecraft / Modrinth / launcher install folders by path instead of being limited to its sandbox-private storage. It also exposes both Wayland and X11 sockets and sets `GDK_BACKEND=wayland,x11`, so the launcher prefers Wayland when available while still allowing Minecraft clients and Java mods to fall back to X11 when needed.

To build the Flatpak bundle automatically:

```sh
bash scripts/build-flatpak.sh
```

This script will:

- vendor Rust dependencies into `flatpak/vendor`
- build the app with `flatpak-builder` against the Flatpak runtime instead of host desktop libraries
- export an arch-specific local Flatpak repo under `flatpak/repo/<arch>`
- emit distributable bundles under `target/release/io.github.SturdyFool10.VertexLauncher-<arch>.flatpak`

By default the standalone helper builds the current Flatpak host architecture. Override `VERTEX_FLATPAK_ARCHES` with a comma-separated list such as `x86_64,aarch64` to request specific arches. Flatpak only allows builds for host-compatible arches unless the ARM64 container helper is used.

The release scripts also invoke the Flatpak helper. On Linux x86-64 they now default to staging both `x86_64` and `aarch64` Flatpak bundles, enabling the ARM64 container helper automatically when needed. Override that with `VERTEX_RELEASE_FLATPAK_ARCHES=<comma-separated arches>`.

The Flatpak app id is `io.github.SturdyFool10.VertexLauncher`.

If you only have an x86-64 Linux builder but want an ARM64 Flatpak, set `VERTEX_FLATPAK_ARCHES=aarch64` and `VERTEX_ENABLE_ARM64_EMULATION=1`. That path delegates the build to `scripts/build-flatpak-arm64-container.sh`, which runs an emulated ARM64 Podman container. It is much slower than a native ARM64 host and depends on working `podman` plus host `binfmt_misc` / QEMU user emulation.

## AppImage

Linux users who want a single-file portable launcher can also build an AppImage. Unlike the Flatpak bundle, the AppImage path still depends on `linuxdeploy` to collect the launcher's native GTK/WebKit stack from the build host, so it should be produced on the same Linux architecture you plan to ship.

When Vertex runs from an AppImage, it now defaults to portable storage beside the AppImage itself unless `VERTEX_CONFIG_LOCATION` is already set. For example, running `VertexLauncher-x86_64.AppImage` will use `VertexLauncher-x86_64.AppImage.data/` for config, instances, cache, logs, and themes.

To build the AppImage bundle automatically on Linux:

```sh
bash scripts/build-appimage.sh
```

This helper expects:

- `linuxdeploy` on `PATH`, or `VERTEX_LINUXDEPLOY=/path/to/linuxdeploy*.AppImage`
- `appimagetool` on `PATH`, or `VERTEX_APPIMAGETOOL=/path/to/appimagetool*.AppImage`
- `linuxdeploy-plugin-gtk` on `PATH` is recommended for the GTK/WebKit runtime bundle, or `VERTEX_LINUXDEPLOY_GTK_PLUGIN=/path/to/linuxdeploy-plugin-gtk*`
- a matching native Linux release binary under `target/<triple>/release/vertexlauncher`

If `linuxdeploy` or `appimagetool` are missing, `build-appimage.sh` now downloads arch-matched AppImage builds into `.cache/appimage-tools/`. Override those download URLs with `VERTEX_LINUXDEPLOY_URL` and `VERTEX_APPIMAGETOOL_URL` if you need pinned mirrors or a private cache.

The staged AppImage artifact is written to `target/release` as one of:

- `vertexlauncher-linuxx86-64.AppImage`
- `vertexlauncher-linuxarm64.AppImage`

On Linux, the AppImage helper now prefers packaging inside a Debian Bookworm container when `podman` is available. That avoids `linuxdeploy` failures on rolling hosts whose system libraries use newer ELF features such as RELR relocations. Set `VERTEX_APPIMAGE_USE_CONTAINER=0` if you need to force host packaging.

If you only have an x86-64 Linux builder but want an ARM64 AppImage, set `VERTEX_APPIMAGE_ARCH=aarch64`. The helper delegates to `scripts/build-appimage-arm64-container.sh`, which runs an emulated ARM64 Podman container and will reuse or download ARM64-compatible `linuxdeploy` / `appimagetool` automatically.

The release scripts now default to building both `vertexlauncher-linuxx86-64.AppImage` and `vertexlauncher-linuxarm64.AppImage` on Linux x86-64. Override that with `VERTEX_RELEASE_APPIMAGE_ARCHES=<comma-separated arches>` or the legacy single-arch `VERTEX_RELEASE_APPIMAGE_ARCH`.

The release scripts still build raw Linux binaries under `target/<triple>/release/vertexlauncher` as packaging intermediates, but they no longer stage those host-linked binaries into `target/release`. The intended Linux release outputs are the AppImage and Flatpak artifacts.

Staged artifacts are written to `target/release` as:

- `vertexlauncher-windowsx86-64.exe`
- `vertexlauncher-windowsarm64.exe`
- `vertexlauncher-linuxx86-64.AppImage`
- `vertexlauncher-linuxarm64.AppImage`
- `vertexlauncher-macosarm64`

## Cross-Build Notes

- Windows cross-builds use `cargo xwin` with the `clang` backend and scrub host-specific compiler flags.
- Linux x86-64 release builds use a containerized Debian userspace so release binaries do not inherit the builder's host glibc and desktop libraries.
- Linux ARM64 release builds use a cross sysroot path. The current helper script can assemble that sysroot for release builds.
- macOS ARM64 release builds require a usable Apple SDK. The scripts detect `SDKROOT`, `DEVELOPER_DIR`, `xcrun`, and `~/.local/share/macos-sdk/MacOSX*.sdk`.

## What The Launcher Can Do

- Create, import, edit, delete, and launch Minecraft instances
- Track favorites and usage metadata per instance
- Sign in with Microsoft accounts and switch between cached accounts
- Auto-provision compatible OpenJDK runtimes when needed
- Resolve and install Minecraft assets, libraries, and version metadata
- Install and update Fabric, Forge, NeoForge, and Quilt content
- Browse Modrinth and CurseForge content inside the launcher
- Filter and install mods, resource packs, shaders, and data packs per instance
- Support direct quick-launch into packs, worlds, and servers from the CLI
- Show notifications, logs, settings, skins, legal/privacy views, and themed UI configuration

## Workspace Layout

- `crates/vertexlauncher`: desktop app entrypoint, app shell, CLI dispatch
- `crates/launcher_ui`: screens, widgets, notifications, desktop UI helpers
- `crates/installation`: Minecraft setup, dependency resolution, Java/runtime provisioning, launch orchestration
- `crates/auth`: Microsoft/Minecraft auth and account state
- `crates/instances`: persisted instance records and related metadata
- `crates/config`: launcher configuration and serialization
- `crates/modprovider`, `crates/modrinth`, `crates/curseforge`: content provider integration
- `crates/runtime_bootstrap`, `crates/launcher_runtime`: async runtime creation and task execution
- `crates/textui`, `crates/fontloader`: text, layout, and font support

## CLI

Quick-launch commands run without opening the full desktop UI.

Launch an instance:

```sh
vertexlauncher --quick-launch-pack --instance <instance-id-or-name> --user <profile-id-or-username>
```

Launch directly into a world:

```sh
vertexlauncher --quick-launch-world --instance <instance-id-or-name> --world <world-folder-name> --user <profile-id-or-username>
```

Launch directly into a server:

```sh
vertexlauncher --quick-launch-server --instance <instance-id-or-name> --server <server-name-or-address> --user <profile-id-or-username>
```

Show quick-launch help:

```sh
vertexlauncher --quick-launch-help
```

List quick-launch targets for an instance:

```sh
vertexlauncher --list-quick-launch-targets --instance <instance-id-or-name>
```

Build launch arguments for scripts or external launchers:

```sh
vertexlauncher --build-quick-launch-args --mode <pack|world|server> --instance <instance-id-or-name> --user <profile-id-or-username> [--world <world-folder-name>] [--server <server-name-or-address>]
```
