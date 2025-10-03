use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use arc_swap::{ArcSwap, ArcSwapOption};
use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::{ActorContext, ContextBorrow};
use crate::actor::context::context_snapshot::ContextSnapshot;
use crate::actor::context::StdActorContextSnapshot;
use crate::actor::context::{
  BasePart, Context, CoreSenderPart, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext,
  ReceiverPart, SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
};
use crate::actor::core::ActorError;
use crate::actor::core::ActorHandle;
use crate::actor::core::Continuer;
use crate::actor::core::ExtendedPid;
use crate::actor::core::Props;
use crate::actor::core::SpawnError;
use crate::actor::message::MessageEnvelope;
use crate::actor::message::MessageHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::actor::message::ResponseHandle;
use crate::actor::process::actor_future::ActorFuture;
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
use nexus_actor_core_rs::context::CoreContextSnapshot;
use nexus_actor_core_rs::CorePid;

use super::ContextRegistry;

#[derive(Debug, Clone, Copy, Default)]
pub struct ContextLockMetricsSnapshot {
  pub read_lock_acquisitions: u64,
  pub write_lock_acquisitions: u64,
  pub snapshot_hits: u64,
  pub snapshot_misses: u64,
}

#[cfg(feature = "lock-metrics")]
struct ContextLockMetrics {
  read_lock_acquisitions: AtomicU64,
  write_lock_acquisitions: AtomicU64,
  snapshot_hits: AtomicU64,
  snapshot_misses: AtomicU64,
}

#[cfg(feature = "lock-metrics")]
impl ContextLockMetrics {
  const fn new() -> Self {
    Self {
      read_lock_acquisitions: AtomicU64::new(0),
      write_lock_acquisitions: AtomicU64::new(0),
      snapshot_hits: AtomicU64::new(0),
      snapshot_misses: AtomicU64::new(0),
    }
  }
}

#[cfg(feature = "lock-metrics")]
static CONTEXT_LOCK_METRICS: ContextLockMetrics = ContextLockMetrics::new();

#[cfg(feature = "lock-metrics")]
#[inline(always)]
fn record_snapshot_hit() {
  CONTEXT_LOCK_METRICS.snapshot_hits.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "lock-metrics"))]
#[inline(always)]
fn record_snapshot_hit() {}

#[cfg(feature = "lock-metrics")]
#[inline(always)]
fn record_snapshot_miss() {
  CONTEXT_LOCK_METRICS.snapshot_misses.fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "lock-metrics"))]
#[inline(always)]
fn record_snapshot_miss() {}

#[cfg(feature = "lock-metrics")]
#[inline(always)]
fn record_read_lock_acquisition() {
  CONTEXT_LOCK_METRICS
    .read_lock_acquisitions
    .fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "lock-metrics"))]
#[inline(always)]
fn record_read_lock_acquisition() {}

#[cfg(feature = "lock-metrics")]
#[inline(always)]
fn record_write_lock_acquisition() {
  CONTEXT_LOCK_METRICS
    .write_lock_acquisitions
    .fetch_add(1, Ordering::Relaxed);
}

#[cfg(not(feature = "lock-metrics"))]
#[inline(always)]
fn record_write_lock_acquisition() {}

#[cfg(feature = "lock-metrics")]
pub fn reset_context_lock_metrics() {
  CONTEXT_LOCK_METRICS.read_lock_acquisitions.store(0, Ordering::Relaxed);
  CONTEXT_LOCK_METRICS.write_lock_acquisitions.store(0, Ordering::Relaxed);
  CONTEXT_LOCK_METRICS.snapshot_hits.store(0, Ordering::Relaxed);
  CONTEXT_LOCK_METRICS.snapshot_misses.store(0, Ordering::Relaxed);
}

#[cfg(not(feature = "lock-metrics"))]
#[inline(always)]
pub fn reset_context_lock_metrics() {}

#[cfg(feature = "lock-metrics")]
pub fn context_lock_metrics_snapshot() -> ContextLockMetricsSnapshot {
  ContextLockMetricsSnapshot {
    read_lock_acquisitions: CONTEXT_LOCK_METRICS.read_lock_acquisitions.load(Ordering::Relaxed),
    write_lock_acquisitions: CONTEXT_LOCK_METRICS.write_lock_acquisitions.load(Ordering::Relaxed),
    snapshot_hits: CONTEXT_LOCK_METRICS.snapshot_hits.load(Ordering::Relaxed),
    snapshot_misses: CONTEXT_LOCK_METRICS.snapshot_misses.load(Ordering::Relaxed),
  }
}

#[cfg(not(feature = "lock-metrics"))]
#[inline(always)]
pub fn context_lock_metrics_snapshot() -> ContextLockMetricsSnapshot {
  ContextLockMetricsSnapshot::default()
}

