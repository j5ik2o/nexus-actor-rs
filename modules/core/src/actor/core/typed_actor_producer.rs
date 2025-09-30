use crate::actor::context::TypedContextHandle;
use crate::actor::core::typed_actor::{TypedActor, TypedActorWrapper};
use crate::actor::core::typed_actor_handle::{TypeWrapperActorHandle, TypedActorHandle};
use crate::actor::core::{ActorHandle, ActorProducer};
use crate::actor::message::Message;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct TypedActorProducer<M: Message> {
  underlying: ActorProducer,
  phantom_data: PhantomData<M>,
}

unsafe impl<M: Message> Send for TypedActorProducer<M> {}
unsafe impl<M: Message> Sync for TypedActorProducer<M> {}

impl<M: Message + Clone> TypedActorProducer<M> {
  pub fn new<A, F, Fut>(f: F) -> Self
  where
    A: TypedActor<M>,
    F: Fn(TypedContextHandle<M>) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = A> + Send + 'static, {
    let p = ActorProducer::new(move |c| {
      let f = f.clone();
      async move {
        let ctx = TypedContextHandle::new(c);
        let a = f(ctx).await;
        let a = TypedActorWrapper::new(a);
        ActorHandle::new(a)
      }
    });
    Self {
      underlying: p,
      phantom_data: PhantomData,
    }
  }

  pub fn get_underlying(&self) -> &ActorProducer {
    &self.underlying
  }

  pub async fn run(&self, c: TypedContextHandle<M>) -> TypedActorHandle<M> {
    TypedActorHandle::new(TypeWrapperActorHandle::<M>::new(
      self.underlying.run(c.get_underlying().clone()).await,
    ))
  }
}

impl<M: Message> Debug for TypedActorProducer<M> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Producer")
  }
}

impl<M: Message> PartialEq for TypedActorProducer<M> {
  fn eq(&self, other: &Self) -> bool {
    self.underlying == other.underlying
  }
}

impl<M: Message> Eq for TypedActorProducer<M> {}

impl<M: Message> std::hash::Hash for TypedActorProducer<M> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.underlying.hash(state);
  }
}
