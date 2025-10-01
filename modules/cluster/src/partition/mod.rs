pub mod manager;
pub mod messages;
pub mod placement_actor;

#[cfg(test)]
mod tests {
  use super::manager::{ClusterTopology, PartitionManager};
  use crate::cluster::Cluster;
  use crate::config::ClusterConfig;
  use crate::rendezvous::ClusterMember;
  use nexus_actor_std_rs::actor::actor_system::ActorSystem;
  use std::sync::Arc;

  #[tokio::test]
  async fn partition_manager_updates_rendezvous() {
    let system = Arc::new(ActorSystem::new().await.expect("actor system"));
    let cluster = Arc::new(Cluster::new(system.clone(), ClusterConfig::new("test")));
    let manager = PartitionManager::new();
    manager.ensure_cluster_attached(cluster.clone());

    manager
      .update_topology(ClusterTopology {
        members: vec![ClusterMember::new("node-a", vec!["greeter".to_string()])],
      })
      .await;

    let owner = manager.owner_for("greeter", "id");
    assert_eq!(owner, Some("node-a".to_string()));
  }
}
