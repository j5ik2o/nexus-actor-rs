use async_trait::async_trait;
use futures::channel::mpsc::unbounded;
use nexus_actor_core_rs::actor::actor::{Actor, ActorError, ExtendedPid, Props};
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{ContextHandle, InfoPart, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_core_rs::actor::dispatch::{
  unbounded_mailbox_creator_with_opts, MailboxMiddleware, MailboxMiddlewareHandle,
};
use nexus_actor_core_rs::actor::message::Message;
use nexus_actor_core_rs::actor::message::MessageHandle;
use nexus_actor_message_derive_rs::Message;
use std::sync::atomic::AtomicI64;
use std::sync::Arc;
use tokio::time::sleep;

#[derive(Debug, Clone, PartialEq, Message)]
struct RequestMoreWork {
  items: i32,
}

#[derive(Debug, Clone)]
struct RequestWorkBehavior {
  tokens: Arc<AtomicI64>,
  producer: ExtendedPid,
  actor_system: ActorSystem,
}

impl RequestWorkBehavior {
  pub fn new(actor_system: ActorSystem, tokens: i64, producer: ExtendedPid) -> Self {
    Self {
      tokens: Arc::new(AtomicI64::new(tokens)),
      producer,
      actor_system,
    }
  }

  pub async fn request_more(&mut self) {
    tracing::info!("Requesting more tokens");
    self.tokens.store(50, std::sync::atomic::Ordering::Relaxed);
    self
      .actor_system
      .get_root_context()
      .await
      .send(self.producer.clone(), MessageHandle::new(RequestMoreWork { items: 50 }))
  }
}

impl MailboxMiddleware for RequestWorkBehavior {
  async fn mailbox_started(&mut self) {}

  async fn message_posted(&mut self, message_handle: MessageHandle) {}

  async fn message_received(&mut self, message_handle: MessageHandle) {
    self.tokens.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    if self.tokens.load(std::sync::atomic::Ordering::Relaxed) == 0 {
      self.request_more().await;
    }
  }

  async fn mailbox_empty(&mut self) {}
}

#[derive(Debug, Clone)]
struct Producer {
  requested_work: i32,
  produced_work: i32,
  worker: Option<ExtendedPid>,
}

impl Actor for Producer {
  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let request_more_work_opt = ctx.get_message_handle().await.to_typed::<RequestMoreWork>();
    if let Some(request_more_work) = request_more_work_opt {
      self.requested_work += request_more_work.items;
      self.produced_work = 0;
      ctx.send(ctx.get_self().await, MessageHandle::new(Produce {})).await;
    }
    let produce_opt = ctx.get_message_handle().await.to_typed::<Produce>();
    if produce_opt.is_some() {
      self.produced_work += 1;
      ctx
        .send(
          self.worker.clone().expect("Not found"),
          MessageHandle::new(Work { id: self.produced_work }),
        )
        .await;
      if self.requested_work > 0 {
        self.requested_work -= 1;
        ctx.send(ctx.get_self().await, MessageHandle::new(Produce {})).await;
      }
    }
    Ok(())
  }

  async fn post_start(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let actor_system = ctx.get_actor_system().await;
    let mb = unbounded_mailbox_creator_with_opts([MailboxMiddlewareHandle::new(RequestWorkBehavior::new(
      actor_system,
      0,
      ctx.get_self().await,
    ))]);
    let worker_props =
      Props::from_async_actor_producer_with_opts(move |_| async { Worker {} }, [Props::with_mailbox_producer(mb)]);
    self.worker = Some(ctx.spawn(worker_props).await);
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Message)]
struct Produce;

#[derive(Debug, Clone)]
struct Worker;

#[async_trait]
impl Actor for Worker {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let work_opt = context_handle.get_message_handle().await.to_typed::<Work>();
    if let Some(work) = work_opt {
      tracing::info!("Received work: {:?}", work);
      sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Message)]
struct Work {
  id: i32,
}

#[tokio::main]
async fn main() {
  let system = ActorSystem::new().await.unwrap();
  let props = Props::from_async_actor_producer(|_| async {
    Producer {
      requested_work: 0,
      produced_work: 0,
      worker: None,
    }
  })
  .await;
  system.get_root_context().await.spawn(props).await;
  sleep(std::time::Duration::from_secs(10)).await;
}
