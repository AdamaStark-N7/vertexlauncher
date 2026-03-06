pub mod app {
    pub mod tokio_runtime {
        pub use launcher_runtime::{init, spawn, spawn_blocking};
    }
}

// Reuse the existing UI/screen source files from the launcher crate while
// compiling them as a separate crate boundary to improve incremental rebuilds.
#[path = "../../mclauncher/src/assets/mod.rs"]
pub mod assets;
#[path = "../../mclauncher/src/screens/mod.rs"]
pub mod screens;
#[path = "../../mclauncher/src/ui/mod.rs"]
pub mod ui;
#[path = "../../mclauncher/src/window_effects.rs"]
pub mod window_effects;
