//! Timers: [`sleep`] and [`timeout`].

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use crate::exec;

/// A future that finishes `true` after sleeping, or `false` if the backend's timer could not be used.
pub type BackendSleep = Pin<Box<dyn Future<Output = bool> + Send + 'static>>;

/// What the application can provide for timers, so waits ride on its own timer reactor instead of the fallback thread.
#[derive(Clone, Copy)]
pub struct AsyncBackend {
    pub sleep: fn(Duration) -> BackendSleep,
}

static BACKEND: OnceLock<AsyncBackend> = OnceLock::new();

/// Installs the timer backend. The first call wins.
pub fn install_async_backend(backend: AsyncBackend) {
    let _ = BACKEND.set(backend);
}

/// Finishes after the given time.
pub fn sleep(duration: Duration) -> Sleep {
    Sleep { deadline: Instant::now() + duration, engine: None, started: false }
}

pub struct Sleep {
    deadline: Instant,
    engine: Option<BackendSleep>,
    started: bool,
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let now = Instant::now();
        if now >= self.deadline {
            return Poll::Ready(());
        }
        if !self.started {
            self.started = true;
            if let Some(backend) = BACKEND.get() {
                let remaining = self.deadline - now;
                self.engine = Some((backend.sleep)(remaining));
            }
        }
        if let Some(engine) = self.engine.as_mut() {
            match engine.as_mut().poll(cx) {
                Poll::Ready(true) => return Poll::Ready(()),
                Poll::Ready(false) => self.engine = None,
                Poll::Pending => return Poll::Pending,
            }
        }
        exec::wake_at(self.deadline, cx.waker().clone());
        Poll::Pending
    }
}

/// The time ran out before the future finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed;

impl std::fmt::Display for Elapsed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "deadline has elapsed")
    }
}

impl std::error::Error for Elapsed {}

/// Runs `future`, giving up after `limit`.
pub fn timeout<F: Future>(limit: Duration, future: F) -> Timeout<F> {
    Timeout { future: Box::pin(future), sleep: sleep(limit) }
}

pub struct Timeout<F> {
    future: Pin<Box<F>>,
    sleep: Sleep,
}

impl<F: Future> Future for Timeout<F> {
    type Output = Result<F::Output, Elapsed>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Ready(value) = self.future.as_mut().poll(cx) {
            return Poll::Ready(Ok(value));
        }
        match Pin::new(&mut self.sleep).poll(cx) {
            Poll::Ready(()) => Poll::Ready(Err(Elapsed)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_waits_at_least_as_long_as_asked() {
        let started = Instant::now();
        crate::block_on(sleep(Duration::from_millis(60))).unwrap();
        assert!(started.elapsed() >= Duration::from_millis(60));
    }

    #[test]
    fn timeout_gives_up_on_a_slow_future_and_passes_a_fast_one() {
        let slow = crate::block_on(timeout(Duration::from_millis(30), sleep(Duration::from_secs(5)))).unwrap();
        assert_eq!(slow, Err(Elapsed));
        let fast = crate::block_on(timeout(Duration::from_secs(5), async { 7 })).unwrap();
        assert_eq!(fast, Ok(7));
    }
}
