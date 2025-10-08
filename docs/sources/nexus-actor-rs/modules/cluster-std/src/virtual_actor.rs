use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::FutureExt;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, Continuer, ErrorReason, ExtendedPid, Props, SpawnError};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle, ResponseHandle, SystemMessage};
use nexus_actor_std_rs::actor::process::actor_future::ActorFuture;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::cluster::Cluster;
use crate::identity::ClusterIdentity;
use crate::messages::ClusterInit;

/// Virtual Actor に渡されるコンテキスト。
#[derive(Debug, Clone)]
pub struct VirtualActorContext {
  identity: ClusterIdentity,
  actor_system: Arc<ActorSystem>,
  cluster: Cluster,
}

impl VirtualActorContext {
  pub fn new(identity: ClusterIdentity, actor_system: Arc<ActorSystem>, cluster: Cluster) -> Self {
    Self {
      identity,
      actor_system,
      cluster,
    }
  }

  pub fn identity(&self) -> &ClusterIdentity {
    &self.identity
  }

  pub fn actor_system(&self) -> Arc<ActorSystem> {
    self.actor_system.clone()
  }

  pub fn cluster(&self) -> Cluster {
    self.cluster.clone()
  }
}

#[async_trait]
pub trait VirtualActor: Send + Sync + std::fmt::Debug + 'static {
  async fn activate(&mut self, ctx: &VirtualActorContext) -> Result<(), ActorError>;

  async fn deactivate(&mut self, _ctx: &VirtualActorContext) -> Result<(), ActorError> {
    Ok(())
  }

  async fn handle(&mut self, message: MessageHandle, runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError>;
}

pub(crate) type VirtualActorFactory<V> = Arc<dyn Fn(ClusterIdentity) -> BoxFuture<'static, V> + Send + Sync + 'static>;

#[derive(Clone)]
pub(crate) struct VirtualActorRunner<V> {
  identity: ClusterIdentity,
  factory: VirtualActorFactory<V>,
}

impl<V> fmt::Debug for VirtualActorRunner<V> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("VirtualActorRunner")
      .field("identity", &self.identity)
      .finish()
  }
}

impl<V> VirtualActorRunner<V>
where
  V: VirtualActor,
{
  pub fn new(identity: ClusterIdentity, factory: VirtualActorFactory<V>) -> Self {
    Self { identity, factory }
  }
}

#[derive(Debug)]
struct VirtualActorState<V>
where
  V: VirtualActor, {
  runner: VirtualActorRunner<V>,
  context: Option<VirtualActorContext>,
  actor: Option<V>,
}

impl<V> VirtualActorState<V>
where
  V: VirtualActor,
{
  fn new(runner: VirtualActorRunner<V>) -> Self {
    Self {
      runner,
      context: None,
      actor: None,
    }
  }

  async fn ensure_actor(&mut self, ctx: &VirtualActorContext) -> Result<&mut V, ActorError> {
    if self.actor.is_none() {
      let factory = self.runner.factory.clone();
      let identity = ctx.identity().clone();
      let mut actor = factory(identity).await;
      actor.activate(ctx).await?;
      self.actor = Some(actor);
    }
    self
      .actor
      .as_mut()
      .ok_or_else(|| ActorError::of_initialization_error(ErrorReason::from("virtual actor activation failed")))
  }
}

