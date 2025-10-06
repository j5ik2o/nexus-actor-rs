use std::sync::Arc;

use nexus_cluster_std_rs::{Cluster, ClusterConfig, InMemoryClusterProvider};

#[tokio::main]
async fn main() {
  let system = Arc::new(
    nexus_actor_std_rs::actor::actor_system::ActorSystem::new()
      .await
      .expect("actor system"),
  );

  let provider = Arc::new(InMemoryClusterProvider::new());
  let cluster = Cluster::new(
    system.clone(),
    ClusterConfig::new("provider-example").with_provider(provider.clone()),
  );

  cluster.start_member().await.expect("start member");
  println!("Members: {:?}", provider.members_snapshot().await);

  cluster.start_client().await.expect("start client");
  println!("Clients: {:?}", provider.clients_snapshot().await);

  cluster.shutdown(true).await.expect("shutdown");
}
