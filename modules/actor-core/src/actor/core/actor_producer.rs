use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use crate::actor::context::ContextHandle;
use crate::actor::core::actor::Actor;
use crate::actor::core::actor_handle::ActorHandle;
use futures::future::BoxFuture;

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct ActorProducer(Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, ActorHandle> + Send + Sync + 'static>);

unsafe impl Send for ActorProducer {}
unsafe impl Sync for ActorProducer {}

impl ActorProducer {
  pub fn from_handle<F, Fut>(f: F) -> Self
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ActorHandle> + Send + 'static, {
    Self(Arc::new(move |ch| Box::pin(f(ch)) as BoxFuture<'static, ActorHandle>))
  }

  pub fn new<A, F, Fut>(f: F) -> Self
  where
    A: Actor,
    F: Fn(ContextHandle) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = A> + Send + 'static, {
    Self::from_handle(move |c| {
      let f = f.clone();
      async move {
        let a = f(c).await;
        ActorHandle::new(a)
      }
    })
  }

  pub async fn run(&self, c: ContextHandle) -> ActorHandle {
    (self.0)(c).await
  }
}

impl Debug for ActorProducer {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Producer")
  }
}

impl PartialEq for ActorProducer {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ActorProducer {}

impl std::hash::Hash for ActorProducer {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle) -> BoxFuture<'static, ActorHandle>).hash(state);
  }
}

static_assertions::assert_impl_all!(ActorProducer: Send, Sync);
