use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::actor::ActorError;
use crate::actor::context::context_handle::ContextHandle;

#[derive(Clone)]
pub struct ReceiveFunc(Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync>);

impl Debug for ReceiveFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ReceiveFunc")
  }
}

impl PartialEq for ReceiveFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ReceiveFunc {}

impl std::hash::Hash for ReceiveFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle) -> BoxFuture<'static, Result<(), ActorError>>).hash(state);
  }
}

impl ReceiveFunc {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    ReceiveFunc(Arc::new(move |ch| {
      Box::pin(f(ch)) as BoxFuture<'static, Result<(), ActorError>>
    }))
  }

  pub async fn run(&self, context: ContextHandle) -> Result<(), ActorError> {
    (self.0)(context).await
  }
}
