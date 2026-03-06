use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};

static TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn build_runtime() -> Runtime {
    match Builder::new_multi_thread()
        .enable_all()
        .thread_name("vertex-tokio")
        .build()
    {
        Ok(runtime) => runtime,
        Err(multithread_error) => {
            eprintln!(
                "Failed to build multi-thread launcher runtime: {multithread_error}. Falling back to single-thread runtime."
            );

            Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_else(|single_thread_error| {
                    // If Tokio cannot initialize in either mode, async work cannot run at all.
                    // This is considered unrecoverable for the launcher process.
                    panic!(
                        "Unrecoverable: failed to build both multi-thread and single-thread launcher runtimes. multi-thread error: {multithread_error}; single-thread error: {single_thread_error}"
                    )
                })
        }
    }
}

fn runtime() -> &'static Runtime {
    TOKIO_RUNTIME.get_or_init(build_runtime)
}

pub fn init() {
    let _ = runtime();
}

pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}

pub fn spawn_blocking<F, R>(task: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    runtime().spawn_blocking(task)
}
