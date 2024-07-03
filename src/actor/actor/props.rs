use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use once_cell::sync::Lazy;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::actor::actor::actor_process::ActorProcess;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::{Actor, ActorError, ActorHandle};
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::{ContextHandle, InfoPart, ReceiverPart, SpawnerContextHandle};
use crate::actor::dispatch::dispatcher::*;
use crate::actor::dispatch::mailbox::{Mailbox, MailboxHandle, MailboxProduceFunc};
use crate::actor::dispatch::message_invoker::MessageInvokerHandle;
use crate::actor::dispatch::unbounded::unbounded_mailbox_creator_with_opts;
use crate::actor::message::{ContextDecoratorFunc, MessageHandle, ProducerFunc, ReceiveFunc, ReceiverFunc, SenderFunc};
use crate::actor::messages::{Started, SystemMessage};
use crate::actor::middleware_chain::{
  make_context_decorator_chain, make_receiver_middleware_chain, make_sender_middleware_chain,
  make_spawn_middleware_chain,
};
use crate::actor::process::ProcessHandle;
use crate::actor::supervisor::supervisor_strategy::{SupervisorStrategyHandle, DEFAULT_SUPERVISION_STRATEGY};

#[derive(Debug, Clone, Error)]
pub enum SpawnError {
  #[error("Name already exists: {0}")]
  ErrNameExists(ExtendedPid),
}

#[derive(Clone)]
pub struct SpawnFunc(
  Arc<
    dyn Fn(ActorSystem, String, Props, SpawnerContextHandle) -> BoxFuture<'static, Result<ExtendedPid, SpawnError>>
      + Send
      + Sync,
  >,
);

impl Debug for SpawnFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SpawnFunc")
  }
}

impl PartialEq for SpawnFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for SpawnFunc {}

impl std::hash::Hash for SpawnFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref()
      as *const dyn Fn(
        ActorSystem,
        String,
        Props,
        SpawnerContextHandle,
      ) -> BoxFuture<'static, Result<ExtendedPid, SpawnError>>)
      .hash(state);
  }
}

impl SpawnFunc {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ActorSystem, String, Props, SpawnerContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<ExtendedPid, SpawnError>> + Send + 'static, {
    Self(Arc::new(move |s, name, p, sch| {
      Box::pin(f(s, name, p, sch)) as BoxFuture<'static, Result<ExtendedPid, SpawnError>>
    }))
  }

  pub async fn run(
    &self,
    actor_system: ActorSystem,
    name: &str,
    props: Props,
    parent_context: SpawnerContextHandle,
  ) -> Result<ExtendedPid, SpawnError> {
    (self.0)(actor_system, name.to_string(), props, parent_context).await
  }
}

#[derive(Clone)]
pub struct ReceiverMiddleware(Arc<dyn Fn(ReceiverFunc) -> ReceiverFunc + Send + Sync>);

impl Debug for ReceiverMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ReceiverMiddleware")
  }
}

impl PartialEq for ReceiverMiddleware {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for ReceiverMiddleware {}

impl std::hash::Hash for ReceiverMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ReceiverFunc) -> ReceiverFunc).hash(state);
  }
}

impl ReceiverMiddleware {
  pub fn new(f: impl Fn(ReceiverFunc) -> ReceiverFunc + Send + Sync + 'static) -> Self {
    ReceiverMiddleware(Arc::new(f))
  }

  pub fn run(&self, next: ReceiverFunc) -> ReceiverFunc {
    (self.0)(next)
  }
}

#[derive(Clone)]
pub struct SenderMiddleware(Arc<dyn Fn(SenderFunc) -> SenderFunc + Send + Sync>);

impl Debug for SenderMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SenderMiddleware")
  }
}

impl PartialEq for SenderMiddleware {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for SenderMiddleware {}

impl std::hash::Hash for SenderMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(SenderFunc) -> SenderFunc).hash(state);
  }
}

impl SenderMiddleware {
  pub fn new(f: impl Fn(SenderFunc) -> SenderFunc + Send + Sync + 'static) -> Self {
    SenderMiddleware(Arc::new(f))
  }

  pub fn run(&self, next: SenderFunc) -> SenderFunc {
    (self.0)(next)
  }
}

