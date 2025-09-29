use std::fmt::Debug;

use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[cfg(feature = "tokio-console")]
use dashmap::DashMap;
#[cfg(feature = "tokio-console")]
use once_cell::sync::Lazy;
#[cfg(feature = "tokio-console")]
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub struct InstrumentedRwLock<T> {
  target: &'static str,
  inner: RwLock<T>,
}

impl<T> InstrumentedRwLock<T> {
  pub fn new(value: T, target: &'static str) -> Self {
    Self {
      target,
      inner: RwLock::new(value),
    }
  }

  pub async fn read(&self, operation: &'static str) -> RwLockReadGuard<'_, T> {
    let timer = LockWaitRecorder::new("read", self.target, operation);
    let guard = self.inner.read().await;
    timer.record();
    guard
  }

  pub async fn write(&self, operation: &'static str) -> RwLockWriteGuard<'_, T> {
    let timer = LockWaitRecorder::new("write", self.target, operation);
    let guard = self.inner.write().await;
    timer.record();
    guard
  }
}

#[cfg(feature = "tokio-console")]
#[derive(Debug, Default)]
struct LockStats {
  total_wait_ns: AtomicU64,
  max_wait_ns: AtomicU64,
  samples: AtomicU64,
}

#[cfg(feature = "tokio-console")]
impl LockStats {
  fn record(&self, wait_ns: u64) {
    self.total_wait_ns.fetch_add(wait_ns, Ordering::Relaxed);
    self.samples.fetch_add(1, Ordering::Relaxed);
    self
      .max_wait_ns
      .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        if wait_ns > current {
          Some(wait_ns)
        } else {
          None
        }
      })
      .ok();
  }

  fn snapshot(&self) -> (u64, u64, u64) {
    (
      self.total_wait_ns.load(Ordering::Relaxed),
      self.max_wait_ns.load(Ordering::Relaxed),
      self.samples.load(Ordering::Relaxed),
    )
  }
}

#[cfg(feature = "tokio-console")]
static LOCK_WAIT_STATS: Lazy<DashMap<(&'static str, &'static str, &'static str), LockStats>> = Lazy::new(DashMap::new);

#[cfg(feature = "tokio-console")]
fn record_wait(component: &'static str, operation: &'static str, mode: &'static str, wait_ns: u64) {
  let stats = LOCK_WAIT_STATS.entry((component, operation, mode)).or_default();
  stats.record(wait_ns);
}

#[cfg(feature = "tokio-console")]
#[derive(Debug, Clone)]
pub struct LockStatRecord {
  pub component: &'static str,
  pub operation: &'static str,
  pub mode: &'static str,
  pub samples: u64,
  pub total_wait_ns: u64,
  pub max_wait_ns: u64,
}

#[cfg(feature = "tokio-console")]
impl LockStatRecord {
  pub fn avg_wait_ns(&self) -> f64 {
    if self.samples == 0 {
      0.0
    } else {
      self.total_wait_ns as f64 / self.samples as f64
    }
  }
}

#[cfg(not(feature = "tokio-console"))]
#[derive(Debug, Clone)]
pub struct LockStatRecord;

#[cfg(feature = "tokio-console")]
pub fn lock_wait_snapshot() -> Vec<LockStatRecord> {
  LOCK_WAIT_STATS
    .iter()
    .map(|entry| {
      let ((component, operation, mode), stats) = entry.pair();
      let (total_wait_ns, max_wait_ns, samples) = stats.snapshot();
      LockStatRecord {
        component,
        operation,
        mode,
        samples,
        total_wait_ns,
        max_wait_ns,
      }
    })
    .collect()
}

#[cfg(not(feature = "tokio-console"))]
#[inline]
pub fn lock_wait_snapshot() -> Vec<LockStatRecord> {
  Vec::new()
}

#[derive(Debug)]
struct LockWaitRecorder {
  #[cfg(feature = "tokio-console")]
  start: tokio::time::Instant,
  #[cfg(feature = "tokio-console")]
  mode: &'static str,
  #[cfg(feature = "tokio-console")]
  target: &'static str,
  #[cfg(feature = "tokio-console")]
  operation: &'static str,
}

impl LockWaitRecorder {
  fn new(mode: &'static str, target: &'static str, operation: &'static str) -> Self {
    #[cfg(feature = "tokio-console")]
    {
      return Self {
        start: tokio::time::Instant::now(),
        mode,
        target,
        operation,
      };
    }

    #[cfg(not(feature = "tokio-console"))]
    {
      let _ = (mode, target, operation);
      Self {}
    }
  }

  fn record(&self) {
    #[cfg(feature = "tokio-console")]
    {
      let waited = self.start.elapsed();
      let wait_ns = waited.as_nanos() as u64;
      let wait_ms = waited.as_secs_f64() * 1000.0;
      record_wait(self.target, self.operation, self.mode, wait_ns);
      if wait_ms >= 1.0 {
        tracing::info!(
          target = "nexus_actor::lock_wait",
          component = self.target,
          operation = self.operation,
          mode = self.mode,
          wait_ms
        );
      } else {
        tracing::debug!(
          target = "nexus_actor::lock_wait",
          component = self.target,
          operation = self.operation,
          mode = self.mode,
          wait_ms
        );
      }
    }
  }
}
