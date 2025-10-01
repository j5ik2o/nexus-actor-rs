use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use nexus_actor_std_rs::actor::core::{ActorError, ErrorReason};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_message_derive_rs::Message as MessageDerive;
use tokio::time::timeout;

use nexus_cluster_std_rs::{
  Cluster, ClusterConfig, ClusterIdentity, ClusterKind, InMemoryClusterProvider, VirtualActor, VirtualActorContext,
  VirtualActorRuntime,
};

#[derive(Debug, Clone, PartialEq, MessageDerive)]
struct Greet(pub String);

#[derive(Debug, Clone, PartialEq, MessageDerive)]
struct Greeted(pub String);

#[derive(Debug, Default)]
struct GreeterActor;

#[async_trait]
impl VirtualActor for GreeterActor {
  async fn activate(&mut self, ctx: &VirtualActorContext) -> Result<(), ActorError> {
    println!("[activate] kind={} id={}", ctx.identity().kind(), ctx.identity().id());
    Ok(())
  }

  async fn handle(&mut self, message: MessageHandle, runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
    if let Some(greet) = message.as_typed::<Greet>() {
      let reply = Greeted(format!("Hello, {}!", greet.0));
      runtime.respond(reply).await;
      return Ok(());
    }

    Err(ActorError::of_receive_error(ErrorReason::from("unsupported message")))
  }

  async fn deactivate(&mut self, ctx: &VirtualActorContext) -> Result<(), ActorError> {
    println!("[deactivate] kind={} id={}", ctx.identity().kind(), ctx.identity().id());
    Ok(())
  }
}

#[tokio::main]
async fn main() {
  let system = Arc::new(
    nexus_actor_std_rs::actor::actor_system::ActorSystem::new()
      .await
      .expect("actor system"),
  );

  let provider = Arc::new(InMemoryClusterProvider::new());
  let config = ClusterConfig::new("virtual-actor-example")
    .with_provider(provider.clone())
    .with_request_timeout(Duration::from_secs(2));
  let cluster = Cluster::new(system.clone(), config);

  cluster.start_member().await.expect("start member");

  cluster.register_kind(ClusterKind::virtual_actor("greeter", move |_identity| async move {
    GreeterActor::default()
  }));

  let identity = ClusterIdentity::new("greeter", "alice");

  let response = cluster
    .request_message(identity.clone(), Greet("World".into()))
    .await
    .expect("greet response");

  let greeted = response.to_typed::<Greeted>().expect("typed response");
  println!("Actor replied: {}", greeted.0);

  let timed_out = timeout(
    Duration::from_millis(200),
    cluster.request_message_with_timeout(identity, Greet("Timeout".into()), Duration::from_millis(50)),
  )
  .await
  .expect("timeout future ready");

  if let Err(err) = timed_out {
    println!("Request failed as expected: {err}");
  }

  cluster.shutdown(true).await.expect("shutdown");
  tokio::time::sleep(Duration::from_millis(100)).await;
}