#[derive(Debug, Default)]
pub struct ContextCell {
  actor_context: Arc<ArcSwapOption<ActorContext>>,
  snapshot_hits: AtomicU64,
  snapshot_misses: AtomicU64,
}

impl ContextCell {
  pub fn replace_actor_context(&self, ctx: ActorContext) {
    self.actor_context.store(Some(Arc::new(ctx)));
  }

  pub fn load_actor_context(&self) -> Option<Arc<ActorContext>> {
    match self.actor_context.load_full() {
      Some(ctx) => {
        self.snapshot_hits.fetch_add(1, Ordering::Relaxed);
        Some(ctx)
      }
      None => {
        self.snapshot_misses.fetch_add(1, Ordering::Relaxed);
        None
      }
    }
  }

  pub fn capture_from<C>(&self, ctx: &C)
  where
    C: Context + Any + Clone, {
    if let Some(actor_ctx) = (ctx as &dyn Any).downcast_ref::<ActorContext>() {
      self.replace_actor_context(actor_ctx.clone());
    }
  }

  pub fn snapshot_stats(&self) -> ContextCellStats {
    ContextCellStats {
      hits: self.snapshot_hits.load(Ordering::Relaxed),
      misses: self.snapshot_misses.load(Ordering::Relaxed),
    }
  }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ContextCellStats {
  pub hits: u64,
  pub misses: u64,
}

#[derive(Debug, Clone)]
pub struct ContextHandle {
  inner: Arc<ArcSwap<RwLock<Box<dyn Context>>>>,
  cell: Arc<ContextCell>,
}

#[derive(Debug, Clone)]
pub struct WeakContextHandle {
  inner: Weak<ArcSwap<RwLock<Box<dyn Context>>>>,
  cell: Weak<ContextCell>,
}

impl ContextHandle {
  fn from_swap(inner: Arc<ArcSwap<RwLock<Box<dyn Context>>>>, cell: Arc<ContextCell>) -> Self {
    ContextHandle { inner, cell }
  }

  pub fn new_arc(context: Arc<RwLock<Box<dyn Context>>>, cell: Arc<ContextCell>) -> Self {
    let swap = Arc::new(ArcSwap::from(context));
    ContextHandle::from_swap(swap, cell)
  }

  pub fn new<C>(c: C) -> Self
  where
    C: Context + Clone + Any + 'static, {
    let cell = Arc::new(ContextCell::default());
    cell.capture_from(&c);
    let context_arc: Arc<RwLock<Box<dyn Context>>> = Arc::new(RwLock::new(Box::new(c) as Box<dyn Context>));
    let swap = Arc::new(ArcSwap::from(context_arc));
    ContextHandle::from_swap(swap, cell)
  }

  pub fn downgrade(&self) -> WeakContextHandle {
    WeakContextHandle {
      inner: Arc::downgrade(&self.inner),
      cell: Arc::downgrade(&self.cell),
    }
  }

  pub fn actor_context_arc(&self) -> Option<Arc<ActorContext>> {
    let result = self.cell.load_actor_context();
    #[cfg(feature = "lock-metrics")]
    {
      if result.is_some() {
        record_snapshot_hit();
      } else {
        record_snapshot_miss();
      }
    }
    result
  }

  pub fn with_actor_context<R, F>(&self, f: F) -> Option<R>
  where
    F: FnOnce(&ActorContext) -> R, {
    self.actor_context_arc().map(|ctx| f(ctx.as_ref()))
  }

