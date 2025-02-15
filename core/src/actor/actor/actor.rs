//! Actor trait implementation.

use async_trait::async_trait;
use std::fmt::Debug;

use crate::actor::actor_error::ActorError;
use crate::actor::context::Context;
use crate::actor::message::MessageOrEnvelope;
use crate::actor::supervisor::SupervisorStrategy;

#[async_trait]
pub trait Actor: Debug + Send + Sync + 'static {
  async fn receive(&mut self, ctx: &dyn Context, msg: MessageOrEnvelope);
  async fn started(&mut self, _ctx: &dyn Context) -> Result<(), ActorError> {
    Ok(())
  }
  async fn stopped(&mut self, _ctx: &dyn Context) -> Result<(), ActorError> {
    Ok(())
  }
  async fn restarting(&mut self, _ctx: &dyn Context) -> Result<(), ActorError> {
    Ok(())
  }
  async fn get_supervisor_strategy(&mut self) -> Option<Box<dyn SupervisorStrategy>> {
    None
  }
}
