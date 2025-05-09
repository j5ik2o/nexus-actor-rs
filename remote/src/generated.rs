mod actor {
  pub use nexus_actor_core_rs::generated::actor::*;
}
pub mod remote {
  #![allow(clippy::enum_variant_names)]
  include!("../generated/remote.rs");
}
pub mod remote_impl;

pub mod cluster {
  #![allow(clippy::enum_variant_names)]
  include!("../generated/cluster.rs");
}
pub mod cluster_impl;
