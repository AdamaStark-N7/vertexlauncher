//! CPU work on whichever executor the application provides.
//!
//! The launcher's own Tokio runtime ([`crate::spawn`]) is for IO: it has a reactor, and a large blocking pool for
//! calls that wait. CPU-bound work is different: it should be spread over exactly the machine's cores, next to the UI
//! engine's own jobs rather than as a second set of threads competing with them. The application installs the UI
//! engine's task scheduler as the compute backend ([`install_compute_backend`]); [`spawn_compute`] then runs futures
//! there, and a future that wants more parallelism spawns more of them and awaits their handles, which hands its worker
//! back to the scheduler while it waits instead of blocking it. Where no backend is installed (tests, tools) the same
//! futures run on the launcher's Tokio runtime, so callers do not care which they got.

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};

use tokio::sync::oneshot;

use crate::TaskError;

/// A boxed future with no output, which is what a backend runs.
pub type ComputeFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// What the application provides: where compute futures run, and how many cores that is.
#[derive(Clone, Copy)]
pub struct ComputeBackend {
    /// Starts `future` running; it must be polled to completion without the caller waiting on it.
    pub spawn: fn(ComputeFuture),
    /// The number of worker threads behind it.
    pub workers: fn() -> usize,
}

static BACKEND: OnceLock<ComputeBackend> = OnceLock::new();

/// Installs the executor compute work runs on. The first call wins.
pub fn install_compute_backend(backend: ComputeBackend) {
    let _ = BACKEND.set(backend);
}

/// How many pieces CPU work should be split into to keep every core busy.
pub fn compute_workers() -> usize {
    match BACKEND.get() {
        Some(backend) => (backend.workers)().max(1),
        None => std::thread::available_parallelism().map_or(1, |n| n.get()),
    }
}

/// A running compute task. Await it, or poll it from a frame with [`ComputeHandle::try_take`].
pub struct ComputeHandle<T> {
    result: oneshot::Receiver<T>,
}

impl<T> ComputeHandle<T> {
    /// The result if the task has finished, without waiting. A task that ended without producing one (it panicked)
    /// reports an error.
    pub fn try_take(&mut self) -> Option<Result<T, TaskError>> {
        match self.result.try_recv() {
            Ok(value) => Some(Ok(value)),
            Err(oneshot::error::TryRecvError::Empty) => None,
            Err(oneshot::error::TryRecvError::Closed) => Some(Err(ended_without_result())),
        }
    }
}

impl<T> Future for ComputeHandle<T> {
    type Output = Result<T, TaskError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.result)
            .poll(cx)
            .map(|result| result.map_err(|_| ended_without_result()))
    }
}

fn ended_without_result() -> TaskError {
    TaskError::JoinFailed("the compute task ended without producing a result (it panicked)".to_owned())
}

/// Runs `future` on the compute backend. Dropping the handle does not stop it.
pub fn spawn_compute<F>(future: F) -> ComputeHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let (sender, result) = oneshot::channel();
    let task: ComputeFuture = Box::pin(async move {
        let _ = sender.send(future.await);
    });
    match BACKEND.get() {
        Some(backend) => (backend.spawn)(task),
        None => crate::spawn_detached(task),
    }
    ComputeHandle { result }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_tasks_can_spawn_and_await_more_compute_tasks() {
        let total = crate::block_on(async {
            // A task that splits its work into pieces and awaits them, as the skin preview's rasteriser does.
            spawn_compute(async {
                let pieces: Vec<_> = (1..=4_u32).map(|n| spawn_compute(async move { n * 10 })).collect();
                let mut sum = 0;
                for piece in pieces {
                    sum += piece.await.expect("a piece finishes");
                }
                sum
            })
            .await
        })
        .expect("the runtime is available")
        .expect("the task finishes");
        assert_eq!(total, 100);
    }

    #[test]
    fn a_task_that_panics_reports_an_error_instead_of_a_result() {
        let outcome = crate::block_on(async { spawn_compute(async { panic!("expected in this test") }).await });
        let outcome: Result<u32, TaskError> = outcome.expect("the runtime is available");
        assert!(outcome.is_err());
    }

    #[test]
    fn there_is_always_at_least_one_worker() {
        assert!(compute_workers() >= 1);
    }
}
