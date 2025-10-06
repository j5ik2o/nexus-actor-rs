#[cfg(test)]
mod tests;

use std::any::type_name;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use once_cell::sync::Lazy;
use tokio::sync::{Mutex, RwLock};

use nexus_actor_core_rs::context::core::CoreSpawnAdapter;
use nexus_actor_core_rs::context::{CoreMailboxFactory, CoreProps};
use nexus_actor_core_rs::runtime::CoreRuntime;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::ActorContext;
use crate::actor::context::ContextHandle;
use crate::actor::context::SpawnerContextHandle;
use crate::actor::context::{InfoPart, ReceiverPart};
use crate::actor::core::actor::Actor;
use crate::actor::core::actor_error::ActorError;
use crate::actor::core::actor_handle::ActorHandle;
use crate::actor::core::actor_process::ActorProcess;
use crate::actor::core::actor_producer::ActorProducer;
use crate::actor::core::actor_receiver::ActorReceiver;
use crate::actor::core::context_decorator::ContextDecorator;
use crate::actor::core::context_decorator_chain::ContextDecoratorChain;
use crate::actor::core::context_handler::ContextHandler;
use crate::actor::core::middleware_chain::{
  make_context_decorator_chain, make_receiver_middleware_chain, make_sender_middleware_chain,
  make_spawn_middleware_chain,
};
use crate::actor::core::pid::ExtendedPid;
use crate::actor::core::receiver_middleware::ReceiverMiddleware;
use crate::actor::core::receiver_middleware_chain::ReceiverMiddlewareChain;
use crate::actor::core::sender_middleware::SenderMiddleware;
use crate::actor::core::sender_middleware_chain::SenderMiddlewareChain;
use crate::actor::core::spawn::adapter::StdSpawnAdapter;
use crate::actor::core::spawn_middleware::SpawnMiddleware;
use crate::actor::core::spawner::{SpawnError, Spawner};
use crate::actor::dispatch::unbounded_mailbox_creator_with_opts;
use crate::actor::dispatch::Mailbox;
use crate::actor::dispatch::MailboxProducer;
use crate::actor::dispatch::*;
use crate::actor::message::AutoReceiveMessage;
use crate::actor::message::MessageHandle;
use crate::actor::message::SystemMessage;
use crate::actor::process::ProcessHandle;
use crate::actor::supervisor::SupervisorStrategyHandle;
use crate::actor::supervisor::DEFAULT_SUPERVISION_STRATEGY;

#[derive(Debug, Clone)]
pub struct Props {
  spawner: Option<Spawner>,
  pub(crate) producer: Option<ActorProducer>,
  mailbox_producer: Option<MailboxProducer>,
  dispatcher: Option<DispatcherHandle>,
  guardian_strategy: Option<SupervisorStrategyHandle>,
  supervisor_strategy: Option<SupervisorStrategyHandle>,
  receiver_middleware: Vec<ReceiverMiddleware>,
  sender_middleware: Vec<SenderMiddleware>,
  spawn_middleware: Vec<SpawnMiddleware>,
  receiver_middleware_chain: Option<ReceiverMiddlewareChain>,
  sender_middleware_chain: Option<SenderMiddlewareChain>,
  spawn_middleware_chain: Option<Spawner>,
  context_decorator: Vec<ContextDecorator>,
  context_decorator_chain: Option<ContextDecoratorChain>,
  on_init: Vec<ContextHandler>,
  actor_type_hint: Option<Arc<str>>,
  core_props: CoreProps,
}

static_assertions::assert_impl_all!(Props: Send, Sync);

static DEFAULT_MAILBOX_PRODUCER: Lazy<MailboxProducer> = Lazy::new(|| unbounded_mailbox_creator_with_opts(vec![]));

