//! A result channel for background work polled from the UI thread.
//!
//! Screens start a worker, keep the receiving end in their state, and drain it every frame.
//! This wraps the sender/receiver pair, creating it lazily, surviving `Clone` of the owning
//! state, and reporting a dead worker instead of every screen hand-rolling that handling.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

/// What [`WorkerChannel::drain`] found.
#[derive(Debug)]
pub struct Drained<T> {
    /// Messages received since the last drain, oldest first.
    pub items: Vec<T>,
    /// The channel broke: every sender was dropped without finishing, or the receiver lock
    /// was poisoned. The channel has already been reset; callers should clear their
    /// "in flight" flags and surface an error.
    pub disconnected: bool,
}

/// Lazily created sender/receiver pair for worker results of type `T`.
#[derive(Debug)]
pub struct WorkerChannel<T> {
    inner: Option<(Sender<T>, Arc<Mutex<Receiver<T>>>)>,
}

impl<T> Default for WorkerChannel<T> {
    fn default() -> Self {
        Self { inner: None }
    }
}

impl<T> Clone for WorkerChannel<T> {
    /// Clones share the same channel, like cloning the `Arc<Mutex<Receiver>>` by hand.
    fn clone(&self) -> Self {
        Self {
            inner: self
                .inner
                .as_ref()
                .map(|(tx, rx)| (tx.clone(), Arc::clone(rx))),
        }
    }
}

impl<T> WorkerChannel<T> {
    /// A sender for a new worker, creating the channel on first use.
    pub fn sender(&mut self) -> Sender<T> {
        let (tx, _) = self.inner.get_or_insert_with(|| {
            let (tx, rx) = mpsc::channel();
            (tx, Arc::new(Mutex::new(rx)))
        });
        tx.clone()
    }

    /// Whether the channel has been created (a worker may be running).
    pub fn is_open(&self) -> bool {
        self.inner.is_some()
    }

    /// Drops the channel; results from workers still holding a sender are discarded.
    pub fn reset(&mut self) {
        self.inner = None;
    }

    /// Collects everything the workers have sent so far without blocking.
    ///
    /// The channel keeps one sender of its own, so an idle channel is `Empty`, never
    /// `Disconnected`; `disconnected` therefore only signals a poisoned receiver (a worker
    /// thread panicked while holding it) or a channel torn down elsewhere.
    pub fn drain(&mut self) -> Drained<T> {
        self.drain_up_to(usize::MAX)
    }

    /// Like [`Self::drain`] but takes at most `max` messages, leaving the rest queued for the
    /// next call. Use to spread expensive per-result work across frames.
    pub fn drain_up_to(&mut self, max: usize) -> Drained<T> {
        let Some((_, receiver)) = self.inner.as_ref() else {
            return Drained {
                items: Vec::new(),
                disconnected: false,
            };
        };
        let mut items = Vec::new();
        let mut disconnected = false;
        match receiver.lock() {
            Ok(receiver) => {
                while items.len() < max {
                    match receiver.try_recv() {
                        Ok(item) => items.push(item),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }
            }
            Err(_) => disconnected = true,
        }
        if disconnected {
            self.inner = None;
        }
        Drained {
            items,
            disconnected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivers_in_order_from_several_workers_and_survives_clone() {
        let mut channel: WorkerChannel<u32> = WorkerChannel::default();
        assert!(!channel.is_open());
        assert!(channel.drain().items.is_empty());

        let first = channel.sender();
        let second = channel.sender();
        first.send(1).unwrap();
        second.send(2).unwrap();
        let mut copy = channel.clone();
        assert_eq!(copy.drain().items, [1, 2]);
        // The clone consumed them; the original sees a shared, now-empty channel.
        assert!(channel.drain().items.is_empty());
        first.send(3).unwrap();
        assert_eq!(channel.drain().items, [3]);
    }

    #[test]
    fn drain_up_to_leaves_the_rest_queued() {
        let mut channel: WorkerChannel<u32> = WorkerChannel::default();
        let tx = channel.sender();
        for n in 0..5 {
            tx.send(n).unwrap();
        }
        assert_eq!(channel.drain_up_to(2).items, [0, 1]);
        assert_eq!(channel.drain_up_to(2).items, [2, 3]);
        assert_eq!(channel.drain().items, [4]);
    }

    #[test]
    fn reset_discards_pending_results_and_starts_fresh() {
        let mut channel: WorkerChannel<&str> = WorkerChannel::default();
        let stale = channel.sender();
        channel.reset();
        let _ = stale.send("late");
        assert!(!channel.is_open());
        let fresh = channel.sender();
        fresh.send("new").unwrap();
        assert_eq!(channel.drain().items, ["new"]);
    }

    #[test]
    fn a_poisoned_receiver_reports_disconnect_and_resets() {
        let mut channel: WorkerChannel<u32> = WorkerChannel::default();
        let _ = channel.sender();
        let shared = Arc::clone(&channel.inner.as_ref().unwrap().1);
        let _ = std::thread::spawn(move || {
            let _guard = shared.lock().unwrap();
            panic!("worker died holding the lock");
        })
        .join();
        let drained = channel.drain();
        assert!(drained.disconnected);
        assert!(!channel.is_open());
    }
}
