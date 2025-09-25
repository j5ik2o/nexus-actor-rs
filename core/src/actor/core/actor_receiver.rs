use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::context::ContextHandle;
use crate::actor::core::actor_error::ActorError;

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct ActorReceiver(
  Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync + 'static>,
);

unsafe impl Send for ActorReceiver {}
unsafe impl Sync for ActorReceiver {}

impl ActorReceiver {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    ActorReceiver(Arc::new(move |ch| {
      Box::pin(f(ch)) as BoxFuture<'static, Result<(), ActorError>>
    }))
  }

  pub async fn run(&self, context: ContextHandle) -> Result<(), ActorError> {
    (self.0)(context).await
  }
}

impl Debug for ActorReceiver {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ActorReceiver")
  }
}

impl PartialEq for ActorReceiver {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ActorReceiver {}

impl std::hash::Hash for ActorReceiver {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle) -> BoxFuture<'static, Result<(), ActorError>>).hash(state);
  }
}

static_assertions::assert_impl_all!(ActorReceiver: Send, Sync);
