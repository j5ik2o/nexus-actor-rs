use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{SenderPart, SpawnerPart};
use nexus_actor_std_rs::actor::core::Props;
use nexus_actor_std_rs::actor::message::{AutoRespond, Message, MessageHandle, ResponseHandle};
use nexus_message_derive_rs::Message;
use std::env;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, PartialEq, Message)]
pub struct MyAutoResponder {
  name: String,
}

#[tokio::main]
async fn main() {
  env::set_var("RUST_LOG", "actor_auto_respond=info");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let system = ActorSystem::new().await.unwrap();
  let mut root = system.get_root_context().await;

  let props = Props::from_sync_actor_receiver(|_| Ok(())).await;

  let pid = root.spawn(props).await;
  let msg = AutoRespond::new(|_| async {
    ResponseHandle::new(MyAutoResponder {
      name: "hello".to_string(),
    })
  });

  let response = root
    .request_future(pid, MessageHandle::new(msg), Duration::from_secs(10))
    .await
    .result()
    .await
    .unwrap();
  let typed_response = response.to_typed::<MyAutoResponder>().unwrap();

  assert_eq!(typed_response.name, "hello");
}
