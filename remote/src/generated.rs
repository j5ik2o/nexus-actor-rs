mod actor {
  pub use nexus_actor_core_rs::generated::actor::*;
}
pub mod remote {
  include!("../generated/remote.rs");
}
pub mod remote_impl;

pub mod cluster {
  include!("../generated/cluster.rs");
}
pub mod cluster_impl;
