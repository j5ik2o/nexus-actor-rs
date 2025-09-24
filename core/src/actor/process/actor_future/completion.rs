use crate::actor::message::MessageHandle;
use crate::actor::process::future::ActorFutureError;
use futures::future::BoxFuture;
use std::fmt::Debug;
use std::sync::Arc;

type CompletionFunc =
  Arc<dyn Fn(Option<MessageHandle>, Option<ActorFutureError>) -> BoxFuture<'static, ()> + Send + Sync + 'static>;

#[derive(Clone)]
pub(crate) struct Completion(CompletionFunc);

unsafe impl Send for Completion {}
unsafe impl Sync for Completion {}

impl Completion {
  pub(crate) fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(Option<MessageHandle>, Option<ActorFutureError>) -> Fut + Send + Sync + 'static,
    Fut: core::future::Future<Output = ()> + Send + 'static, {
    Self(Arc::new(move |message, error| {
      Box::pin(f(message, error)) as BoxFuture<'static, ()>
    }))
  }

  pub(crate) async fn run(&self, result: Option<MessageHandle>, error: Option<ActorFutureError>) {
    (self.0)(result, error).await
  }
}

impl Debug for Completion {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Completion")
  }
}
