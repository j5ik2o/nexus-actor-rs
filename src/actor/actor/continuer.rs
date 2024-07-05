use std::fmt::Debug;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::future::FutureError;
use crate::actor::message::message_handle::MessageHandle;

#[derive(Clone)]
pub struct Continuer(
  Arc<dyn Fn(Option<MessageHandle>, Option<FutureError>) -> BoxFuture<'static, ()> + Send + Sync + 'static>,
);

unsafe impl Send for Continuer {}
unsafe impl Sync for Continuer {}

impl Continuer {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(Option<MessageHandle>, Option<FutureError>) -> BoxFuture<'static, ()> + Send + Sync + 'static, {
    Self(Arc::new(f))
  }

  pub async fn run(&self, result: Option<MessageHandle>, error: Option<FutureError>) {
    (self.0)(result, error).await
  }
}

impl Debug for Continuer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContinuationFunc")
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
    (self.0.as_ref() as *const dyn Fn(Option<MessageHandle>, Option<FutureError>) -> BoxFuture<'static, ()>)
      .hash(state);
  }
}