#[derive(Clone)]
pub struct ContextDecorator(Arc<dyn Fn(ContextDecoratorFunc) -> ContextDecoratorFunc + Send + Sync>);

impl Debug for ContextDecorator {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextDecorator")
  }
}

impl PartialEq for ContextDecorator {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for ContextDecorator {}

impl std::hash::Hash for ContextDecorator {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextDecoratorFunc) -> ContextDecoratorFunc).hash(state);
  }
}

impl ContextDecorator {
  pub fn new(f: impl Fn(ContextDecoratorFunc) -> ContextDecoratorFunc + Send + Sync + 'static) -> Self {
    ContextDecorator(Arc::new(f))
  }

  pub fn run(&self, next: ContextDecoratorFunc) -> ContextDecoratorFunc {
    (self.0)(next)
  }
}

#[derive(Clone)]
pub struct SpawnMiddleware(Arc<dyn Fn(SpawnFunc) -> SpawnFunc + Send + Sync>);

impl Debug for SpawnMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SpawnMiddleware")
  }
}

impl PartialEq for SpawnMiddleware {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for SpawnMiddleware {}

impl std::hash::Hash for SpawnMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(SpawnFunc) -> SpawnFunc).hash(state);
  }
}

impl SpawnMiddleware {
  pub fn new(f: impl Fn(SpawnFunc) -> SpawnFunc + Send + Sync + 'static) -> Self {
    SpawnMiddleware(Arc::new(f))
  }

  pub fn run(&self, next: SpawnFunc) -> SpawnFunc {
    self.0(next)
  }
}

#[derive(Clone)]
pub struct ContextHandleFunc(Arc<dyn Fn(ContextHandle) + Send + Sync>);

impl Debug for ContextHandleFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextHandleFunc")
  }
}

impl PartialEq for ContextHandleFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for ContextHandleFunc {}

impl std::hash::Hash for ContextHandleFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle)).hash(state);
  }
}

impl ContextHandleFunc {
  pub fn new(f: impl Fn(ContextHandle) + Send + Sync + 'static) -> Self {
    ContextHandleFunc(Arc::new(f))
  }

  pub fn run(&self, ctx: ContextHandle) {
    self.0(ctx)
  }
}

#[derive(Debug, Clone)]
pub struct Props {
  spawner: Option<SpawnFunc>,
  producer: Option<ProducerFunc>,
  mailbox_producer: Option<MailboxProduceFunc>,
  guardian_strategy: Option<SupervisorStrategyHandle>,
  supervisor_strategy: Option<SupervisorStrategyHandle>,
  dispatcher: Option<DispatcherHandle>,
  receiver_middleware: Vec<ReceiverMiddleware>,
  sender_middleware: Vec<SenderMiddleware>,
  spawn_middleware: Vec<SpawnMiddleware>,
  receiver_middleware_chain: Option<ReceiverFunc>,
  sender_middleware_chain: Option<SenderFunc>,
  spawn_middleware_chain: Option<SpawnFunc>,
  context_decorator: Vec<ContextDecorator>,
  context_decorator_chain: Option<ContextDecoratorFunc>,
  on_init: Vec<ContextHandleFunc>,
}

unsafe impl Send for Props {}
unsafe impl Sync for Props {}

static DEFAULT_DISPATCHER: Lazy<DispatcherHandle> =
  Lazy::new(|| DispatcherHandle::new(TokioRuntimeContextDispatcher::new().unwrap()));
static DEFAULT_MAILBOX_PRODUCER: Lazy<MailboxProduceFunc> = Lazy::new(|| unbounded_mailbox_creator_with_opts(vec![]));

