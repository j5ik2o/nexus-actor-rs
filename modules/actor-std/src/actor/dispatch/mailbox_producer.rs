use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::dispatch::MailboxHandle;

#[derive(Clone)]
pub struct MailboxProducer(Arc<dyn Fn() -> BoxFuture<'static, MailboxHandle> + Send + Sync + 'static>);

unsafe impl Send for MailboxProducer {}
unsafe impl Sync for MailboxProducer {}

impl Debug for MailboxProducer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "MailboxProducer")
  }
}

impl PartialEq for MailboxProducer {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for MailboxProducer {}

impl std::hash::Hash for MailboxProducer {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn() -> BoxFuture<'static, MailboxHandle>).hash(state);
  }
}

impl MailboxProducer {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = MailboxHandle> + Send + 'static, {
    Self(Arc::new(move || Box::pin(f()) as BoxFuture<'static, MailboxHandle>))
  }

  pub async fn run(&self) -> MailboxHandle {
    (self.0)().await
  }
}

static_assertions::assert_impl_all!(MailboxProducer: Send, Sync);
