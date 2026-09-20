//! Helpers for keeping memory use bounded around bursts of heavy background work.

use std::sync::{Condvar, Mutex};

/// A counting semaphore for blocking threads, used to cap how many image decodes (which briefly
/// hold several full-size copies of the image) run at once.
pub struct BlockingGate {
    free: Mutex<usize>,
    available: Condvar,
}

/// Held while a slot of a [`BlockingGate`] is in use; releases it on drop.
pub struct BlockingGatePermit<'a> {
    gate: &'a BlockingGate,
}

impl BlockingGate {
    pub const fn new(slots: usize) -> Self {
        Self {
            free: Mutex::new(slots),
            available: Condvar::new(),
        }
    }

    /// Blocks until a slot is free.
    pub fn acquire(&self) -> BlockingGatePermit<'_> {
        let mut free = self.free.lock().unwrap_or_else(|err| err.into_inner());
        while *free == 0 {
            free = self
                .available
                .wait(free)
                .unwrap_or_else(|err| err.into_inner());
        }
        *free -= 1;
        BlockingGatePermit { gate: self }
    }
}

impl Drop for BlockingGatePermit<'_> {
    fn drop(&mut self) {
        let mut free = self.gate.free.lock().unwrap_or_else(|err| err.into_inner());
        *free += 1;
        self.gate.available.notify_one();
    }
}

/// Asks the allocator to hand freed memory back to the operating system.
///
/// Freed image buffers otherwise stay in the process's heap arenas, so resident memory keeps
/// looking high after a screen releases its images. Only glibc needs this; elsewhere it is a no-op.
pub fn release_memory_to_os() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: `malloc_trim` has no preconditions; it only returns unused heap pages to the OS.
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn gate_never_lets_more_than_its_slots_run_at_once() {
        static GATE: BlockingGate = BlockingGate::new(2);
        static RUNNING: AtomicUsize = AtomicUsize::new(0);
        static PEAK: AtomicUsize = AtomicUsize::new(0);
        let threads: Vec<_> = (0..8)
            .map(|_| {
                std::thread::spawn(|| {
                    let _permit = GATE.acquire();
                    let now = RUNNING.fetch_add(1, Ordering::SeqCst) + 1;
                    PEAK.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    RUNNING.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert!(PEAK.load(Ordering::SeqCst) <= 2);
    }
}
