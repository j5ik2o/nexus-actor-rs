use crate::actor::context::TypedContextHandle;
use crate::actor::core::typed_actor::{TypedActor, TypedActorWrapper};
use crate::actor::core::typed_actor_producer::TypedActorProducer;
use crate::actor::core::typed_actor_receiver::TypedActorReceiver;
use crate::actor::core::{ActorHandle, ActorProducer, ActorReceiverActor, Props, PropsOption};
use crate::actor::message::Message;
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TypedProps<M: Message> {
  underlying: Props,
  phantom_data: PhantomData<M>,
}

impl<M: Message + Clone> TypedProps<M> {
  pub fn new(underlying: Props) -> Self {
    Self {
      underlying,
      phantom_data: PhantomData,
    }
  }

  pub async fn from_async_actor_producer<A, F, Fut>(f: F) -> TypedProps<M>
  where
    A: TypedActor<M>,
    F: Fn(TypedContextHandle<M>) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = A> + Send + 'static, {
    Self::from_async_actor_producer_with_opts(f, []).await
  }

  pub async fn from_async_actor_producer_with_opts<A, F, Fut>(
    f: F,
    opts: impl IntoIterator<Item = PropsOption>,
  ) -> TypedProps<M>
  where
    A: TypedActor<M>,
    F: Fn(TypedContextHandle<M>) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = A> + Send + 'static, {
    Props::from_async_actor_producer_with_opts(
      move |c| {
        let f = f.clone();
        async move {
          let ctx = TypedContextHandle::new(c);
          let a = f(ctx).await;
          let a = TypedActorWrapper::new(a);
          ActorHandle::new(a)
        }
      },
      opts,
    )
    .await
    .into()
  }

  pub async fn from_sync_actor_producer<A, F>(f: F) -> TypedProps<M>
  where
    A: TypedActor<M>,
    F: Fn(TypedContextHandle<M>) -> A + Clone + Send + Sync + 'static, {
    let f = Arc::new(f);
    Self::from_async_actor_producer(move |ctx| {
      let f = f.clone();
      async move { f(ctx) }
    })
    .await
  }

  pub async fn from_sync_actor_producer_with_opts<A, F>(
    f: F,
    opts: impl IntoIterator<Item = PropsOption>,
  ) -> TypedProps<M>
  where
    A: TypedActor<M>,
    F: Fn(TypedContextHandle<M>) -> A + Clone + Send + Sync + 'static, {
    let f = Arc::new(f);
    Self::from_async_actor_producer_with_opts(
      move |ctx| {
        let f = f.clone();
        async move { f(ctx) }
      },
      opts,
    )
    .await
  }

  pub async fn from_async_actor_receiver<F, Fut>(f: F) -> TypedProps<M>
  where
    F: Fn(TypedContextHandle<M>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), crate::actor::core::ActorError>> + Send + 'static, {
    Self::from_async_actor_receiver_with_opts(f, []).await
  }

  pub async fn from_async_actor_receiver_with_opts<F, Fut>(
    f: F,
    opts: impl IntoIterator<Item = PropsOption>,
  ) -> TypedProps<M>
  where
    F: Fn(TypedContextHandle<M>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), crate::actor::core::ActorError>> + Send + 'static, {
    Props::from_async_actor_receiver_with_opts(
      move |c| {
        let r = f(TypedContextHandle::new(c));
        Box::pin(r) as futures::future::BoxFuture<'static, Result<(), crate::actor::core::ActorError>>
      },
      opts,
    )
    .await
    .into()
  }

  pub async fn from_sync_actor_receiver<F>(f: F) -> TypedProps<M>
  where
    F: Fn(TypedContextHandle<M>) -> Result<(), crate::actor::core::ActorError> + Send + Sync + 'static, {
    let f = Arc::new(f);
    Self::from_async_actor_receiver(move |ctx| {
      let f = f.clone();
      async move { f(ctx) }
    })
    .await
  }

  pub async fn from_sync_actor_receiver_with_opts<F>(
    f: F,
    opts: impl IntoIterator<Item = PropsOption>,
  ) -> TypedProps<M>
  where
    F: Fn(TypedContextHandle<M>) -> Result<(), crate::actor::core::ActorError> + Send + Sync + 'static, {
    let f = Arc::new(f);
    Self::from_async_actor_receiver_with_opts(
      move |ctx| {
        let f = f.clone();
        async move { f(ctx) }
      },
      opts,
    )
    .await
  }

  pub fn with_actor_producer(producer: TypedActorProducer<M>) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      props.producer = Some(producer.get_underlying().clone());
    })
  }

  pub fn with_actor_receiver(actor_receiver: TypedActorReceiver<M>) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      let actor_receiver = actor_receiver.clone();
      props.producer = Some(ActorProducer::from_handle(move |_| {
        let actor_receiver = actor_receiver.clone();
        async move {
          let actor = ActorReceiverActor::new(actor_receiver.get_underlying().clone());
          ActorHandle::new(actor)
        }
      }));
    })
  }

  pub fn get_underlying(&self) -> &Props {
    &self.underlying
  }
}

impl<M: Message + Clone> From<Props> for TypedProps<M> {
  fn from(props: Props) -> Self {
    Self::new(props)
  }
}

impl<M: Message> From<TypedProps<M>> for Props {
  fn from(typed_props: TypedProps<M>) -> Self {
    typed_props.underlying
  }
}
