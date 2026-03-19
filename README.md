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

To build the Flatpak bundle automatically:

```sh
bash scripts/build-flatpak.sh
```

This script will:

- vendor Rust dependencies into `flatpak/vendor`
- build the app with `flatpak-builder` against the Flatpak runtime instead of host desktop libraries
- export an arch-specific local Flatpak repo under `flatpak/repo/<arch>`
- emit distributable bundles under `target/release/io.github.SturdyFool10.VertexLauncher-<arch>.flatpak`

By default the script builds the current Flatpak host architecture. Override `VERTEX_FLATPAK_ARCHES` with a comma-separated list such as `x86_64,aarch64` to request specific arches. Flatpak only allows builds for host-compatible arches, so producing both x86-64 and ARM64 bundles requires running the script on builders that support each architecture.

The release scripts also invoke the Flatpak helper so `build-release-artifacts` / your compile-all flow stages Flatpak bundles alongside the native release artifacts when Flatpak tooling is available.

The Flatpak app id is `io.github.SturdyFool10.VertexLauncher`.

Staged artifacts are written to `target/release` as:

- `vertexlauncher-windowsx86-64.exe`
- `vertexlauncher-windowsarm64.exe`
- `vertexlauncher-linuxx86-64`
- `vertexlauncher-linuxarm64`
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
