use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::{
  BasePart, Context, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart,
  SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
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
  inner: Arc<RwLock<dyn Context>>,
  cell: Arc<ContextCell>,
}

#[derive(Debug, Clone)]
pub struct WeakContextHandle {
  inner: Weak<RwLock<dyn Context>>,
  cell: Weak<ContextCell>,
}

impl ContextHandle {
  pub fn new_arc(context: Arc<RwLock<dyn Context>>, cell: Arc<ContextCell>) -> Self {
    ContextHandle { inner: context, cell }
  }

  pub fn new<C>(c: C) -> Self
  where
    C: Context + Clone + Any + 'static, {
    let cell = Arc::new(ContextCell::default());
    cell.capture_from(&c);
    ContextHandle {
      inner: Arc::new(RwLock::new(c)),
      cell,
    }
  }

  pub fn downgrade(&self) -> WeakContextHandle {
    WeakContextHandle {
      inner: Arc::downgrade(&self.inner),
      cell: Arc::downgrade(&self.cell),
    }
  }

  pub fn actor_context_arc(&self) -> Option<Arc<ActorContext>> {
    self.cell.load_actor_context()
  }

  pub fn context_cell(&self) -> Arc<ContextCell> {
    self.cell.clone()
  }

  pub fn context_cell_stats(&self) -> ContextCellStats {
    self.cell.snapshot_stats()
  }

  pub async fn try_into_actor_context(&self) -> Option<ActorContext> {
    if let Some(actor_ctx) = self.actor_context_arc() {
      return Some(actor_ctx.as_ref().clone());
    }
    let mg = self.inner.read().await;
    mg.as_any().downcast_ref::<ActorContext>().cloned()
  }

  pub(crate) async fn to_actor_context(&self) -> Option<ActorContext> {
    self.try_into_actor_context().await
  }
}

impl WeakContextHandle {
  pub fn upgrade(&self) -> Option<ContextHandle> {
    let inner = self.inner.upgrade()?;
    let cell = self.cell.upgrade().unwrap_or_else(|| Arc::new(ContextCell::default()));
    Some(ContextHandle::new_arc(inner, cell))
  }
}

impl ExtensionContext for ContextHandle {}

#[async_trait]
impl ExtensionPart for ContextHandle {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let mut mg = self.inner.write().await;
    mg.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    let mut mg = self.inner.write().await;
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
    let mg = self.inner.read().await;
    mg.get_parent().await
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    if let Some(actor_ctx) = self.actor_context_arc() {
      let snapshot = actor_ctx.borrow();
      return snapshot.self_pid().cloned();
    }
    let mg = self.inner.read().await;
    mg.get_self_opt().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.inner.write().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    if let Some(actor_ctx) = self.actor_context_arc() {
      let snapshot = actor_ctx.borrow();
      return snapshot.actor().cloned();
    }
    let mg = self.inner.read().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    if let Some(actor_ctx) = self.actor_context_arc() {
      let snapshot = actor_ctx.borrow();
      return snapshot.actor_system().clone();
    }
    let mg = self.inner.read().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl SenderPart for ContextHandle {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    let mg = self.inner.read().await;
    mg.get_sender().await
  }

  async fn send(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    let mut mg = self.inner.write().await;
    mg.send(pid, message_handle).await
  }

  async fn request(&mut self, pid: ExtendedPid, message_handle: MessageHandle) {
    let mut mg = self.inner.write().await;
    mg.request(pid, message_handle).await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message_handle: MessageHandle, sender: ExtendedPid) {
    let mut mg = self.inner.write().await;
    mg.request_with_custom_sender(pid, message_handle, sender).await
  }

  async fn request_future(&self, pid: ExtendedPid, message_handle: MessageHandle, timeout: Duration) -> ActorFuture {
    let mg = self.inner.read().await;
    mg.request_future(pid, message_handle, timeout).await
  }
}

#[async_trait]
impl MessagePart for ContextHandle {
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    let mg = self.inner.read().await;
    mg.get_message_envelope_opt().await
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    let mg = self.inner.read().await;
    mg.get_message_handle_opt().await
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    let mg = self.inner.read().await;
    mg.get_message_header_handle().await
  }
}

impl ReceiverContext for ContextHandle {}

#[async_trait]
impl ReceiverPart for ContextHandle {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let mut mg = self.inner.write().await;
    mg.receive(envelope).await
  }
}

impl SpawnerContext for ContextHandle {}

#[async_trait]
impl SpawnerPart for ContextHandle {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    let mut mg = self.inner.write().await;
    mg.spawn(props).await
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    let mut mg = self.inner.write().await;
    mg.spawn_prefix(props, prefix).await
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    let mut mg = self.inner.write().await;
    mg.spawn_named(props, id).await
  }
}

#[async_trait]
impl BasePart for ContextHandle {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn get_receive_timeout(&self) -> Duration {
    let mg = self.inner.read().await;
    mg.get_receive_timeout().await
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    let mg = self.inner.read().await;
    mg.get_children().await
  }

  async fn respond(&self, response: ResponseHandle) {
    let mg = self.inner.read().await;
    mg.respond(response).await
  }

  async fn stash(&mut self) {
    let mut mg = self.inner.write().await;
    mg.stash().await
  }

  async fn un_stash_all(&mut self) -> Result<(), ActorError> {
    let mut mg = self.inner.write().await;
    mg.un_stash_all().await
  }

  async fn watch(&mut self, pid: &ExtendedPid) {
    let mut mg = self.inner.write().await;
    mg.watch(pid).await
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    let mut mg = self.inner.write().await;
    mg.unwatch(pid).await
  }

  async fn set_receive_timeout(&mut self, d: &Duration) {
    let mut mg = self.inner.write().await;
    mg.set_receive_timeout(d).await
  }

  async fn cancel_receive_timeout(&mut self) {
    let mut mg = self.inner.write().await;
    mg.cancel_receive_timeout().await
  }

  async fn forward(&self, pid: &ExtendedPid) {
    let mg = self.inner.read().await;
    mg.forward(pid).await
  }

  async fn reenter_after(&self, f: ActorFuture, continuation: Continuer) {
    let mg = self.inner.read().await;
    mg.reenter_after(f, continuation).await
  }
}

#[async_trait]
impl StopperPart for ContextHandle {
  async fn stop(&mut self, pid: &ExtendedPid) {
    let mut mg = self.inner.write().await;
    mg.stop(pid).await
  }

  async fn stop_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let mut mg = self.inner.write().await;
    mg.stop_future_with_timeout(pid, timeout).await
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    let mut mg = self.inner.write().await;
    mg.poison(pid).await
  }

  async fn poison_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    let mut mg = self.inner.write().await;
    mg.poison_future_with_timeout(pid, timeout).await
  }
}

impl Context for ContextHandle {}
