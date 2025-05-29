use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::SpawnerContextHandle;
use crate::actor::core::actor_error::ActorError;
use crate::actor::core::pid::ExtendedPid;
use crate::actor::core::props::Props;
use futures::future::BoxFuture;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpawnError {
  #[error("Name already exists: {0}")]
  ErrNameExists(ExtendedPid),
  #[error("Actor error: {0}")]
  ErrPreStart(ActorError),
}

type SpawnerFn = Arc<
  dyn Fn(ActorSystem, String, Props, SpawnerContextHandle) -> BoxFuture<'static, Result<ExtendedPid, SpawnError>>
    + Send
    + Sync
    + 'static,
>;

#[derive(Clone)]
pub struct Spawner(SpawnerFn);

impl Debug for Spawner {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SpawnFunc")
  }
}

impl PartialEq for Spawner {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for Spawner {}

impl std::hash::Hash for Spawner {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref()
      as *const dyn Fn(
        ActorSystem,
        String,
        Props,
        SpawnerContextHandle,
      ) -> BoxFuture<'static, Result<ExtendedPid, SpawnError>>)
      .hash(state);
  }
}

impl Spawner {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ActorSystem, String, Props, SpawnerContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ExtendedPid, SpawnError>> + Send + 'static, {
    Self(Arc::new(move |s, name, p, sch| {
      Box::pin(f(s, name, p, sch)) as BoxFuture<'static, Result<ExtendedPid, SpawnError>>
    }))
  }

  pub async fn run(
    &self,
    actor_system: ActorSystem,
    name: &str,
    props: Props,
    parent_context: SpawnerContextHandle,
  ) -> Result<ExtendedPid, SpawnError> {
    (self.0)(actor_system, name.to_string(), props, parent_context).await
  }
}

static_assertions::assert_impl_all!(Spawner: Send, Sync);
