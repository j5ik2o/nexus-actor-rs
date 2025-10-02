use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::context::{ContextHandle, ContextSnapshot};

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct ContextDecoratorChain {
  sync_chain: Arc<dyn Fn(ContextSnapshot) -> ContextSnapshot + Send + Sync + 'static>,
  tail: Arc<dyn Fn(ContextSnapshot) -> BoxFuture<'static, ContextHandle> + Send + Sync + 'static>,
}

unsafe impl Send for ContextDecoratorChain {}
unsafe impl Sync for ContextDecoratorChain {}

impl ContextDecoratorChain {
  pub fn new<F, Fut>(tail: F) -> Self
  where
    F: Fn(ContextSnapshot) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ContextHandle> + Send + 'static, {
    let tail_arc: Arc<dyn Fn(ContextSnapshot) -> BoxFuture<'static, ContextHandle> + Send + Sync> =
      Arc::new(move |snapshot| Box::pin(tail(snapshot)) as BoxFuture<'static, ContextHandle>);

    Self {
      sync_chain: Arc::new(|snapshot| snapshot),
      tail: tail_arc,
    }
  }

  pub fn with_decorator(
    self,
    decorator: Arc<dyn Fn(ContextSnapshot) -> ContextSnapshot + Send + Sync + 'static>,
  ) -> Self {
    let prev_sync = self.sync_chain.clone();
    Self {
      sync_chain: Arc::new(move |snapshot| {
        let transformed = decorator.as_ref()(snapshot);
        prev_sync.as_ref()(transformed)
      }),
      tail: self.tail.clone(),
    }
  }

  pub fn prepend(self, decorator: super::context_decorator::ContextDecorator) -> Self {
    let decorator_fn = decorator.into_inner();
    self.with_decorator(decorator_fn)
  }

  pub async fn run(&self, context: ContextHandle) -> ContextHandle {
    let snapshot = context.snapshot_with_core().await;
    let transformed_snapshot = (self.sync_chain)(snapshot);
    (self.tail)(transformed_snapshot).await
  }
}

impl Debug for ContextDecoratorChain {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextDecoratorChain")
  }
}

impl PartialEq for ContextDecoratorChain {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.sync_chain, &other.sync_chain) && Arc::ptr_eq(&self.tail, &other.tail)
  }
}

impl Eq for ContextDecoratorChain {}

impl std::hash::Hash for ContextDecoratorChain {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.sync_chain.as_ref() as *const dyn Fn(ContextSnapshot) -> ContextSnapshot).hash(state);
    (self.tail.as_ref() as *const dyn Fn(ContextSnapshot) -> BoxFuture<'static, ContextHandle>).hash(state);
  }
}

static_assertions::assert_impl_all!(ContextDecoratorChain: Send, Sync);
