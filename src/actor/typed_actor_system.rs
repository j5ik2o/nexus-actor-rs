use crate::actor::actor::ExtendedPid;
use crate::actor::actor_system::{ActorSystem, Config};
use crate::actor::context::TypedRootContext;
use crate::actor::guardian::GuardiansValue;
use crate::actor::message::Message;
use crate::actor::process::process_registry::ProcessRegistry;
use crate::actor::process::ProcessHandle;
use crate::event_stream::EventStream;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TypedActorSystem<M: Message> {
  underlying: ActorSystem,
  phantom_data: PhantomData<M>,
}

impl<M: Message> TypedActorSystem<M> {
  pub fn new(underlying: ActorSystem) -> Self {
    Self {
      underlying,
      phantom_data: PhantomData,
    }
  }

  pub async fn new_local_pid(&self, id: &str) -> ExtendedPid {
    self.underlying.new_local_pid(id).await
  }

  pub async fn get_address(&self) -> String {
    self.underlying.get_address().await
  }

  pub async fn get_config(&self) -> Config {
    self.underlying.get_config().await
  }

  pub async fn get_root_context(&self) -> TypedRootContext<M> {
    TypedRootContext::new(self.underlying.get_root_context().await)
  }

  pub async fn get_dead_letter(&self) -> ProcessHandle {
    self.underlying.get_dead_letter().await
  }

  pub async fn get_process_registry(&self) -> ProcessRegistry {
    self.underlying.get_process_registry().await
  }

  pub async fn get_event_stream(&self) -> Arc<EventStream> {
    self.underlying.get_event_stream().await
  }

  pub async fn get_guardians(&self) -> GuardiansValue {
    self.underlying.get_guardians().await
  }
}

impl<M: Message> From<ActorSystem> for TypedActorSystem<M> {
  fn from(actor_system: ActorSystem) -> Self {
    Self::new(actor_system)
  }
}

impl<M: Message> From<TypedActorSystem<M>> for ActorSystem {
  fn from(typed_actor_system: TypedActorSystem<M>) -> Self {
    typed_actor_system.underlying
  }
}
