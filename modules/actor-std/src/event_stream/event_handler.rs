use crate::actor::message::MessageHandle;
use futures::future::BoxFuture;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

// Handler defines a callback function that must be passed when subscribing.
#[derive(Clone)]
pub struct EventHandler(Arc<dyn Fn(MessageHandle) -> BoxFuture<'static, ()> + Send + Sync + 'static>);

unsafe impl Send for EventHandler {}
unsafe impl Sync for EventHandler {}

impl EventHandler {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(MessageHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Self(Arc::new(move |mh| Box::pin(f(mh)) as BoxFuture<'static, ()>))
  }

  pub async fn run(&self, evt: MessageHandle) {
    (self.0)(evt).await
  }
}

impl Debug for EventHandler {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Handler")
  }
}

impl PartialEq for EventHandler {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for EventHandler {}

impl std::hash::Hash for EventHandler {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(MessageHandle) -> BoxFuture<'static, ()>).hash(state);
  }
}
