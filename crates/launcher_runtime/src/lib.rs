//! The launcher's background runtime: async tasks, blocking tasks, timers and `block_on`.
//!
//! There is no Tokio here. Async tasks run on a small executor, blocking calls run on a thread pool that grows on
//! demand, and timers use the application's timer when it installs one ([`install_async_backend`], the UI engine's) or
//! a single timer thread otherwise. CPU-bound work has its own entry point, [`spawn_compute`].

use std::{
    future::Future,
    pin::Pin,
    sync::OnceLock,
    task::{Context, Poll},
};

mod compute;
mod exec;
pub mod fs;
mod memory;
pub mod process;
mod sync;
mod time;
mod worker_channel;
pub use compute::{
    ComputeBackend, ComputeFuture, ComputeHandle, compute_workers, install_compute_backend, spawn_compute,
};
pub use memory::{BlockingGate, BlockingGatePermit, release_memory_to_os};
pub use sync::{Semaphore, SemaphorePermit};
pub use time::{AsyncBackend, Elapsed, Sleep, install_async_backend, sleep, timeout};
pub use worker_channel::{Drained, WorkerChannel};

static DETACHED_TASK_REPORTER: OnceLock<fn(&str, &TaskError)> = OnceLock::new();

/// Kept for callers that report a failed start. Starting is now infallible, so nothing returns one.
#[derive(Debug, Clone)]
pub struct RuntimeBootstrapError(pub String);

impl std::fmt::Display for RuntimeBootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RuntimeBootstrapError {}

#[derive(Debug, Clone)]
pub enum TaskError {
    RuntimeUnavailable(RuntimeBootstrapError),
    JoinFailed(String),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RuntimeUnavailable(error) => write!(f, "{error}"),
            Self::JoinFailed(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for TaskError {}

pub fn set_detached_task_reporter(reporter: fn(&str, &TaskError)) {
    let _ = DETACHED_TASK_REPORTER.set(reporter);
}

fn log_detached_task_failure(task_kind: &str, error: &TaskError) {
    if let Some(reporter) = DETACHED_TASK_REPORTER.get().copied() {
        reporter(task_kind, error);
    }
    if tracing::dispatcher::has_been_set() {
        tracing::error!(
            target: "vertexlauncher/runtime_bootstrap",
            error = %error,
            task_kind,
            "Detached launcher task failed"
        );
    } else {
        eprintln!("Detached launcher task failed ({task_kind}): {error}");
    }
}

pub struct TaskHandle<T> {
    result: exec::Receiver<T>,
}

impl<T> TaskHandle<T> {
    /// The result if the task has finished, without waiting.
    pub fn try_take(&mut self) -> Option<Result<T, TaskError>> {
        self.result.try_take().map(|result| result.map_err(TaskError::JoinFailed))
    }
}

impl<T> Future for TaskHandle<T> {
    type Output = Result<T, TaskError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.result)
            .poll(cx)
            .map(|result| result.map_err(TaskError::JoinFailed))
    }
}

/// Eagerly starts the executor's threads. Optional: every entry point starts what it needs.
pub fn init() -> Result<(), RuntimeBootstrapError> {
    exec::spawn_boxed(Box::pin(async {}));
    Ok(())
}

/// Spawns an asynchronous task. Dropping the handle does not stop it.
pub fn spawn<F>(future: F) -> TaskHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let (completer, result) = exec::cell();
    exec::spawn_boxed(Box::pin(async move {
        completer.complete(future.await);
    }));
    TaskHandle { result }
}

/// Runs an asynchronous operation to completion on the calling thread.
pub fn block_on<F>(future: F) -> Result<F::Output, TaskError>
where
    F: Future,
{
    Ok(exec::block_on(future))
}

/// Runs a blocking closure on the blocking thread pool.
pub fn spawn_blocking<F, R>(task: F) -> TaskHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (completer, result) = exec::cell();
    exec::spawn_blocking_boxed(Box::new(move || completer.complete(task())));
    TaskHandle { result }
}

/// Spawns an asynchronous task and logs any failure.
pub fn spawn_detached<F>(future: F)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let handle = spawn(future);
    exec::spawn_boxed(Box::pin(async move {
        if let Err(error) = handle.await {
            log_detached_task_failure("detached async task", &error);
        }
    }));
}

/// Runs a blocking closure detached from the caller and logs failures.
pub fn spawn_blocking_detached<F, R>(task: F)
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let handle = spawn_blocking(task);
    exec::spawn_boxed(Box::pin(async move {
        if let Err(error) = handle.await {
            log_detached_task_failure("detached blocking task", &error);
        }
    }));
}
