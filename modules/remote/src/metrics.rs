use nexus_actor_std_rs::actor::context::ContextHandle;
use nexus_actor_std_rs::actor::core::ExtendedPid;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
struct SenderSnapshotCounters {
  hits: AtomicU64,
  misses: AtomicU64,
}

static SENDER_SNAPSHOT_COUNTERS: Lazy<SenderSnapshotCounters> = Lazy::new(SenderSnapshotCounters::default);

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SenderSnapshotReport {
  pub hits: u64,
  pub misses: u64,
}

/// Try to retrieve the sender PID synchronously, and fall back to the core snapshot when necessary.
pub async fn record_sender_snapshot(context: &ContextHandle) -> Option<ExtendedPid> {
  if let Some(pid) = context.try_get_sender_opt() {
    SENDER_SNAPSHOT_COUNTERS.hits.fetch_add(1, Ordering::Relaxed);
    return Some(pid);
  }

  SENDER_SNAPSHOT_COUNTERS.misses.fetch_add(1, Ordering::Relaxed);
  let core_snapshot = context.core_snapshot().await;
  core_snapshot.sender_pid_core().map(ExtendedPid::from_core)
}

#[allow(dead_code)]
pub fn sender_snapshot_report() -> SenderSnapshotReport {
  SenderSnapshotReport {
    hits: SENDER_SNAPSHOT_COUNTERS.hits.load(Ordering::Relaxed),
    misses: SENDER_SNAPSHOT_COUNTERS.misses.load(Ordering::Relaxed),
  }
}

#[allow(dead_code)]
pub fn reset_sender_snapshot_metrics() {
  SENDER_SNAPSHOT_COUNTERS.hits.store(0, Ordering::Relaxed);
  SENDER_SNAPSHOT_COUNTERS.misses.store(0, Ordering::Relaxed);
}