  pub fn with_actor_borrow<R, F>(&self, f: F) -> Option<R>
  where
    F: for<'a> FnOnce(ContextBorrow<'a>) -> R, {
    self.actor_context_arc().map(|ctx| {
      let borrow = ctx.borrow();
      f(borrow)
    })
  }

  fn context_arc(&self) -> Arc<RwLock<Box<dyn Context>>> {
    self.inner.load_full()
  }

  pub fn context_cell(&self) -> Arc<ContextCell> {
    self.cell.clone()
  }

  pub fn context_cell_stats(&self) -> ContextCellStats {
    self.cell.snapshot_stats()
  }

  pub fn snapshot(&self) -> ContextSnapshot {
    ContextSnapshot::from_context_handle(self)
  }

  pub async fn core_snapshot(&self) -> nexus_actor_core_rs::context::CoreActorContextSnapshot {
    StdActorContextSnapshot::capture(self).await.into_core()
  }

  pub fn snapshot_with_borrow(&self) -> ContextSnapshot {
    if let Some(snapshot) = self.with_actor_borrow(|borrow| {
      ContextSnapshot::from_borrow(&borrow)
        .with_sender_opt(self.try_get_sender_opt())
        .with_message_envelope_opt(self.try_get_message_envelope_opt())
        .with_message_handle_opt(self.try_get_message_handle_opt())
        .with_message_header_opt(self.try_get_message_header_handle())
        .with_context_handle_opt(Some(self.clone()))
    }) {
      snapshot
    } else {
      ContextSnapshot::from_context_handle(self).with_context_handle(self.clone())
    }
  }

  pub async fn snapshot_with_core(&self) -> ContextSnapshot {
    let core_snapshot = self.core_snapshot().await;
    let actor_system = self.get_actor_system().await;
    let system_id = actor_system.system_id();
    let self_pid = core_snapshot.self_pid_core();

    ContextRegistry::register(system_id, self_pid.id(), self);

    self
      .snapshot_with_borrow()
      .with_core_snapshot(CoreContextSnapshot::from(core_snapshot))
  }

  pub async fn try_into_actor_context(&self) -> Option<ActorContext> {
    if let Some(actor_ctx) = self.actor_context_arc() {
      return Some(actor_ctx.as_ref().clone());
    }
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.as_ref().as_any().downcast_ref::<ActorContext>().cloned()
  }

  pub(crate) async fn to_actor_context(&self) -> Option<ActorContext> {
    self.try_into_actor_context().await
  }
}

impl WeakContextHandle {
  pub fn upgrade(&self) -> Option<ContextHandle> {
    let inner = self.inner.upgrade()?;
    let cell = self.cell.upgrade()?;
    Some(ContextHandle::from_swap(inner, cell))
  }
}

impl ExtensionContext for ContextHandle {}

#[async_trait]
impl ExtensionPart for ContextHandle {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    let ctx = self.context_arc();
    let mut mg = ctx.write().await;
    mg.set(ext).await
  }
}

impl SenderContext for ContextHandle {}

#[async_trait]
impl InfoPart for ContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    if let Some(actor_ctx) = self.actor_context_arc() {
      let snapshot = actor_ctx.borrow();
      return snapshot.parent().cloned();
    }
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_parent().await
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    if let Some(actor_ctx) = self.actor_context_arc() {
      let snapshot = actor_ctx.borrow();
      return snapshot.self_pid().cloned();
    }
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_self_opt().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    if let Some(actor_ctx) = self.actor_context_arc() {
      let snapshot = actor_ctx.borrow();
      return snapshot.actor().cloned();
    }
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    if let Some(actor_ctx) = self.actor_context_arc() {
      let snapshot = actor_ctx.borrow();
      return snapshot.actor_system().clone();
    }
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl SenderPart for ContextHandle {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    if let Some(sender) = self.try_get_sender_opt() {
      return Some(sender);
    }
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_sender().await
  }

  async fn send(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.send(pid, message_handle).await
  }

  async fn request(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.request(pid, message_handle).await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message_handle: MessageHandle, sender: ExtendedPid) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.request_with_custom_sender(pid, message_handle, sender).await
  }

  async fn request_future(&self, pid: ExtendedPid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.request_future(pid, message_handle, timeout).await
  }
}

#[async_trait]
impl CoreSenderPart for ContextHandle {
  async fn get_sender_core(&self) -> Option<CorePid> {
    self.get_sender().await.map(|pid| pid.to_core())
  }

  async fn send_core(&mut self, pid: CorePid, message_handle: MessageHandle) {
    self.send(ExtendedPid::from(pid), message_handle).await
  }

  async fn request_core(&mut self, pid: CorePid, message_handle: MessageHandle) {
    self.request(ExtendedPid::from(pid), message_handle).await
  }

  async fn request_with_custom_sender_core(&mut self, pid: CorePid, message_handle: MessageHandle, sender: CorePid) {
    self
      .request_with_custom_sender(ExtendedPid::from(pid), message_handle, ExtendedPid::from(sender))
      .await
  }

  async fn request_future_core(&self, pid: CorePid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    self
      .request_future(ExtendedPid::from(pid), message_handle, timeout)
      .await
  }
}

#[async_trait]
impl MessagePart for ContextHandle {
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    if let Some(envelope) = self.try_get_message_envelope_opt() {
      return Some(envelope);
    }
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_message_envelope_opt().await
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    if let Some(handle) = self.try_get_message_handle_opt() {
      return Some(handle);
    }
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_message_handle_opt().await
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    if let Some(header) = self.try_get_message_header_handle() {
      return Some(header);
    }
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_message_header_handle().await
  }
}

