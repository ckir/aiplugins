//! Time seam: `SystemClock` for production, `TestClock` for deterministic tests.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub trait Clock: Send + Sync + 'static {
    fn now_ms(&self) -> i64;
    fn sleep(&self, d: Duration) -> impl Future<Output = ()> + Send;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    fn sleep(&self, d: Duration) -> impl Future<Output = ()> + Send {
        tokio::time::sleep(d)
    }
}

struct Inner {
    now: AtomicI64,
    notify: tokio::sync::Notify,
    deadlines: Mutex<BTreeMap<u64, i64>>,
    next_id: AtomicU64,
}

#[derive(Clone)]
pub struct TestClock {
    inner: Arc<Inner>,
}

struct DeadlineGuard {
    inner: Arc<Inner>,
    id: u64,
}

impl Drop for DeadlineGuard {
    fn drop(&mut self) {
        self.inner.deadlines.lock().unwrap().remove(&self.id);
    }
}

impl TestClock {
    pub fn new(start_ms: i64) -> Self {
        Self {
            inner: Arc::new(Inner {
                now: AtomicI64::new(start_ms),
                notify: tokio::sync::Notify::new(),
                deadlines: Mutex::new(BTreeMap::new()),
                next_id: AtomicU64::new(0),
            }),
        }
    }

    pub fn advance(&self, ms: i64) {
        self.inner.now.fetch_add(ms, Ordering::SeqCst);
        self.inner.notify.notify_waiters();
    }

    pub fn set(&self, ms: i64) {
        self.inner.now.store(ms, Ordering::SeqCst);
        self.inner.notify.notify_waiters();
    }

    pub fn pending_sleepers(&self) -> usize {
        self.inner.deadlines.lock().unwrap().len()
    }

    pub fn next_deadline(&self) -> Option<i64> {
        self.inner.deadlines.lock().unwrap().values().min().copied()
    }

    /// Drive virtual time: once every runnable task has parked on a sleep
    /// (the sleeper set is stable for 32 consecutive yields), jump to the
    /// earliest deadline. Never returns; use inside `tokio::select!` on a
    /// current-thread runtime.
    pub async fn run_until_idle(&self) {
        const STABLE: u32 = 32;
        const MAX_YIELDS: u64 = 100_000;
        let mut total: u64 = 0;
        loop {
            let mut last = (self.pending_sleepers(), self.next_deadline());
            let mut stable = 0u32;
            loop {
                tokio::task::yield_now().await;
                total += 1;
                let cur = (self.pending_sleepers(), self.next_deadline());
                if cur == last {
                    stable += 1;
                } else {
                    stable = 0;
                    last = cur;
                }
                if stable >= STABLE && cur.0 >= 1 {
                    break;
                }
                if total > MAX_YIELDS {
                    panic!("TestClock::run_until_idle: no quiescence after {MAX_YIELDS} yields (sleepers, next_deadline) = {cur:?}");
                }
            }
            if let Some(d) = self.next_deadline() {
                self.set(d);
            }
        }
    }
}

impl Clock for TestClock {
    fn now_ms(&self) -> i64 {
        self.inner.now.load(Ordering::SeqCst)
    }

    fn sleep(&self, d: Duration) -> impl Future<Output = ()> + Send {
        // The deadline is fixed when `sleep` is CALLED (not on first poll), so a
        // future handed to `spawn` and then outrun by `advance` still fires.
        let inner = self.inner.clone();
        let deadline = inner.now.load(Ordering::SeqCst) + d.as_millis() as i64;
        let id = inner.next_id.fetch_add(1, Ordering::SeqCst);
        inner.deadlines.lock().unwrap().insert(id, deadline);
        let guard = DeadlineGuard {
            inner: inner.clone(),
            id,
        };
        async move {
            let _guard = guard;
            loop {
                if inner.now.load(Ordering::SeqCst) >= deadline {
                    return;
                }
                let n = inner.notify.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                if inner.now.load(Ordering::SeqCst) >= deadline {
                    return;
                }
                n.await;
            }
        }
    }
}
