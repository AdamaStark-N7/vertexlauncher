use std::sync::OnceLock;

use tokio::runtime::{Builder, Handle, Runtime};

static AUTH_TOKIO_RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub(crate) fn auth_runtime() -> &'static Runtime {
    AUTH_TOKIO_RUNTIME.get_or_init(build_auth_runtime)
}

pub(crate) fn auth_runtime_handle() -> &'static Handle {
    auth_runtime().handle()
}

fn build_auth_runtime() -> Runtime {
    match Builder::new_multi_thread()
        .enable_all()
        .thread_name("vertex-auth-tokio")
        .build()
    {
        Ok(runtime) => runtime,
        Err(multithread_error) => {
            eprintln!(
                "Failed to build multi-thread auth runtime: {multithread_error}. Falling back to single-thread runtime."
            );

            Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_else(|single_thread_error| {
                    // Without any Tokio runtime we cannot perform login network operations,
                    // so failing both runtime setups is unrecoverable.
                    panic!(
                        "Unrecoverable: failed to build both multi-thread and single-thread auth runtimes. multi-thread error: {multithread_error}; single-thread error: {single_thread_error}"
                    )
                })
        }
    }
}