static DEFAULT_SPAWNER: Lazy<Spawner> = Lazy::new(|| {
  Spawner::new(
    |actor_system: ActorSystem, name: String, props: Props, parent_context: SpawnerContextHandle| async move {
      let actor_type_label = props
        .actor_type_hint_str()
        .map(|s| s.to_owned())
        .unwrap_or_else(|| "unknown".to_string());
      let actor_type_label_ref = actor_type_label.as_str();
      tracing::debug!(actor_type = actor_type_label_ref, "Spawn actor: {}", name);
      let mut ctx = ActorContext::new(actor_system.clone(), props.clone(), parent_context.get_self_opt().await).await;
      let core_runtime = actor_system.core_runtime();
      let mut mb = props.produce_mailbox(&core_runtime).await;

      let dp = if let Some(dispatcher) = props.get_dispatcher() {
        dispatcher
      } else {
        DispatcherHandle::new_arc(actor_system.get_config().system_dispatcher.clone())
      };
      let proc = ActorProcess::new(mb.clone());
      let proc_handle = ProcessHandle::new(proc);
      let pr = actor_system.get_process_registry().await;

      let (pid, absent) = pr.add_process(proc_handle, &name).await;
      if !absent {
        return Err(SpawnError::ErrNameExists(pid.clone()));
      }

      ctx.set_self(pid.clone()).await;

      initialize(props, ctx.clone());

      let wants_metrics = ctx.metrics_sink().is_some();
      let mut mi = MessageInvokerHandle::new_with_metrics(Arc::new(RwLock::new(ctx.clone())), wants_metrics);

      mb.register_handlers(Some(mi.clone()), Some(dp.clone())).await;
      tracing::debug!(
        actor_type = actor_type_label_ref,
        "mailbox handlers registered: {}",
        name
      );

      let result = mi
        .invoke_user_message(MessageHandle::new(AutoReceiveMessage::PreStart))
        .await;

      if result.is_err() {
        return Err(SpawnError::ErrPreStart(result.err().unwrap()));
      }

      mb.post_system_message(MessageHandle::new(SystemMessage::Start)).await;
      tracing::debug!(
        actor_type = actor_type_label_ref,
        "post_system_message: started: {}",
        name
      );
      mb.start().await;
      tracing::debug!(actor_type = actor_type_label_ref, "mailbox started: {}", name);

      Ok(pid)
    },
  )
});

fn initialize(props: Props, ctx: ActorContext) {
  for init in props.on_init {
    init.run(ctx.context_handle());
  }
}

#[derive(Debug, Clone)]
pub struct ActorReceiverActor(ActorReceiver);

impl ActorReceiverActor {
  pub fn new(actor_receiver: ActorReceiver) -> Self {
    Self(actor_receiver)
  }
}

#[async_trait]
impl Actor for ActorReceiverActor {
  async fn handle(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    self.0.run(ctx).await
  }

  async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

type PropsOptionFn = Arc<Mutex<dyn FnMut(&mut Props) + Send + Sync + 'static>>;

#[derive(Clone)]
pub struct PropsOption(PropsOptionFn);

impl PropsOption {
  pub fn new(f: impl FnMut(&mut Props) + Send + Sync + 'static) -> Self {
    Self(Arc::new(Mutex::new(f)))
  }

  pub async fn run(&self, props: &mut Props) {
    let mut mg = self.0.lock().await;
    mg(props)
  }
}

impl Props {
  pub fn actor_type_hint_str(&self) -> Option<&str> {
    self.actor_type_hint.as_deref()
  }

  pub fn actor_type_hint(&self) -> Option<Arc<str>> {
    self.actor_type_hint.clone()
  }

  pub fn core_props(&self) -> CoreProps {
    self.core_props.clone()
  }

