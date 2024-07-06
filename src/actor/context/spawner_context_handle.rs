use std::sync::Arc;

use crate::actor::actor::actor_handle::ActorHandle;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::props::Props;
use crate::actor::actor::spawner::SpawnError;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{InfoPart, SpawnerContext, SpawnerPart};
use async_trait::async_trait;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct SpawnerContextHandle(Arc<Mutex<dyn SpawnerContext>>);

impl PartialEq for SpawnerContextHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for SpawnerContextHandle {}

impl std::hash::Hash for SpawnerContextHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const Mutex<dyn SpawnerContext>).hash(state);
  }
}

impl SpawnerContextHandle {
  pub fn new_arc(context: Arc<Mutex<dyn SpawnerContext>>) -> Self {
    SpawnerContextHandle(context)
  }

  pub fn new(c: impl SpawnerContext + 'static) -> Self {
    SpawnerContextHandle(Arc::new(Mutex::new(c)))
  }
}

#[async_trait]
impl InfoPart for SpawnerContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_parent().await
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_self_opt().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let mg = self.0.lock().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.0.lock().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl SpawnerPart for SpawnerContextHandle {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    let mut mg = self.0.lock().await;
    mg.spawn(props).await
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    let mut mg = self.0.lock().await;
    mg.spawn_prefix(props, prefix).await
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    let mut mg = self.0.lock().await;
    mg.spawn_named(props, id).await
  }
}

impl SpawnerContext for SpawnerContextHandle {}
