# SE5 migration: remaining work

Goal: the SE5 UI is a **1:1 move-over** from egui. Same screens, features, labels, tooltips, shortcuts and behaviour, with no egui, eframe or replacement UI crates left in the live path.

Legend: `[x]` done (compiles and unit-tested where possible, **not yet exercised in a live window**), `[ ]` to do, `[~]` partial.

Rule while verifying: never launch the launcher against real accounts. Verify through `cargo check/test`, then ask the user to do the live pass.

## 1. Screens and modals

| Area | Status | Notes |
|---|---|---|
| Top bar, sidebar, chrome | [x] | Verify against egui: instance context menu, account menu, window controls, drag region |
| Library | [x] | Verify grid/list modes, drag-reorder, move/delete flows |
| Home (instances, worlds, servers) | [~] | Sync modals done; see 4b for tooltips/ping details |
| Home screenshots | [~] | Cross-instance gallery and viewer done; verify delete, thumbnails and grouping |
| Instance screen | [x] | Verify every tab: Content, Logs, Screenshots, Exports, Settings |
| Instance settings modal | [x] | Verify runtime overrides, autosave, version save |
| Instance export (pack/server) | [x] | |
| Instance move modal | [x] | |
| Content browser | [~] | Verify filters, tag chips, install flow, version matching |
| Discover and detail | [~] | Verify search, detail page, markdown/body rendering, install |
| Settings | [x] | Just rewritten; check every control against egui labels and conditions |
| Console | [x] | ANSI/log polish outstanding |
| Legal | [x] | |
| First-run config-format dialog | [x] | |
| Create instance | [x] | |
| Import instance | [x] | Includes package import flows |
| Sign-in / accounts | [x] | Browser and webview sign-in; verify webview runtime on all platforms |
| **Game settings modal** (`screens/game_settings`, schema-driven by `mc_options`) | [x] | Ported via `game_settings_api`; opened from the instance screen. Verify live |
| **Skins screen** (`skins*.rs`, cape grid, expressions) | [x] | `se5/screens/skins.rs` over `skins_api`; drop zone, model, cape tiles, save. Verify live (layout, drag-and-drop over the zone only vs anywhere) |
| **3D skin viewer** (`skins_preview*`, `vertex_3d`) | [~] | Scene building reused; drawn by a CPU rasteriser (`skins_raster.rs`, unit-tested) into an engine texture updated in place each frame (`Assets::update_texture`). Differences from the GPU path: no tonemap curve, SMAA/TAA approximated by FXAA/2x supersampling, no SSAO. Decide after a live check whether to move this onto SE5 meshes + offscreen target for GPU speed |
| **World / server sync modals** (`home_world_sync`, `home_server_sync`) | [x] | `sync_dialogs.rs`, opened from the Home entry context menu. Verify live |
| **Gamepad navigation** (`gamepad.rs`) | [~] | Device layer split into toolkit-free `poll`; `se5/gamepad.rs` feeds SE5 `FocusNavigator` (D-pad/stick spatial focus, A activate, B back/dismiss, bumpers/right stick scroll, slider steps, skins orbit, viewer pan/zoom). Kit widgets declare themselves focusable and draw rings. Missing: X (context menu on focused item), per-screen explicit focus bridges (spatial nav replaces them), B closing popups/dropdowns. Verify with a real controller |
| **Gamepad calibration modal** | [x] | `screens/gamepad_calibration.rs` over `GamepadCalibrationState::primary`. Verify live |
| Screenshot viewers (home, instance, single) | [x] | Zoom (wheel, triggers) and pan (drag, right stick) added in `screens/viewer.rs`; verify wheel units live |
| Notifications and toasts | [x] | Verify actions, expiry bars, easing, stacking |
| Streamer-mode name scrubbing | [~] | Wrapped on `kit::label`; audit every drawn string, including tooltips and window titles |

## 2. Behaviour and platform

- [x] **Theme live reload**: poll the themes dir and reload `ThemeCatalog` (the Settings tooltip already promises this).
- [x] **Emoji and CJK font pickers**: use `fontloader::FontCatalog::families_covering_any` (the crate no longer depends on egui).
- [ ] **Image loading**: remote project icons (ureq `ImageSource::Url`), caching and eviction parity.
- [ ] **Window effects**: transparency, blur (Linux protocols, Windows backdrop, macOS visual effect) go through the SE5 appliers. Verify each, then delete `window_effects.rs`.
- [ ] **Runtime-applied settings**: confirm every formerly-restart setting really applies live: graphics API, adapter, HDR, frame limiter, text path, window effects. Check the `Degraded`/`Failed` rollback and its notifications.
- [ ] **Drag-and-drop**: file drops for import, content install, skins.
- [ ] **Clipboard, IME and CJK input** parity (egui build had IME support).
- [ ] **File dialogs**: all callers use `file_dialog`.
- [ ] **Single-instance, taskbar progress, Discord presence, OBS detection, app icon**: confirm they work under the SE5 shell.
- [x] **Redraw policy**: `FrameMode::Continuous` with vsync off: frames are drawn as fast as the GPU allows, or exactly at the frame limit when one is set. Idle CPU/GPU use is now whatever the limiter allows, so check it.
- [x] `--legacy-ui` removed. [ ] Re-check CLI flags (`cli.rs`) under the SE5 shell.
- [ ] **Accessibility and keyboard shortcuts** parity.

