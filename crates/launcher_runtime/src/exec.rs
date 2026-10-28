//! The machinery under [`crate::spawn`] and friends: a result cell, an async executor, a blocking-thread pool, `block_on`
//! and a timer. Standard library only; the launcher has no async runtime of its own beyond this.

use std::collections::{BinaryHeap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------------------------------------------------------------
// Result cell: one value, delivered once, awaitable or pollable.
// ---------------------------------------------------------------------------------------------------------------------

struct CellState<T> {
    value: Option<Result<T, String>>,
    waker: Option<Waker>,
    done: bool,
}

struct Cell<T> {
    state: Mutex<CellState<T>>,
}

/// The sending half: completes the cell once. Dropped without completing, the receiver gets an error.
pub(crate) struct Completer<T> {
    cell: Arc<Cell<T>>,
    completed: bool,
}

/// The receiving half: a future, and [`Receiver::try_take`] for polling from a frame.
pub(crate) struct Receiver<T> {
    cell: Arc<Cell<T>>,
}

pub(crate) fn cell<T>() -> (Completer<T>, Receiver<T>) {
    let cell = Arc::new(Cell {
        state: Mutex::new(CellState { value: None, waker: None, done: false }),
    });
    (Completer { cell: cell.clone(), completed: false }, Receiver { cell })
}

impl<T> Completer<T> {
    pub(crate) fn complete(mut self, value: T) {
        self.completed = true;
        self.finish(Ok(value));
    }

    fn finish(&self, value: Result<T, String>) {
        let waker = {
            let mut state = lock(&self.cell.state);
            state.value = Some(value);
            state.done = true;
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T> Drop for Completer<T> {
    fn drop(&mut self) {
        if !self.completed {
            self.finish(Err("the task panicked or was dropped before it finished".to_owned()));
        }
    }
}

impl<T> Receiver<T> {
    /// The result if it has arrived, without waiting.
    pub(crate) fn try_take(&mut self) -> Option<Result<T, String>> {
        lock(&self.cell.state).value.take()
    }
}

impl<T> Future for Receiver<T> {
    type Output = Result<T, String>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = lock(&self.cell.state);
        match state.value.take() {
            Some(value) => Poll::Ready(value),
            None if state.done => Poll::Ready(Err("the result was already taken".to_owned())),
            None => {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// block_on
// ---------------------------------------------------------------------------------------------------------------------

struct ThreadWaker {
    thread: std::thread::Thread,
    woken: AtomicBool,
}

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.woken.store(true, Ordering::Release);
        self.thread.unpark();
    }
}

/// Drives `future` to completion on the calling thread.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let thread_waker = Arc::new(ThreadWaker { thread: std::thread::current(), woken: AtomicBool::new(false) });
    let waker = Waker::from(thread_waker.clone());
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        while !thread_waker.woken.swap(false, Ordering::Acquire) {
            std::thread::park();
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Async executor
// ---------------------------------------------------------------------------------------------------------------------

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

struct Task {
    future: Mutex<Option<BoxFuture>>,
    queued: AtomicBool,
}

struct Executor {
    queue: Mutex<VecDeque<Arc<Task>>>,
    ready: Condvar,
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if !self.queued.swap(true, Ordering::AcqRel) {
            let executor = executor();
            lock(&executor.queue).push_back(self.clone());
            executor.ready.notify_one();
        }
    }
}

fn executor() -> &'static Executor {
    static EXECUTOR: OnceLock<Executor> = OnceLock::new();
    static STARTED: OnceLock<()> = OnceLock::new();
    let executor = EXECUTOR.get_or_init(|| Executor { queue: Mutex::new(VecDeque::new()), ready: Condvar::new() });
    STARTED.get_or_init(|| {
        let workers = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(2, 8);
        for index in 0..workers {
            let spawned = std::thread::Builder::new()
                .name(format!("vertex-async-{index}"))
                .spawn(move || worker_loop(executor));
            if let Err(error) = spawned {
                tracing::error!(target: "vertexlauncher/runtime", "could not start an async worker: {error}");
            }
        }
    });
    executor
}

fn worker_loop(executor: &'static Executor) {
    loop {
        let task = {
            let mut queue = lock(&executor.queue);
            loop {
                if let Some(task) = queue.pop_front() {
                    break task;
                }
                queue = executor.ready.wait(queue).unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };
        // Cleared before polling, so a wake that arrives while the task runs queues it again.
        task.queued.store(false, Ordering::Release);
        let waker = Waker::from(task.clone());
        let mut cx = Context::from_waker(&waker);
        let mut slot = lock(&task.future);
        if let Some(future) = slot.as_mut() {
            let polled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| future.as_mut().poll(&mut cx)));
            if !matches!(polled, Ok(Poll::Pending)) {
                // Finished, or panicked: either way the future is dropped, which tells whoever awaits its result.
                *slot = None;
            }
        }
    }
}

/// Starts `future` running on the executor.
pub(crate) fn spawn_boxed(future: BoxFuture) {
    let task = Arc::new(Task { future: Mutex::new(Some(future)), queued: AtomicBool::new(true) });
    let executor = executor();
    lock(&executor.queue).push_back(task);
    executor.ready.notify_one();
}

// ---------------------------------------------------------------------------------------------------------------------
// Blocking pool
// ---------------------------------------------------------------------------------------------------------------------

type Job = Box<dyn FnOnce() + Send + 'static>;

const MAX_BLOCKING_THREADS: usize = 512;
const IDLE_THREAD_LIFETIME: Duration = Duration::from_secs(30);

struct PoolState {
    queue: VecDeque<Job>,
    idle: usize,
    threads: usize,
}

struct Pool {
    state: Mutex<PoolState>,
    wake: Condvar,
}

fn pool() -> &'static Pool {
    static POOL: OnceLock<Pool> = OnceLock::new();
    POOL.get_or_init(|| Pool {
        state: Mutex::new(PoolState { queue: VecDeque::new(), idle: 0, threads: 0 }),
        wake: Condvar::new(),
    })
}

/// Runs `job` on a pool thread that is allowed to block. Threads are made on demand and leave after sitting idle.
pub(crate) fn spawn_blocking_boxed(job: Job) {
    let pool = pool();
    let mut state = lock(&pool.state);
    state.queue.push_back(job);
    if state.idle >= state.queue.len() {
        pool.wake.notify_one();
        return;
    }
    if state.threads >= MAX_BLOCKING_THREADS {
        // Every thread is busy; the job waits its turn.
        return;
    }
    state.threads += 1;
    drop(state);
    let spawned = std::thread::Builder::new().name("vertex-blocking".to_owned()).spawn(blocking_loop);
    if let Err(error) = spawned {
        tracing::error!(target: "vertexlauncher/runtime", "could not start a blocking worker: {error}");
        let mut state = lock(&pool.state);
        state.threads -= 1;
        // With no thread at all to run the queue, dropping the jobs fails their handles instead of hanging them.
        if state.threads == 0 {
            state.queue.clear();
        }
    }
}

fn blocking_loop() {
    let pool = pool();
    let mut state = lock(&pool.state);
    loop {
        if let Some(job) = state.queue.pop_front() {
            drop(state);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            state = lock(&pool.state);
            continue;
        }
        state.idle += 1;
        let (next, timeout) = pool
            .wake
            .wait_timeout(state, IDLE_THREAD_LIFETIME)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state = next;
        state.idle -= 1;
        if timeout.timed_out() && state.queue.is_empty() {
            state.threads -= 1;
            return;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Timer
// ---------------------------------------------------------------------------------------------------------------------

struct Timer {
    entries: Mutex<BinaryHeap<TimerEntry>>,
    changed: Condvar,
}

struct TimerEntry {
    deadline: Instant,
    waker: Waker,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}
impl Eq for TimerEntry {}
impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Earliest deadline first out of a max-heap.
        other.deadline.cmp(&self.deadline)
    }
}

fn timer() -> &'static Timer {
    static TIMER: OnceLock<Timer> = OnceLock::new();
    static STARTED: OnceLock<()> = OnceLock::new();
    let timer = TIMER.get_or_init(|| Timer { entries: Mutex::new(BinaryHeap::new()), changed: Condvar::new() });
    STARTED.get_or_init(|| {
        let spawned = std::thread::Builder::new().name("vertex-timer".to_owned()).spawn(move || timer_loop(timer));
        if let Err(error) = spawned {
            tracing::error!(target: "vertexlauncher/runtime", "could not start the timer thread: {error}");
        }
    });
    timer
}

fn timer_loop(timer: &'static Timer) {
    let mut entries = lock(&timer.entries);
    loop {
        let now = Instant::now();
        while entries.peek().is_some_and(|entry| entry.deadline <= now) {
            if let Some(entry) = entries.pop() {
                entry.waker.wake();
            }
        }
        entries = match entries.peek().map(|entry| entry.deadline) {
            Some(deadline) => {
                timer
                    .changed
                    .wait_timeout(entries, deadline.saturating_duration_since(now))
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .0
            }
            None => timer.changed.wait(entries).unwrap_or_else(|poisoned| poisoned.into_inner()),
        };
    }
}

/// Wakes `waker` at `deadline`.
pub(crate) fn wake_at(deadline: Instant, waker: Waker) {
    let timer = timer();
    lock(&timer.entries).push(TimerEntry { deadline, waker });
    timer.changed.notify_one();
}