impl ContextHandle {
  pub fn try_get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    self.actor_context_arc().and_then(|ctx| ctx.try_message_envelope())
  }

  pub fn try_get_message_handle_opt(&self) -> Option<MessageHandle> {
    self.actor_context_arc().and_then(|ctx| ctx.try_message_handle())
  }

  pub fn try_get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.actor_context_arc().and_then(|ctx| ctx.try_message_header())
  }

  pub fn try_get_sender_opt(&self) -> Option<ExtendedPid> {
    self.actor_context_arc().and_then(|ctx| ctx.try_sender())
  }

  pub fn try_get_sender_core(&self) -> Option<CorePid> {
    self.try_get_sender_opt().map(|pid| pid.to_core())
  }

  pub fn sender_snapshot(&self) -> Option<ExtendedPid> {
    self.try_get_sender_opt()
  }

  pub async fn watch_core(&mut self, pid: &CorePid) {
    BasePart::watch(self, &ExtendedPid::from(pid.clone())).await;
  }

  pub async fn unwatch_core(&mut self, pid: &CorePid) {
    BasePart::unwatch(self, &ExtendedPid::from(pid.clone())).await;
  }

  pub async fn forward_core(&self, pid: &CorePid) {
    BasePart::forward(self, &ExtendedPid::from(pid.clone())).await;
  }

  pub fn with_typed_borrow<M, R, F>(&self, f: F) -> Option<R>
  where
    M: crate::actor::message::Message,
    F: for<'a> FnOnce(crate::actor::context::TypedContextBorrow<'a, M>) -> R, {
    let actor_ctx = self.actor_context_arc()?;
    let borrow = actor_ctx.borrow();
    let view = crate::actor::context::TypedContextBorrow::new(actor_ctx.as_ref(), self.clone(), borrow);
    Some(f(view))
  }
}

impl ReceiverContext for ContextHandle {}

#[async_trait]
impl ReceiverPart for ContextHandle {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.receive(envelope).await
  }
}

impl SpawnerContext for ContextHandle {}

#[async_trait]
impl SpawnerPart for ContextHandle {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.spawn(props).await
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.spawn_prefix(props, prefix).await
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.spawn_named(props, id).await
  }
}

#[async_trait]
impl BasePart for ContextHandle {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn get_receive_timeout(&self) -> Duration {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_receive_timeout().await
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.get_children().await
  }

  async fn respond(&self, response: ResponseHandle) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.respond(response).await
  }

  async fn stash(&mut self) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.stash().await
  }

  async fn un_stash_all(&mut self) -> Result<(), ActorError> {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.un_stash_all().await
  }

  async fn watch(&mut self, pid: &ExtendedPid) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.watch(pid).await
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.unwatch(pid).await
  }

  async fn set_receive_timeout(&mut self, d: &Duration) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.set_receive_timeout(d).await
  }

  async fn cancel_receive_timeout(&mut self) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.cancel_receive_timeout().await
  }

  async fn forward(&self, pid: &ExtendedPid) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.forward(pid).await
  }

  async fn reenter_after(&self, f: ActorFuture, continuation: Continuer) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_read_lock_acquisition();
    let mg = ctx.read().await;
    mg.reenter_after(f, continuation).await
  }
}

#[async_trait]
impl StopperPart for ContextHandle {
  async fn stop(&mut self, pid: &ExtendedPid) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.stop(pid).await
  }

  async fn stop_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.stop_future_with_timeout(pid, timeout).await
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.poison(pid).await
  }

  async fn poison_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let ctx = self.context_arc();
    #[cfg(feature = "lock-metrics")]
    record_write_lock_acquisition();
    let mut mg = ctx.write().await;
    mg.poison_future_with_timeout(pid, timeout).await
  }
}

impl Context for ContextHandle {}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::core::{ActorError, Props};

  #[tokio::test]
  async fn with_actor_borrow_returns_actor_system_clone() {
    let system = ActorSystem::new().await.expect("actor system");
    let props = Props::from_async_actor_receiver(|_ctx| async { Ok::<(), ActorError>(()) }).await;
    let actor_context = ActorContext::new(system.clone(), props, None).await;
    let handle = ContextHandle::new(actor_context);

    let borrowed_system = handle
      .with_actor_borrow(|borrow| borrow.actor_system().clone())
      .expect("actor system snapshot");

    let original_id = system.get_id().await;
    let borrowed_id = borrowed_system.get_id().await;
    assert_eq!(original_id, borrowed_id);
  }

  #[tokio::test]
  async fn weak_context_handle_upgrade_fails_when_cell_dropped() {
    let system = ActorSystem::new().await.expect("actor system");
    let props = Props::from_async_actor_receiver(|_ctx| async { Ok::<(), ActorError>(()) }).await;
    let actor_context = ActorContext::new(system, props, None).await;

    let weak_handle = {
      let handle = ContextHandle::new(actor_context);
      handle.downgrade()
    };

    assert!(weak_handle.upgrade().is_none());
  }
}
