use std::env;
use std::sync::Arc;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::InfoPart;
use crate::actor::context::TypedContextHandle;
use crate::actor::core::ActorError;
use crate::actor::core::{TypedActor, TypedProps};
use crate::actor::message::Message;
use crate::actor::supervisor::SupervisorStrategyHandle;
use crate::actor::typed_context::{TypedSenderPart, TypedSpawnerPart};
use crate::actor::{Config, ConfigOption, MetricsProvider};
use async_trait::async_trait;
use nexus_message_derive_rs::Message;
use nexus_utils_std_rs::concurrent::AsyncBarrier;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[tokio::test]
async fn test_actor_system_new() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();
  let root = system.get_root_context().await;
  assert_eq!(root.get_self_opt().await, None);

  sleep(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn test_actor_system_new_with_config() {
  let system = ActorSystem::new_with_config(Config::default()).await.unwrap();
  let root = system.get_root_context().await;
  assert_eq!(root.get_self_opt().await, None);
}

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct Hello(pub String);

#[derive(Debug)]
struct MyActor {
  b: AsyncBarrier,
}

#[async_trait]
impl TypedActor<Hello> for MyActor {
  async fn receive(&mut self, _: TypedContextHandle<Hello>) -> Result<(), ActorError> {
    self.b.wait().await;
    Ok(())
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[tokio::test]
async fn test_actor_system_spawn_actor() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let b = AsyncBarrier::new(2);
  let cloned_b = b.clone();
  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_typed_root_context().await;

  let props = TypedProps::from_async_actor_producer(move |_| {
    let cloned_b = b.clone();
    async move { MyActor { b: cloned_b.clone() } }
  })
  .await;

  let pid = root_context.spawn(props).await;
  root_context.send(pid, Hello("hello".to_string())).await;

  cloned_b.wait().await;
}

#[tokio::test]
async fn test_metrics_foreach_sync_access() {
  let provider = Arc::new(MetricsProvider::Sdk(SdkMeterProvider::default()));
  let system = ActorSystem::new_config_options([ConfigOption::SetMetricsProvider(provider)])
    .await
    .unwrap();

  let address_via_metrics = system
    .metrics_foreach(|runtime| runtime.address().to_string())
    .expect("metrics runtime must be available");

  let direct_address = system.get_address().await;
  assert_eq!(address_via_metrics, direct_address);
}

#[tokio::test]
async fn test_metrics_foreach_disabled_returns_none() {
  let system = ActorSystem::new().await.unwrap();
  assert!(system
    .metrics_foreach(|runtime| runtime.address().to_string())
    .is_none());
}
