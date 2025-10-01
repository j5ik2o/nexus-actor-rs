use crate::actor::context::TypedContextHandle;
use crate::actor::core::{ActorError, ActorReceiver};
use crate::actor::message::Message;
use futures::future::BoxFuture;
use std::future::Future;
use std::marker::PhantomData;

#[derive(Debug, Clone)]
pub struct TypedActorReceiver<M: Message> {
  underlying: ActorReceiver,
  phantom_data: PhantomData<M>,
}

impl<M: Message> TypedActorReceiver<M> {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(TypedContextHandle<M>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    let underlying = ActorReceiver::new(move |ch| {
      let r = f(TypedContextHandle::new(ch));
      Box::pin(r) as BoxFuture<'static, Result<(), ActorError>>
    });
    Self {
      underlying,
      phantom_data: PhantomData,
    }
  }

  pub async fn run(&self, context: TypedContextHandle<M>) -> Result<(), ActorError> {
    self.underlying.run(context.get_underlying().clone()).await
  }

  pub fn get_underlying(&self) -> &ActorReceiver {
    &self.underlying
  }
}
