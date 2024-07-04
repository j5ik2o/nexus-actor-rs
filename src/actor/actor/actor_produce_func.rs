use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::actor::ActorHandle;
use crate::actor::context::context_handle::ContextHandle;

#[derive(Clone)]
pub struct ActorProduceFunc(Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, ActorHandle> + Send + Sync>);

impl Debug for ActorProduceFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Producer")
  }
}

impl PartialEq for ActorProduceFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ActorProduceFunc {}

impl std::hash::Hash for ActorProduceFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle) -> BoxFuture<'static, ActorHandle>).hash(state);
  }
}

impl ActorProduceFunc {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ActorHandle> + Send + 'static, {
    Self(Arc::new(move |ch| Box::pin(f(ch)) as BoxFuture<'static, ActorHandle>))
  }

  pub async fn run(&self, c: ContextHandle) -> ActorHandle {
    (self.0)(c).await
  }
}