static DEFAULT_SPAWNER: Lazy<SpawnFunc> = Lazy::new(|| {
  SpawnFunc::new(
    |actor_system: ActorSystem, name: String, props: Props, parent_context: SpawnerContextHandle| {
      async move {
        tracing::debug!("Spawn actor: {}", name);
        let mut ctx = ActorContext::new(actor_system.clone(), props.clone(), parent_context.get_self().await).await;
        let mut mb = props.produce_mailbox().await;
        // prepare the mailbox number counter

        let dp = props.get_dispatcher();
        let proc = ActorProcess::new(mb.clone());
        let proc_handle = ProcessHandle::new(proc);
        let pr = actor_system.get_process_registry().await;

        let (pid, absent) = pr.add_process(proc_handle, &name);
        if !absent {
          return Err(SpawnError::ErrNameExists(pid.clone()));
        }

        ctx.set_self(pid.clone()).await;

        initialize(props, ctx.clone());

        mb.register_handlers(
          Some(MessageInvokerHandle::new(Arc::new(Mutex::new(ctx.clone())))),
          Some(dp.clone()),
        )
        .await;
        tracing::debug!("mailbox handlers registered: {}", name);

        mb.post_system_message(MessageHandle::new(SystemMessage::Started(Started)))
          .await;
        tracing::debug!("post_system_message: started: {}", name);
        mb.start().await;
        tracing::debug!("mailbox started: {}", name);

        Ok(pid)
      }
    },
  )
});

fn initialize(props: Props, ctx: ActorContext) {
  for init in props.on_init {
    init.run(ContextHandle::new(ctx.clone()));
  }
}

#[derive(Debug, Clone)]
struct ReceiveFuncActor(ReceiveFunc);

#[async_trait]
impl Actor for ReceiveFuncActor {
  async fn handle(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    self.0.run(ctx).await
  }

  async fn receive(&mut self, _: ContextHandle, _: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[derive(Clone)]
pub struct PropsOptionFunc(Arc<Mutex<dyn FnMut(&mut Props) + Send + Sync + 'static>>);

impl PropsOptionFunc {
  pub fn new(f: impl FnMut(&mut Props) + Send + Sync + 'static) -> Self {
    PropsOptionFunc(Arc::new(Mutex::new(f)))
  }

  pub async fn run(&self, props: &mut Props) {
    let mut mg = self.0.lock().await;
    mg(props)
  }
}

impl Props {
  pub fn with_on_init(mut init: Vec<ContextHandleFunc>) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      props.on_init.append(&mut init);
    })
  }