  pub fn with_on_init(mut init: Vec<ContextHandler>) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      props.on_init.append(&mut init);
    })
  }

  pub fn with_actor_producer(producer: ActorProducer) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      props.producer = Some(producer.clone());
    })
  }

  pub fn with_actor_receiver(actor_receiver: ActorReceiver) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      let actor_receiver = actor_receiver.clone();
      props.producer = Some(ActorProducer::from_handle(move |_| {
        let actor_receiver = actor_receiver.clone();
        async move {
          let actor = ActorReceiverActor(actor_receiver.clone());
          ActorHandle::new(actor)
        }
      }));
    })
  }

  pub fn with_mailbox_producer(mailbox_producer: MailboxProducer) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      props.mailbox_producer = Some(mailbox_producer.clone());
      props.rebuild_core_props();
    })
  }

  pub fn with_dispatcher(dispatcher: DispatcherHandle) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      props.dispatcher = Some(dispatcher.clone());
    })
  }

  pub fn with_context_decorators(decorators: impl IntoIterator<Item = ContextDecorator> + Send + Sync) -> PropsOption {
    let cloned_decorators = decorators.into_iter().collect::<Vec<_>>();
    PropsOption::new(move |props: &mut Props| {
      let cloned_decorators = cloned_decorators.clone();
      props.context_decorator.extend(cloned_decorators.clone());
      props.context_decorator_chain = make_context_decorator_chain(
        &props.context_decorator,
        ContextDecoratorChain::new(move |snapshot| {
          let handle = snapshot
            .into_context_handle()
            .expect("ContextSnapshot must carry ContextHandle for decorator tail");
          async move { handle }
        }),
      );
    })
  }

  pub fn with_guardian(guardian: SupervisorStrategyHandle) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      props.guardian_strategy = Some(guardian.clone());
    })
  }

  pub fn with_actor_type_hint(hint: &'static str) -> PropsOption {
    let hint_arc: Arc<str> = Arc::from(hint);
    PropsOption::new(move |props: &mut Props| {
      props.actor_type_hint = Some(hint_arc.clone());
      props.rebuild_core_props();
    })
  }

  pub fn with_supervisor_strategy(supervisor: SupervisorStrategyHandle) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      props.supervisor_strategy = Some(supervisor.clone());
      props.rebuild_core_props();
    })
  }

  pub fn with_receiver_middlewares(
    middlewares: impl IntoIterator<Item = ReceiverMiddleware> + Send + Sync,
  ) -> PropsOption {
    let middlewares = middlewares.into_iter().collect::<Vec<_>>();
    PropsOption::new(move |props: &mut Props| {
      props.receiver_middleware.extend(middlewares.clone());
      props.receiver_middleware_chain = make_receiver_middleware_chain(
        &props.receiver_middleware,
        ReceiverMiddlewareChain::new(|mut rch, me| async move { rch.receive(me).await }),
      );
    })
  }

  pub fn with_sender_middlewares(middlewares: impl IntoIterator<Item = SenderMiddleware> + Send + Sync) -> PropsOption {
    let middlewares = middlewares.into_iter().collect::<Vec<_>>();
    PropsOption::new(move |props: &mut Props| {
      props.sender_middleware.extend(middlewares.clone());
      props.sender_middleware_chain = make_sender_middleware_chain(
        &props.sender_middleware,
        SenderMiddlewareChain::new(|sch, target_core, me| async move {
          let target = ExtendedPid::from(target_core);
          target
            .send_user_message(sch.get_actor_system().await.clone(), MessageHandle::new(me))
            .await
        }),
      );
    })
  }

  pub fn with_spawner(spawner: Spawner) -> PropsOption {
    PropsOption::new(move |props: &mut Props| {
      props.spawner = Some(spawner.clone());
    })
  }

  pub fn with_spawn_middleware(
    spawn_middlewares: impl IntoIterator<Item = SpawnMiddleware> + Send + Sync,
  ) -> PropsOption {
    let spawn_middlewares = spawn_middlewares.into_iter().collect::<Vec<_>>();
    PropsOption::new(move |props: &mut Props| {
      props.spawn_middleware.extend(spawn_middlewares.clone());
      props.spawn_middleware_chain = make_spawn_middleware_chain(
        &props.spawn_middleware,
        Spawner::new(move |s, id, p, sch| async move {
          if let Some(spawner) = &p.spawner {
            spawner.run(s, &id, p.clone(), sch).await
          } else {
            DEFAULT_SPAWNER.run(s, &id, p, sch).await
          }
        }),
      );
    })
  }

  fn get_spawner(&self) -> Spawner {
    self.spawner.clone().unwrap_or(DEFAULT_SPAWNER.clone())
  }

  pub(crate) fn get_producer(&self) -> ActorProducer {
    self.producer.clone().unwrap()
  }

  pub(crate) fn get_supervisor_strategy(&self) -> SupervisorStrategyHandle {
    self
      .supervisor_strategy
      .clone()
      .unwrap_or_else(|| DEFAULT_SUPERVISION_STRATEGY.clone())
  }

  pub(crate) fn get_dispatcher(&self) -> Option<DispatcherHandle> {
    self.dispatcher.clone()
  }

  pub(crate) fn get_spawn_middleware_chain(&self) -> Option<Spawner> {
    self.spawn_middleware_chain.clone()
  }

  pub(crate) fn get_guardian_strategy(&self) -> Option<SupervisorStrategyHandle> {
    self.guardian_strategy.clone()
  }

  pub(crate) fn get_sender_middleware_chain(&self) -> Option<SenderMiddlewareChain> {
    self.sender_middleware_chain.clone()
  }

  pub(crate) fn get_receiver_middleware_chain(&self) -> Option<ReceiverMiddlewareChain> {
    self.receiver_middleware_chain.clone()
  }

  pub(crate) fn get_context_decorator_chain(&self) -> Option<ContextDecoratorChain> {
    self.context_decorator_chain.clone()
  }

  async fn produce_mailbox(&self, core_runtime: &CoreRuntime) -> MailboxHandle {
    let mut mailbox = if let Some(mailbox_producer) = &self.mailbox_producer {
      mailbox_producer.run().await
    } else {
      DEFAULT_MAILBOX_PRODUCER.run().await
    };

    mailbox.install_async_yielder(core_runtime.yielder()).await;
    mailbox
  }

  pub async fn from_async_actor_producer<A, F, Fut>(f: F) -> Props
  where
    A: Actor,
    F: Fn(ContextHandle) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = A> + Send + 'static, {
    Props::from_async_actor_producer_with_opts(f, [Props::with_actor_type_hint(type_name::<A>())]).await
  }

  pub async fn from_async_actor_producer_with_opts<A, F, Fut>(
    f: F,
    opts: impl IntoIterator<Item = PropsOption>,
  ) -> Props
  where
    A: Actor,
    F: Fn(ContextHandle) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = A> + Send + 'static, {
    let producer = ActorProducer::new(f);
    let mut opts = opts.into_iter().collect::<Vec<_>>();
    opts.push(Props::with_actor_type_hint(type_name::<A>()));
    let mut props = Props {
      on_init: vec![],
      producer: Some(producer),
      mailbox_producer: None,
      dispatcher: None,
      context_decorator: vec![],
      guardian_strategy: None,
      supervisor_strategy: None,
      receiver_middleware: vec![],
      sender_middleware: vec![],
      spawner: None,
      spawn_middleware: vec![],
      receiver_middleware_chain: None,
      sender_middleware_chain: None,
      spawn_middleware_chain: None,
      context_decorator_chain: None,
      actor_type_hint: None,
      core_props: CoreProps::new(),
    };
    props.configure(&opts).await;
    props
  }

  pub async fn from_async_actor_receiver_with_opts<F, Fut>(f: F, opts: impl IntoIterator<Item = PropsOption>) -> Props
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    let actor_receiver = ActorReceiver::new(f);
    let mut opts = opts.into_iter().collect::<Vec<_>>();
    opts.push(Props::with_actor_type_hint(type_name::<ActorReceiverActor>()));
    let producer = move |_| {
      let cloned = actor_receiver.clone();
      async move {
        let actor = ActorReceiverActor(cloned);
        ActorHandle::new(actor)
      }
    };
    Props::from_async_actor_producer_with_opts(producer, opts).await
  }

  pub async fn from_async_actor_receiver<F, Fut>(f: F) -> Props
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    Self::from_async_actor_receiver_with_opts(f, [Props::with_actor_type_hint(type_name::<ActorReceiverActor>())]).await
  }

  pub async fn from_sync_actor_producer_with_opts<A, F>(f: F, opts: impl IntoIterator<Item = PropsOption>) -> Props
  where
    A: Actor,
    F: Fn(ContextHandle) -> A + Clone + Send + Sync + 'static, {
    let f_arc = Arc::new(f);
    Self::from_async_actor_producer_with_opts(
      move |ctx| {
        let cloned_f_arc = f_arc.clone();
        async move { (*cloned_f_arc)(ctx.clone()) }
      },
      opts
        .into_iter()
        .chain([Props::with_actor_type_hint(type_name::<A>())])
        .collect::<Vec<_>>(),
    )
    .await
  }

  pub async fn from_sync_actor_producer<A, F>(f: F) -> Props
  where
    A: Actor,
    F: Fn(ContextHandle) -> A + Clone + Send + Sync + 'static, {
    Self::from_sync_actor_producer_with_opts(f, [Props::with_actor_type_hint(type_name::<A>())]).await
  }

  pub async fn from_sync_actor_receiver_with_opts<F>(f: F, opts: impl IntoIterator<Item = PropsOption>) -> Props
  where
    F: Fn(ContextHandle) -> Result<(), ActorError> + Send + Sync + 'static, {
    let f_arc = Arc::new(f);
    Self::from_async_actor_receiver_with_opts(
      move |ctx| {
        let cloned_f_arc = f_arc.clone();
        async move { (*cloned_f_arc)(ctx.clone()) }
      },
      opts
        .into_iter()
        .chain([Props::with_actor_type_hint(type_name::<ActorReceiverActor>())])
        .collect::<Vec<_>>(),
    )
    .await
  }

  pub async fn from_sync_actor_receiver<F>(f: F) -> Props
  where
    F: Fn(ContextHandle) -> Result<(), ActorError> + Send + Sync + 'static, {
    Self::from_sync_actor_receiver_with_opts(f, [Props::with_actor_type_hint(type_name::<ActorReceiverActor>())]).await
  }

  pub(crate) async fn spawn(
    self,
    actor_system: ActorSystem,
    name: &str,
    parent_context: SpawnerContextHandle,
  ) -> Result<ExtendedPid, SpawnError> {
    self.get_spawner().run(actor_system, name, self, parent_context).await
  }

  async fn configure(&mut self, opts: &[PropsOption]) -> &mut Self {
    for opt in opts {
      opt.run(self).await;
    }
    self.rebuild_core_props();
    self
  }

  fn rebuild_core_props(&mut self) {
    let mailbox_producer = self
      .mailbox_producer
      .clone()
      .unwrap_or_else(|| DEFAULT_MAILBOX_PRODUCER.clone());
    let mailbox_factory: CoreMailboxFactory = {
      let producer = mailbox_producer.clone();
      Arc::new(move || {
        let producer = producer.clone();
        Box::pin(async move {
          let handle = producer.run().await;
          Arc::new(handle) as Arc<dyn nexus_actor_core_rs::actor::core_types::mailbox::CoreMailbox + Send + Sync>
        })
      })
    };

    let supervisor_strategy = self.get_supervisor_strategy().core_strategy();

    let mut core_props = CoreProps::new()
      .with_actor_type(self.actor_type_hint.clone())
      .with_mailbox_factory(mailbox_factory)
      .with_supervisor_strategy(supervisor_strategy);

    if let Some(chain) = &self.receiver_middleware_chain {
      core_props = core_props.with_receiver_middleware_chain(chain.to_core_invocation_chain());
    }

    if let Some(chain) = &self.sender_middleware_chain {
      core_props = core_props.with_sender_middleware_chain(chain.to_core_invocation_chain());
    }

    let effective_spawner = self
      .spawn_middleware_chain
      .clone()
      .unwrap_or_else(|| self.get_spawner());

    let adapter = Arc::new(StdSpawnAdapter::new(self.clone(), effective_spawner));
    let spawn_chain = StdSpawnAdapter::to_core_chain(adapter.clone());
    let adapter_trait: Arc<dyn CoreSpawnAdapter> = adapter;

    core_props = core_props
      .with_spawn_middleware_chain(spawn_chain)
      .with_spawn_adapter(adapter_trait);

    self.core_props = core_props;
  }

  pub fn from_core_props(core_props: CoreProps) -> Option<Props> {
    if let Some(adapter) = core_props.spawn_adapter() {
      if let Some(std_adapter) = adapter.as_any().downcast_ref::<StdSpawnAdapter>() {
        let mut props = std_adapter.props();
        props.core_props = core_props;
        return Some(props);
      }
    }
    None
  }
}