#[async_trait]
impl<V> Actor for VirtualActorState<V>
where
  V: VirtualActor,
{
  async fn receive(&mut self, context: ContextHandle) -> Result<(), ActorError> {
    let Some(message) = context.get_message_handle_opt().await else {
      return Ok(());
    };

    if let Some(init) = message.as_typed::<ClusterInit>() {
      if init.context().identity() != &self.runner.identity {
        return Err(ActorError::of_initialization_error(ErrorReason::from(
          "cluster identity mismatch on ClusterInit",
        )));
      }
      let ctx = init.context().clone();
      self.context = Some(ctx.clone());
      self.ensure_actor(&ctx).await?;
      return Ok(());
    }

    if let Some(system) = message.as_typed::<SystemMessage>() {
      if matches!(system, SystemMessage::Stop | SystemMessage::Terminate(_)) {
        if let (Some(actor), Some(ctx)) = (self.actor.as_mut(), self.context.as_ref()) {
          actor.deactivate(ctx).await?;
        }
        self.actor = None;
        return Ok(());
      }
    }

    let ctx = self
      .context
      .clone()
      .ok_or_else(|| ActorError::of_receive_error(ErrorReason::from("virtual actor not initialized")))?;

    let actor = self.ensure_actor(&ctx).await?;
    let factory = VirtualActorRuntime::new(&ctx, &context);
    actor.handle(message, runtime).await
  }

  async fn post_stop(&mut self, _context_handle: ContextHandle) -> Result<(), ActorError> {
    if let (Some(actor), Some(ctx)) = (self.actor.take(), self.context.clone()) {
      let mut actor = actor;
      actor.deactivate(&ctx).await?;
      self.context = None;
    }
    Ok(())
  }
}

pub(crate) fn make_virtual_actor_props<V, F, Fut>(identity: ClusterIdentity, factory: F) -> BoxFuture<'static, Props>
where
  V: VirtualActor,
  F: Fn(ClusterIdentity) -> Fut + Send + Sync + 'static,
  Fut: std::future::Future<Output = V> + Send + 'static, {
  let factory: VirtualActorFactory<V> = Arc::new(move |identity: ClusterIdentity| factory(identity).boxed());
  async move {
    Props::from_async_actor_producer(move |_| {
      let runner = VirtualActorRunner::new(identity.clone(), factory.clone());
      async move { VirtualActorState::new(runner) }
    })
    .await
  }
  .boxed()
}

#[derive(Debug, Clone)]
pub struct VirtualActorRuntime<'a> {
  context: &'a VirtualActorContext,
  actor_ctx: &'a ContextHandle,
}

impl<'a> VirtualActorRuntime<'a> {
  pub fn new(context: &'a VirtualActorContext, actor_ctx: &'a ContextHandle) -> Self {
    Self { context, actor_ctx }
  }

  pub fn context(&self) -> &VirtualActorContext {
    self.context
  }

  pub fn actor_context(&self) -> &ContextHandle {
    self.actor_ctx
  }

  pub async fn respond<M>(&self, response: M)
  where
    M: Message + Send + Sync + 'static, {
    self.actor_ctx.respond(ResponseHandle::new(response)).await;
  }

  pub async fn reenter_after(&self, future: ActorFuture, continuation: Continuer) {
    self.actor_ctx.reenter_after(future, continuation).await;
  }

  pub async fn tell<M>(&self, pid: ExtendedPid, message: M)
  where
    M: Message + Send + Sync + 'static, {
    let mut ctx = self.actor_ctx.clone();
    ctx.send(pid, MessageHandle::new(message)).await;
  }

  pub async fn request<M>(&self, pid: ExtendedPid, message: M)
  where
    M: Message + Send + Sync + 'static, {
    let mut ctx = self.actor_ctx.clone();
    ctx.request(pid, MessageHandle::new(message)).await;
  }

  pub async fn request_future<M>(&self, pid: ExtendedPid, message: M, timeout: Duration) -> ActorFuture
  where
    M: Message + Send + Sync + 'static, {
    self
      .actor_ctx
      .request_future(pid, MessageHandle::new(message), timeout)
      .await
  }

  pub async fn forward(&self, pid: &ExtendedPid) {
    self.actor_ctx.forward(pid).await;
  }

  pub async fn spawn_child(&self, props: Props) -> ExtendedPid {
    let mut ctx = self.actor_ctx.clone();
    ctx.spawn(props).await
  }

  pub async fn spawn_child_named(&self, props: Props, name: &str) -> Result<ExtendedPid, SpawnError> {
    let mut ctx = self.actor_ctx.clone();
    ctx.spawn_named(props, name).await
  }

  pub async fn watch(&self, pid: &ExtendedPid) {
    let mut ctx = self.actor_ctx.clone();
    ctx.watch(pid).await;
  }

  pub async fn unwatch(&self, pid: &ExtendedPid) {
    let mut ctx = self.actor_ctx.clone();
    ctx.unwatch(pid).await;
  }
}
