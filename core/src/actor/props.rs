//! Props module provides actor creation properties.

use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::actor::Actor;
use crate::actor::actor_error::ActorError;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::{
  ActorContext, Context, InfoPart, MessagePart, ReceiverPart, SenderPart, SpawnerPart, StopperPart,
};
use crate::actor::context::typed_actor_context::TypedActorContext;
use crate::actor::message::{Message, MessageHandle, MessageOrEnvelope};
use crate::actor::pid::Pid;
use crate::actor::process::Process;
use crate::actor::spawner::SpawnError;

#[derive(Debug, Clone)]
pub struct Props {
  actor: Box<dyn Actor>,
}

impl Props {
  pub fn new(actor: Box<dyn Actor>) -> Self {
    Self { actor }
  }

  pub async fn spawn(&self, ctx: &dyn ActorContext) -> Result<Pid, SpawnError> {
    let actor_system = ctx.get_actor_system().await;
    let parent_pid = ctx.get_self_opt().await;
    let pid = actor_system.read().await.spawn_actor(self.clone(), parent_pid).await?;
    Ok(pid)
  }
}

pub async fn initialize<C: ActorContext>(props: Props, ctx: &C) -> Result<(), ActorError> {
  let actor_system = ctx.get_actor_system().await;
  let actor = props.actor;

  // Start the actor
  actor.started(ctx).await?;

  Ok(())
}
