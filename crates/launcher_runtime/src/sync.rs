//! A counting semaphore for async tasks.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};

struct State {
    permits: usize,
    waiters: VecDeque<Waker>,
}

/// Limits how many tasks do something at once.
pub struct Semaphore {
    state: Mutex<State>,
}

/// A held permit; the slot is free again when this drops.
pub struct SemaphorePermit {
    semaphore: Arc<Semaphore>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Semaphore {
    pub fn new(permits: usize) -> Self {
        Self { state: Mutex::new(State { permits, waiters: VecDeque::new() }) }
    }

    /// Waits for a free slot. The permit keeps the semaphore alive, so it can move into a spawned task.
    pub fn acquire_owned(self: Arc<Self>) -> Acquire {
        Acquire { semaphore: self }
    }
}

pub struct Acquire {
    semaphore: Arc<Semaphore>,
}

impl Future for Acquire {
    type Output = Result<SemaphorePermit, std::convert::Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = lock(&self.semaphore.state);
        if state.permits > 0 {
            state.permits -= 1;
            return Poll::Ready(Ok(SemaphorePermit { semaphore: self.semaphore.clone() }));
        }
        state.waiters.push_back(cx.waker().clone());
        Poll::Pending
    }
}

impl Drop for SemaphorePermit {
    fn drop(&mut self) {
        let waiters = {
            let mut state = lock(&self.semaphore.state);
            state.permits += 1;
            // Everyone waiting looks again: a waiter that gave up would otherwise swallow the only wake.
            std::mem::take(&mut state.waiters)
        };
        for waiter in waiters {
            waiter.wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn never_more_tasks_inside_than_permits() {
        let semaphore = Arc::new(Semaphore::new(2));
        let inside = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let tasks: Vec<_> = (0..12)
            .map(|_| {
                let (semaphore, inside, peak) = (semaphore.clone(), inside.clone(), peak.clone());
                crate::spawn(async move {
                    let _permit = semaphore.acquire_owned().await.unwrap();
                    let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    crate::sleep(std::time::Duration::from_millis(10)).await;
                    inside.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for task in tasks {
            crate::block_on(task).unwrap().unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) <= 2);
    }
}
