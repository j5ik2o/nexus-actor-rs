use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::context::ContextHandle;

// ContextDecoratorChain
#[derive(Clone)]
pub struct ContextDecoratorChain(
  Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, ContextHandle> + Send + Sync + 'static>,
);

unsafe impl Send for ContextDecoratorChain {}
unsafe impl Sync for ContextDecoratorChain {}

impl ContextDecoratorChain {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ContextHandle> + Send + 'static, {
    Self(Arc::new(move |ch| Box::pin(f(ch)) as BoxFuture<'static, ContextHandle>))
  }

  pub async fn run(&self, context: ContextHandle) -> ContextHandle {
    (self.0)(context).await
  }
}

impl Debug for ContextDecoratorChain {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextDecoratorChain")
  }
}

impl PartialEq for ContextDecoratorChain {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ContextDecoratorChain {}

impl std::hash::Hash for ContextDecoratorChain {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle) -> BoxFuture<'static, ContextHandle>).hash(state);
  }
}
