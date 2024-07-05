use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::context::context_handle::ContextHandle;

// ContextDecoratorFunc
#[derive(Clone)]
pub struct ContextDecoratorChainFunc(Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, ContextHandle> + Send + Sync>);

unsafe impl Send for ContextDecoratorChainFunc {}
unsafe impl Sync for ContextDecoratorChainFunc {}

impl ContextDecoratorChainFunc {
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

impl Debug for ContextDecoratorChainFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextDecoratorFunc")
  }
}

impl PartialEq for ContextDecoratorChainFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ContextDecoratorChainFunc {}

impl std::hash::Hash for ContextDecoratorChainFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle) -> BoxFuture<'static, ContextHandle>).hash(state);
  }
}


