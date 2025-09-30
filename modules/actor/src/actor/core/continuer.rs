use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::message::MessageHandle;
use crate::actor::process::future::ActorFutureError;

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct Continuer(
  Arc<dyn Fn(Option<MessageHandle>, Option<ActorFutureError>) -> BoxFuture<'static, ()> + Send + Sync + 'static>,
);

unsafe impl Send for Continuer {}
unsafe impl Sync for Continuer {}

impl Continuer {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(Option<MessageHandle>, Option<ActorFutureError>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Self(Arc::new(move |m, e| Box::pin(f(m, e))))
  }

  pub async fn run(&self, result: Option<MessageHandle>, error: Option<ActorFutureError>) {
    (self.0)(result, error).await
  }
}

impl Debug for Continuer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Continuer")
  }
}

impl PartialEq for Continuer {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for Continuer {}

impl std::hash::Hash for Continuer {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(Option<MessageHandle>, Option<ActorFutureError>) -> BoxFuture<'static, ()>)
      .hash(state);
  }
}

static_assertions::assert_impl_all!(Continuer: Send, Sync);
