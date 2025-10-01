use std::fmt;
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::FutureExt;
use nexus_actor_std_rs::actor::core::Props;

use crate::identity::ClusterIdentity;
use crate::virtual_actor::{make_virtual_actor_props, VirtualActor};

/// Virtual Actor の種類を表す。
#[derive(Clone)]
pub struct ClusterKind {
  name: String,
  props_factory: Arc<dyn Fn(&ClusterIdentity) -> BoxFuture<'static, Props> + Send + Sync>,
}

impl ClusterKind {
  pub fn new<F, Fut>(name: impl Into<String>, props_factory: F) -> Self
  where
    F: Fn(&ClusterIdentity) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Props> + Send + 'static, {
    Self {
      name: name.into(),
      props_factory: Arc::new(move |identity| props_factory(identity).boxed()),
    }
  }

  pub fn virtual_actor<V, F, Fut>(name: impl Into<String>, factory: F) -> Self
  where
    V: VirtualActor,
    F: Fn(ClusterIdentity) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = V> + Send + 'static, {
    let factory = Arc::new(move |identity: ClusterIdentity| factory(identity).boxed());
    Self::new(name, move |identity: &ClusterIdentity| {
      let identity = identity.clone();
      let factory = factory.clone();
      async move { make_virtual_actor_props::<V, _, BoxFuture<'static, V>>(identity, move |id| factory(id)).await }
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn props(&self, identity: &ClusterIdentity) -> BoxFuture<'static, Props> {
    (self.props_factory)(identity)
  }
}

impl fmt::Debug for ClusterKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ClusterKind").field("name", &self.name).finish()
  }
}