## 3. Verification

- [ ] Side-by-side screen audit: for each screen, compare the egui source against the SE5 screen, line by line, for labels, tooltips, conditions and empty/error states.
- [ ] Unit tests for the new view-API modules (`discover_api`, `content_browser_api`, `instance_api`, `home_api`).
- [x] `crates/vertexlauncher/tests/se5_widget_lint.rs` replaces the egui text/interaction lints.
- [ ] Live pass by the user on Linux, Windows and macOS: fonts, scaling, blur, HDR, multi-monitor, adapter switching.
- [ ] Performance check: large instance lists, content browser with many results, console with long logs.

## 4. Remove the old stack

- [x] `--legacy-ui`, `app_core*`, `native_options`, egui modals, `persistence`, egui fonts/taskbar/platform code removed from `vertexlauncher`.
- [x] `launcher_ui`: `ui/`, `window_effects`, notification popups, every egui screen/render function removed; what remains is toolkit-free state, workers and view APIs (`geom.rs` holds the few math/colour types the skin scene needs).
- [x] Crates removed: `textui`, `textui_egui`, `ui_foundation`, `modal_host`, `vertex_3d`, `vertex_3d_build`, `skin_preview`. `fontloader` kept as a font-coverage helper without egui.
- [x] Workspace dependencies removed: `eframe`, `egui`, `egui-wgpu`, `egui_extras`, `wgpu`, `glyphon`, `cosmic-text`, `swash`, `etagere`.
- [ ] Dead-code pass in `launcher_ui` (remaining legacy remnants: `home_server_sync.rs`, `home_world_sync.rs`, masonry/thumbnail state, `screenshot_viewer_state`, webview helper leftovers).
- [ ] Packaging: Flatpak manifest, Windows and macOS bundles, CI. Update for the SE5 dependency and drop old libraries.
- [ ] Docs: update `docs/se5-migration.md` Progress with everything done after the first switch, and README screenshots.

## 4b. Parity gaps found by comparing legacy strings (work list)

- [x] Account menu: saved accounts list, refresh/remove, per-account colours, QR device code, webview/browser/device sign-in, coloured name chips (`kit.account_label`).
- [x] Sidebar "+" menu (create / import / link existing profile) and the legacy instance context menu entries.
- [x] Searchable dropdown for long lists (fonts, versions).
- [x] CurseForge API key override applied at startup and on change; default OpenType feature list when the setting is empty.
- [x] Console: line-granular selection (drag over lines), "Copy line / Copy selection / Clear selection".
- [x] Library delete rules and messages (installing / preparing / running / account busy) and "Delete Instance Folder?" wording.
- [x] Screenshot tile hover actions (copy to clipboard, delete) in the Home and Instance galleries; "Loading screenshots..." / "No screenshots found for this instance." states.
- [x] Favorite star tooltips ("Toggle world/server favorite"), server ping tooltips ("Ping unknown", "Server offline", "Latency: Nms").
- [x] Content browser tooltips ("Quick install latest compatible version", "Open mod details", "Already installed in this instance"), detail version actions, instance content tooltips (Disable/Enable mod, Update to this version, Delete this content).
- [x] Discover meta lines were already present; Legal is now the legacy accordion.
- [x] Instance logs description wording; Settings graphics/info wording and the instance settings Linux section match the legacy text. Open: character-level (not line-level) console selection would need a text hit-test API from SE5.
- [x] Gamepad: X context menu on focus, B closing popups.

## 5. Suggested order

1. Theme live reload, game settings modal, world/server sync modals (self-contained).
2. Skins screen plus the 3D viewer (largest piece).
3. Gamepad navigation and calibration.
4. Full side-by-side audit and fixes, with the user's live pass.
5. Remove the old stack and update packaging and docs.

## 6. Build status

`cargo check --workspace` and `cargo test --workspace` pass. The latest UI batch (equal-width tabs, split Open Content
Browser button, icon-only sidebar, top-right toasts, deduped Legal built from `sturdy::images::third_party_licenses`,
info badges on every setting, Linux blur protocol fallback that degrades instead of rolling back, worker-thread skin
rasterising, accelerated + block-compressed screenshot loading, Discover column clipping) is **not yet exercised in a
live window**; it needs the owner's pass.
