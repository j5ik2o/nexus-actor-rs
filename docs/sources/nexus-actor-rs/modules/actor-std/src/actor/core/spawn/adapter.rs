use std::any::Any;
use std::sync::Arc;

use nexus_actor_core_rs::context::core::{CoreActorSpawnError, CoreSpawnAdapter, CoreSpawnFuture, CoreSpawnInvocation};
use nexus_actor_core_rs::context::CoreSpawnMiddlewareChain;
use nexus_actor_core_rs::CorePid;

use crate::actor::actor_system::with_actor_system;
use crate::actor::context::{ContextRegistry, SpawnerContextHandle};
use crate::actor::core::props::Props;
use crate::actor::core::spawner::Spawner;
use tracing::debug;

#[derive(Clone)]
pub struct StdSpawnAdapter {
  props: Props,
  spawner: Spawner,
}

impl StdSpawnAdapter {
  pub fn new(props: Props, spawner: Spawner) -> Self {
    Self { props, spawner }
  }

  pub fn props(&self) -> Props {
    self.props.clone()
  }

  pub fn spawner(&self) -> Spawner {
    self.spawner.clone()
  }

  pub fn to_core_chain(adapter: Arc<Self>) -> CoreSpawnMiddlewareChain {
    CoreSpawnMiddlewareChain::new(move |invocation| StdSpawnAdapter::spawn_with_arc(adapter.clone(), invocation))
  }

  fn spawn_with_arc(
    adapter: Arc<Self>,
    invocation: CoreSpawnInvocation,
  ) -> CoreSpawnFuture<'static, Result<CorePid, CoreActorSpawnError>> {
    Box::pin(async move { adapter.execute(invocation).await })
  }

  async fn execute(&self, invocation: CoreSpawnInvocation) -> Result<CorePid, CoreActorSpawnError> {
    let (parent_snapshot, _child_props, reserved_pid, _metadata, _adapter) = invocation.into_parts();
    let (context_snapshot, system_id) = parent_snapshot.into_parts();
    let parent_pid = context_snapshot.self_pid_core();

    let actor_system =
      with_actor_system(system_id, |sys| sys.clone()).ok_or(CoreActorSpawnError::AdapterUnavailable)?;

    let spawner_context = if let Some(context) = ContextRegistry::get(system_id, parent_pid.id()) {
      SpawnerContextHandle::new(context)
    } else {
      debug!(
        target = "nexus::spawn_adapter",
        system_id,
        parent = parent_pid.id(),
        "Spawner context not found; falling back to RootContext"
      );
      let root = actor_system.get_root_context().await;
      SpawnerContextHandle::new(root)
    };

    let result = self
      .spawner
      .run(
        actor_system.clone(),
        reserved_pid.id(),
        self.props.clone(),
        spawner_context,
      )
      .await
      .map_err(|_| CoreActorSpawnError::AdapterFailed)?;

    Ok(result.to_core())
  }
}

impl CoreSpawnAdapter for StdSpawnAdapter {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn spawn<'a>(&'a self, invocation: CoreSpawnInvocation) -> CoreSpawnFuture<'a, Result<CorePid, CoreActorSpawnError>> {
    Box::pin(self.execute(invocation))
  }
}
