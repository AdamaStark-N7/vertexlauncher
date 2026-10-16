# Migrating Vertex from egui to SturdyEngine5

Last reviewed against SE5 `origin/main` 9934a54 (2026-10-13; 9934a54 added render-surface and window-manager additions). The SE5 agent's own report is
`Rust/plans/vertex-launcher-gaps.md` in the engine repo; this file is Vertex's plan and its reading of that report.
Vertex code status (see Progress at the end): `vertex_shell` (window host, settings appliers, theme palette),
`vertex_theme` (egui-free theme shared by both UIs) and `config::Config::diff` exist. The egui app is unchanged and
still the shipping UI.

## Constraints

- Vertex stays a Rust app and uses SE5 through `Rust/sturdy`, as a **git dependency pinned by `rev`**:
  `sturdy = { git = "https://github.com/sturdyfool10/sturdyengine5", rev = "7974f50", features = ["raw-window-handle"] }`.
  The engine's CMake build lives in Cargo's `OUT_DIR`, so it builds once per target dir/profile; `sturdy-sys/build.rs`
  only reruns on its own files. Set `STURDY_RS_DEPS_DIR` to share fetched third-party sources between profiles.
- Engine rule: a setting that needs a relaunch is a design failure. Every Vertex setting applies live.
- No Vertex-specific features in the engine (owner's direction to the SE5 agent). Generic gaps go to SE5; anything
  launcher-shaped stays in Vertex.
- The UI is immediate-mode: one draw hook builds the frame; state is plain structs; `FrameMode::OnDemand` +
  `ui_kit::AnimationKeepAlive`. `Rust/examples/shell_demo` in SE5 is the reference for this pattern.

## SE5 state, by what Vertex needs

Legend: **ready** (built and run by the SE5 agent), **built** (compiles, never exercised live), **missing**.

### Settings that used to need a restart

| Vertex setting | SE5 API | State |
|---|---|---|
| Graphics API, adapter profile/explicit adapter, prefer integrated GPU | `Engine::switch_device(DeviceRequest)`, `available_backends/adapters` (with `Presentability`), `Adapter::stable_id`, `device_generation`, device-change events | **ready**, cross-API and rollback included |
| Window transparency Opaque/Transparent | `Engine::set_transparent` + `PresentationSettings::transparent_composition` | **ready** on all platforms, including D3D12 |
| Window blur + platform options (Windows backdrop, Linux protocol, macOS material/blending/state/emphasized) | `set_window_effect_with`, `window_effect_support`, `window_effect_status` | Wayland ext-background-effect and X11 **ready**. KDE blur route, Windows and macOS **built** but never run. |
| HDR, vsync, frame limiter | `set_presentation_settings`, `FrameRateLimit` | **ready**. UI follows a live HDR flip. |
| Fonts, emoji/CJK families, size, weight, OpenType features | `register_ui_font_stack`, `unregister_ui_font`, `FontVariation::weight`, `FontStack.features`, `system_font_families` | **built**; not run live |
| Text rendering path | `Ui::set_text_raster_mode` (Auto/Sdf/Msdf; map Alpha Mask → Sdf) | **built** |
| SVG AA, UI scale | `Ui::clear_svg_cache`, supersampled raster | **built** |
| Theme | Vertex-side (file watch with `hot_reload::FileWatcher`) | Vertex work |
| Skin viewer MSAA, AA mode, motion blur | `render_offscreen_view` per-view render settings | **ready**. AA modes reduced to Off/MSAA/FXAA (see below). |
| Skin viewer texel AA | custom Slang material | Vertex work |
| `linux_set_opengl_driver`, `linux_use_zink_driver` | n/a: these only affect the launched game's environment (`installation/process_control.rs`) | already live, unchanged |

### App shell

| Need | SE5 API | State |
|---|---|---|
| Idle at ~0% | `FrameMode::OnDemand`, `request_redraw[_after]`, `RedrawHandle` | **ready** (1.6% CPU idle in a debug build). The window manager still polls (≤8 ms) rather than blocking. |
| Custom title bar | `set_hit_test_regions`, `toggle_maximized`, `is_maximized/minimized`, `show_system_menu` | **built**; hit-test drag not run |
| Icon, app id | `set_icon`, `RuntimeConfig::app_id` | **built** |
| Taskbar progress, attention | `set_taskbar_progress`, `request_attention` | **built** |
| File drop | `take_drop_events` | **built** |
| Clipboard text / images / file lists | `Window::clipboard_*` | text **ready**, typed **built** |
| `rfd` dialogs and webview parenting | `NativeWindowHandle: HasWindowHandle + HasDisplayHandle` | **built** |
| Logs into `tracing` | `diagnostics::{add_log_sink, set_log_level, set_console_logging}` | **built** |
| OS theme, monitors | `system_theme`, `displays` | **built** |

### Widgets and text

Popups/context menus/tooltips/modals/tabs/progress (`ui_kit`), virtual lists (`visible_rows`, `VariableRows`),
markdown with link events (`markdown::MarkdownView`, `code_colorizer` hook for syntect), rich text, ellipsis,
selectable read-only text, undo/redo, gamepad/keyboard focus (`FocusNavigator` fed from `gilrs`), animated images
(`update_texture`, `AnimatedImage`), nearest sampling. The `shell_demo` UI rendered visually; modal, context menu,
tooltip, drops, live fonts, animated images and `VariableRows` in a live UI have not been run.

## Settled by the owner (2026-10-13; 9934a54 added render-surface and window-manager additions)

The engine report (`vertex-launcher-gaps.md`) understated several items. Per the owner, these are **not** gaps:

- Live opaque ↔ transparent on Linux: works in C++, and the same APIs are exposed in Rust (`Engine::set_transparent`
  + `PresentationSettings::transparent_composition`). No window recreate.
- D3D12 transparent (composition) swapchain: supported.
- Cross-API device switch and rollback: done and tested.
- Adapter presentability: exists (`sturdy::device::Presentability` on each adapter from `available_adapters()`).
- Windows MSVC build: works.
- macOS: **WebGPU only, on purpose** (no test hardware; ~2 macOS users). There is no Metal or MoltenVK path. Vertex's
  "Metal" graphics-API option goes away; on macOS the choice is Auto/WebGPU.
- TAA, FXAA+TAA and SMAA are dropped from Vertex: MSAA looks better. The skin viewer's AA options become Off / MSAA /
  FXAA. Old configs holding Smaa/Taa/FxaaTaa load as MSAA (a `SkinPreviewAaMode` deserialisation fallback).

## What SE5 is still missing

1. **A render-failure signal to Rust.** Not found in `Rust/sturdy/src`; the owner expects it to be possible. Needed so an
   applier can tell that a device switch "succeeded" but frames are failing, and roll back.
2. **A forward/transparent pass** for alpha-blended geometry (the skin's hat/jacket layers; cutout works today). The
   owner considers it doable.
3. Quality only: a true blocking event wait in on-demand mode instead of ≤8 ms polling.
4. Documented behaviour: adopted textures (`TextRenderer::adopt`, raw RHI) go blank after a device switch until the app
   recreates them on `device_generation()` change. Vertex handles this itself.
5. A live run of APIs marked **built** above. Vertex's phase 0 and phase 3 will be the first real user of most of them,
   so expect engine bug reports from those phases.

## Vertex plan

### Phase 0: shell crate (can start now)
- New crate `crates/vertex_shell` depending on `sturdy` (git, pinned rev, `raw-window-handle` feature). Opens the
  window with `app_id`, icon, undecorated + hit-test regions, `FrameMode::OnDemand`, `ui_only()` frames.
- Wire `diagnostics::add_log_sink` into `tracing`, and `RedrawHandle` into the tokio runtime (`launcher_runtime`) so
  background tasks wake the UI.
- Keep `main.rs`'s CLI, webview-helper and single-instance paths before engine startup; `rfd` dialogs parent to
  `NativeWindowHandle`.
- First full engine build happens here: needs clang (C++26), cmake ≥ 3.28, ninja, network or `STURDY_RS_DEPS_DIR`.

### Phase 1: settings appliers
- `crates/vertex_shell/src/settings_apply.rs`: on every config change, `old.diff(&new)` (`config::SettingsDiff`) →
  one applier per `SettingsDomain`: `GraphicsDevice` → `switch_device`; `WindowEffect` → `transparent_composition` +
  `set_window_effect_with`, verdict read a frame later via `window_effect_status`; `Presentation` →
  `set_presentation_settings`; `Text` → font stacks + raster mode + `clear_svg_cache`; `Theme` → reload;
  `SkinPreview` → the viewer's per-view settings.
- An applier returns `Applied | Degraded(reason) | Failed(reason)`. `Failed` restores the old value in `Config` and
  raises a notification; `Degraded` keeps it and notifies (for example, blur → transparent).
- On `device_generation()` change, rebuild adopted text targets and any raw RHI resources.
- Remove "Requires a restart." from `config/src/setting_specs.rs` as each applier lands. A test fails if a spec still
  says "restart" for a domain that has an applier.
- The `GraphicsDevice` applier rolls back on `Failed`, and (once SE5 item 1 lands) also when frames keep failing
  after a switch that reported success.
- `GraphicsApiPreference` becomes Auto/Vulkan/Dx12/WebGpu; on macOS only Auto/WebGPU are offered. Old configs holding
  `Metal` load as Auto.

### Phase 2: UI foundation
- Replace `textui`, `textui_egui`, `ui_foundation` and `modal_host` with a thin Vertex widget layer over `sturdy::ui`,
  `widgets` and `ui_kit` (buttons, toggles with animation, sliders, dropdowns, text inputs, modals, notifications,
  sidebar, context menus). Screens call only this layer.
- Typography roles map to `FontStack` ids/`TextStyle` (Vertex-side; no engine type needed).
- Markdown through `MarkdownView` with a syntect `code_colorizer`; ANSI console colours stay in Vertex
  (`ansi_escapers` → rich spans).

### Phase 3: app shell
- Title bar, navigation, notifications, console view (`VariableRows`), gamepad nav (`gilrs` → `NavInput`), file drop
  import, taskbar progress, Discord presence, OBS detection, streamer mode.

### Phase 4: screens
Settings first (it exercises every applier), then accounts/sign-in, home, library/instances, instance detail and
settings modal, discover/content browser, import/export, screenshots (BC textures via `allow_compression`), skins,
legal.

### Phase 5: skin viewer
- Port `vertex_3d`/`skin_preview` from wgpu to `render_offscreen_view` + `resize_offscreen_target`, with pixel-art
  textures via `create_texture_sampled(nearest)`, a Slang skin material (cutout layers, texel-boundary AA) and motion
  blur from real motion vectors (`MotionBlurSettings`).
- AA modes: Off, MSAA (samples rounded to 1/2/4/8), FXAA. SMAA/TAA/FXAA+TAA are removed.
- Alpha-blended hat/jacket layers wait on SE5 item 2; until then they render as cutout.

### Phase 6: removal and packaging
- Remove `eframe`, `egui*`, `wgpu`, `glyphon`, `cosmic-text` and the hand-written platform window-effect code
  (`launcher_ui/src/window_effects.rs`, the x11/wayland/objc deps).
- Flatpak: run `Rust/tools/prefetch-deps.sh`, set `STURDY_RS_DEPS_DIR` + `STURDY_RS_OFFLINE=1`, and use the clang SDK
  extension. Revisit `flatpak-cargo-generator.py` for the git dependency.
- macOS ships on WebGPU only.

## Risks

- Most of the app-shell and widget APIs have compiled but never run. Phase 0/3 will be their first real user, so expect
  to file engine bugs.
- macOS on WebGPU has no tester; it ships best-effort.
- Porting ~80k lines of egui layout code to a different immediate-mode layout model is not mechanical; do it screen by
  screen behind the widget layer.

## Progress

**The egui front end is gone.** The launcher runs only on SE5 (`app::run_se5`); `--legacy-ui`, `eframe`, `egui`,
`egui_extras`, `wgpu`, `textui`, `textui_egui`, `ui_foundation`, `modal_host`, `vertex_3d(_build)` and `skin_preview`
are removed from the workspace. Nothing has been run in a live window yet (the live pass is the owner's); the
workspace compiled and its tests passed at the last build, and the work since then is listed in
`docs/se5-remaining.md`.

Where things live now:
- `crates/vertex_shell`: window/runtime shell, live settings appliers (device, window effect, presentation) with
  rollback, the palette.
- `crates/vertexlauncher/src/app/se5/`: `kit.rs` (every widget and text draw; focus rings, controller navigation,
  searchable dropdowns, account-name chips, log selection), `chrome.rs` (title bar, sidebar, "+" profile menu,
  instance context menu), `accounts.rs` (account menu with colours/refresh/remove, QR device code, browser/webview/
  device sign-in), `gamepad.rs` (controller to focus navigation), `screens/*` (Home, Library, Instance + dialogs,
  Discover, Content Browser, Skins, Settings, Console, Legal, Game settings, sync dialogs, calibration, viewers).
- `crates/launcher_ui`: no UI toolkit. State, workers and the toolkit-free view APIs the screens call
  (`home_api`, `instance_api`, `discover_api`, `content_browser_api`, `skins_api`, `game_settings_api`), the skin
  scene builder and its CPU rasteriser (`skins_raster.rs`), notifications, `geom.rs`, `qr.rs`.
- `crates/fontloader`: font lookup and script coverage only.

Behaviour that changed on purpose: the skin preview is drawn by the CPU rasteriser into an engine texture updated in
place each frame (no tonemap curve or SSAO; SMAA/TAA approximated); the log viewer selects whole lines; dropdowns
with many options get a search box; the engine's per-window icon, taskbar progress and title carry install
progress.

See `docs/se5-remaining.md` for what is still open (mostly verification, a dead-code pass and packaging checks).
