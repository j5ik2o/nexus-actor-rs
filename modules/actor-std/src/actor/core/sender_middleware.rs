use std::fmt::{Debug, Formatter};

use nexus_actor_core_rs::context::CoreSenderMiddleware;

use crate::actor::core::sender_middleware_chain::{SenderInvocation, SenderMiddlewareChain};

#[derive(Clone)]
pub struct SenderMiddleware(CoreSenderMiddleware<SenderInvocation>);

unsafe impl Send for SenderMiddleware {}
unsafe impl Sync for SenderMiddleware {}

impl SenderMiddleware {
  pub fn new(f: impl Fn(SenderMiddlewareChain) -> SenderMiddlewareChain + Send + Sync + 'static) -> Self {
    let middleware = CoreSenderMiddleware::new(move |chain| {
      let wrapped = SenderMiddlewareChain::from_core(chain);
      let result = f(wrapped);
      result.into_core()
    });
    SenderMiddleware(middleware)
  }

  pub fn run(&self, next: SenderMiddlewareChain) -> SenderMiddlewareChain {
    let result = self.0.run(next.into_core());
    SenderMiddlewareChain::from_core(result)
  }

  pub fn as_core(&self) -> &CoreSenderMiddleware<SenderInvocation> {
    &self.0
  }

  pub fn into_core(self) -> CoreSenderMiddleware<SenderInvocation> {
    self.0
  }
}

impl Debug for SenderMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SenderMiddleware")
  }
}

impl PartialEq for SenderMiddleware {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl Eq for SenderMiddleware {}

impl std::hash::Hash for SenderMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.0.hash(state);
  }
}

static_assertions::assert_impl_all!(SenderMiddleware: Send, Sync);