  pub fn with_producer(producer: ProducerFunc) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      props.producer = Some(producer.clone());
    })
  }

  pub fn with_dispatcher(dispatcher: DispatcherHandle) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      props.dispatcher = Some(dispatcher.clone());
    })
  }

  pub fn with_mailbox(mailbox_produce_func: MailboxProduceFunc) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      props.mailbox_producer = Some(mailbox_produce_func.clone());
    })
  }

  pub fn with_context_decorator(decorators: Vec<ContextDecorator>) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      let cloned_decorators = decorators.clone();
      props.context_decorator.extend(cloned_decorators.clone());
      props.context_decorator_chain = make_context_decorator_chain(
        &props.context_decorator,
        ContextDecoratorFunc::new(move |ch| {
          let cloned_ch = ch.clone();
          async move { cloned_ch.clone() }
        }),
      );
    })
  }

  pub fn with_guardian(guardian: SupervisorStrategyHandle) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      props.guardian_strategy = Some(guardian.clone());
    })
  }

  pub fn with_supervisor(supervisor: SupervisorStrategyHandle) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      props.supervisor_strategy = Some(supervisor.clone());
    })
  }

  pub fn with_receiver_middleware(middlewares: Vec<ReceiverMiddleware>) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      tracing::debug!("with_receiver_middleware");
      props.receiver_middleware.extend(middlewares.clone());
      props.receiver_middleware_chain = make_receiver_middleware_chain(
        &props.receiver_middleware,
        ReceiverFunc::new(|mut rch, me| async move { rch.receive(me).await }),
      );
    })
  }

  pub fn with_sender_middleware(middlewares: Vec<SenderMiddleware>) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      props.sender_middleware.extend(middlewares.clone());
      props.sender_middleware_chain = make_sender_middleware_chain(
        &props.sender_middleware,
        SenderFunc::new(|sch, target, me| async move {
          target
            .send_user_message(sch.get_actor_system().await.clone(), MessageHandle::new(me))
            .await
        }),
      );
    })
  }

  pub fn with_spawn_func(spawn_func: SpawnFunc) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      props.spawner = Some(spawn_func.clone());
    })
  }

  pub fn with_receive_func(receive_func: ReceiveFunc) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      let receive_func = receive_func.clone();
      props.producer = Some(ProducerFunc::new(move |_| {
        let cloned = receive_func.clone();
        async move {
          let actor = ReceiveFuncActor(cloned.clone());
          ActorHandle::new(actor)
        }
      }));
    })
  }

  pub fn with_spawn_middleware(spawn_middlewares: Vec<SpawnMiddleware>) -> PropsOptionFunc {
    PropsOptionFunc::new(move |props: &mut Props| {
      props.spawn_middleware.extend(spawn_middlewares.clone());
      props.spawn_middleware_chain = make_spawn_middleware_chain(
        &props.spawn_middleware,
        SpawnFunc::new(move |s, id, p, sch| async move {
          if let Some(spawner) = &p.spawner {
            spawner.run(s, &id, p.clone(), sch).await
          } else {
            DEFAULT_SPAWNER.run(s, &id, p, sch).await
          }
        }),
      );
    })
  }

  // WithSpawnMiddleware

  fn get_spawner(&self) -> SpawnFunc {
    self.spawner.clone().unwrap_or(DEFAULT_SPAWNER.clone())
  }

  pub fn get_producer(&self) -> ProducerFunc {
    self.producer.clone().unwrap()
  }

  fn get_dispatcher(&self) -> DispatcherHandle {
    self.dispatcher.clone().unwrap_or_else(|| DEFAULT_DISPATCHER.clone())
  }

  pub fn get_supervisor_strategy(&self) -> SupervisorStrategyHandle {
    self
      .supervisor_strategy
      .clone()
      .unwrap_or_else(|| DEFAULT_SUPERVISION_STRATEGY.clone())
  }

  pub(crate) fn get_spawn_middleware_chain(&self) -> Option<SpawnFunc> {
    self.spawn_middleware_chain.clone()
  }

  pub(crate) fn get_guardian_strategy(&self) -> Option<SupervisorStrategyHandle> {
    self.guardian_strategy.clone()
  }

  pub(crate) fn get_sender_middleware_chain(&self) -> Option<SenderFunc> {
    self.sender_middleware_chain.clone()
  }

  pub(crate) fn get_receiver_middleware_chain(&self) -> Option<ReceiverFunc> {
    self.receiver_middleware_chain.clone()
  }

  pub(crate) fn get_context_decorator_chain(&self) -> Option<ContextDecoratorFunc> {
    self.context_decorator_chain.clone()
  }

  async fn produce_mailbox(&self) -> MailboxHandle {
    if let Some(mailbox_producer) = &self.mailbox_producer {
      mailbox_producer.run().await
    } else {
      DEFAULT_MAILBOX_PRODUCER.run().await
    }
  }

  pub async fn from_producer_func_with_opts(producer: ProducerFunc, opts: Vec<PropsOptionFunc>) -> Props {
    let mut props = Props {
      on_init: Vec::new(),
      producer: Some(producer),
      dispatcher: None,
      mailbox_producer: None,
      context_decorator: Vec::new(),
      guardian_strategy: None,
      supervisor_strategy: None,
      receiver_middleware: Vec::new(),
      sender_middleware: Vec::new(),
      spawner: None,
      spawn_middleware: Vec::new(),
      receiver_middleware_chain: None,
      sender_middleware_chain: None,
      spawn_middleware_chain: None,
      context_decorator_chain: None,
    };
    props.configure(&opts).await;
    props
  }

  pub async fn from_producer_func(producer: ProducerFunc) -> Props {
    Props::from_producer_func_with_opts(producer, vec![]).await
  }

  pub async fn from_receive_func_with_opts(f: ReceiveFunc, opts: Vec<PropsOptionFunc>) -> Props {
    let producer = ProducerFunc::new(move |_| {
      let cloned = f.clone();
      async move {
        let actor = ReceiveFuncActor(cloned);
        ActorHandle::new(actor)
      }
    });
    Props::from_producer_func_with_opts(producer, opts).await
  }

  pub async fn from_receive_func(f: ReceiveFunc) -> Props {
    Props::from_receive_func_with_opts(f, vec![]).await
  }

  pub async fn spawn(
    self,
    actor_system: ActorSystem,
    name: &str,
    parent_context: SpawnerContextHandle,
  ) -> Result<ExtendedPid, SpawnError> {
    self.get_spawner().run(actor_system, name, self, parent_context).await
  }

  async fn configure(&mut self, opts: &[PropsOptionFunc]) -> &mut Self {
    for opt in opts {
      opt.run(self).await;
    }
    self
  }
}
