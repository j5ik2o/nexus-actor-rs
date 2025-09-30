use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;

use crate::actor::context::ContextHandle;
use crate::actor::message::message_base::Message;
use crate::actor::message::response::ResponseHandle;

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct AutoRespond(Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, ResponseHandle> + Send + Sync + 'static>);

unsafe impl Send for AutoRespond {}
unsafe impl Sync for AutoRespond {}

impl AutoRespond {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ResponseHandle> + Send + 'static, {
    Self(Arc::new(move |mh| Box::pin(f(mh))))
  }
}

impl Debug for AutoRespond {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "AutoRespond")
  }
}

impl Message for AutoRespond {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.eq_message(self)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

#[async_trait]
pub trait AutoResponsive: Debug + Send + Sync + 'static {
  async fn get_auto_response(&self, context: ContextHandle) -> ResponseHandle;
}

#[async_trait]
impl AutoResponsive for AutoRespond {
  async fn get_auto_response(&self, ctx: ContextHandle) -> ResponseHandle {
    (self.0)(ctx).await
  }
}

#[derive(Debug, Clone)]
pub struct AutoRespondHandle(Arc<dyn AutoResponsive>);

impl AutoRespondHandle {
  pub fn new(auto_respond: Arc<dyn AutoResponsive>) -> Self {
    Self(auto_respond)
  }
}

#[async_trait]
impl AutoResponsive for AutoRespondHandle {
  async fn get_auto_response(&self, context: ContextHandle) -> ResponseHandle {
    self.0.get_auto_response(context).await
  }
}

impl PartialEq for AutoRespondHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for AutoRespondHandle {}

impl std::hash::Hash for AutoRespondHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    Arc::as_ptr(&self.0).hash(state)
  }
}
