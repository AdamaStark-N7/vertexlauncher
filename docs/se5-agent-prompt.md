# Prompt for the SturdyEngine5 agent (round 2)

Paste everything below the line. Reviewed against d2bd963 with corrections from the owner.

---

Continue the Vertex Launcher work in `Rust/plans/vertex-launcher-gaps.md`. Same rules: generic engine features only,
nothing may need a relaunch, Rust bindings in `Rust/sturdy`, tests (GPU tests where GPU-visible), push to
`origin/main`, and record honestly what ran versus what only compiled.

First, correct the report. The owner confirms these work and the report should stop listing them as open: live
opaque ↔ transparent windows on Linux (C++ works, same APIs in Rust), the D3D12 composition swapchain, cross-API device
switching and rollback, per-adapter presentability, and the Windows MSVC build. macOS is WebGPU-only by design; say so
in `available_backends()` docs and the README. TAA, FXAA+TAA and SMAA are not wanted.

Then build:

1. **Render-failure signal in Rust.** The frame callback runs even when the frame fails to render. Expose a per-frame
   result or a failed-frame counter with the last error (window and `render_offscreen_view`), so an app can detect that a
   device switch reported success but frames are failing, and roll back.
2. **Forward transparent pass** for alpha-blended geometry in the deferred graph: sorted back to front, depth-tested
   against scene depth, lit by the same lights, usable by custom Slang materials, and working in `render_offscreen_view`
   with MSAA. Cutout already works; this is for thin translucent layers.
3. **Blocking idle wait** in `FrameMode::OnDemand`: replace the ≤8 ms window-manager polling with a blocking wait woken
   by input, redraw requests, main-thread jobs and timers. Measure idle CPU in a release build before and after.
4. Document that adopted textures (`TextRenderer::adopt`, raw RHI textures shown in the UI) must be recreated by the app
   on `device_generation()` change, or re-upload them from a retained CPU copy if that is simple.
