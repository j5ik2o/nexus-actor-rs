use crate::log::log_event::LogEvent;
use futures::future::BoxFuture;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

#[derive(Clone)]
pub struct LogEventHandler(Arc<dyn Fn(LogEvent) -> BoxFuture<'static, ()> + Send + Sync>);

impl LogEventHandler {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(LogEvent) -> Fut + Send + Sync + 'static,
    Fut: futures::Future<Output = ()> + Send + 'static, {
    Self(Arc::new(move |evt| Box::pin(f(evt))))
  }

  pub async fn run(self, event: LogEvent) {
    self.0(event).await
  }
}

impl Debug for LogEventHandler {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "EventHandler")
  }
}

impl PartialEq for LogEventHandler {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for LogEventHandler {}

impl std::hash::Hash for LogEventHandler {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(LogEvent) -> BoxFuture<'static, ()>).hash(state);
  }
}
