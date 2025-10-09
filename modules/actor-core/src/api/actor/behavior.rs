use alloc::boxed::Box;
use alloc::sync::Arc;

use crate::api::supervision::{NoopSupervisor, Supervisor, SupervisorDirective};
use crate::api::MessageEnvelope;
use crate::runtime::context::MapSystemFn;
use crate::runtime::message::DynMessage;
use crate::MailboxFactory;
use crate::PriorityEnvelope;
use crate::SystemMessage;
use nexus_utils_core_rs::Element;

use super::Context;

type ReceiveFn<U, R> = dyn for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) -> BehaviorDirective<U, R> + 'static;
type SystemHandlerFn<U, R> = dyn for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static;
type SignalFn<U, R> = dyn for<'r, 'ctx> Fn(&mut Context<'r, 'ctx, U, R>, Signal) -> BehaviorDirective<U, R> + 'static;
type SetupFn<U, R> = dyn for<'r, 'ctx> Fn(&mut Context<'r, 'ctx, U, R>) -> Behavior<U, R> + 'static;

/// アクターのライフサイクルシグナル。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
  /// アクター停止後に送信されるシグナル
  PostStop,
}

/// スーパーバイザー戦略の設定（内部表現）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SupervisorStrategyConfig {
  /// デフォルト戦略（NoopSupervisor）
  Default,
  /// 固定戦略
  Fixed(SupervisorStrategy),
}

impl SupervisorStrategyConfig {
  pub(crate) fn default() -> Self {
    SupervisorStrategyConfig::Default
  }

  pub(crate) fn from_strategy(strategy: SupervisorStrategy) -> Self {
    SupervisorStrategyConfig::Fixed(strategy)
  }

  pub(crate) fn into_supervisor<M>(&self) -> DynSupervisor<M>
  where
    M: Element,
  {
    let inner: Box<dyn Supervisor<M>> = match self {
      SupervisorStrategyConfig::Default => Box::new(NoopSupervisor),
      SupervisorStrategyConfig::Fixed(strategy) => Box::new(FixedDirectiveSupervisor::new(*strategy)),
    };
    DynSupervisor::new(inner)
  }
}

/// スーパーバイザー戦略の種類。
///
/// 子アクターで障害が発生した際に、親アクターがどのように対処するかを定義する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorStrategy {
  /// アクターを再起動する
  Restart,
  /// アクターを停止する
  Stop,
  /// エラーを無視して処理を継続する
  Resume,
  /// 親へエスカレーションする
  Escalate,
}

impl From<SupervisorStrategy> for SupervisorDirective {
  fn from(value: SupervisorStrategy) -> Self {
    match value {
      SupervisorStrategy::Restart => SupervisorDirective::Restart,
      SupervisorStrategy::Stop => SupervisorDirective::Stop,
      SupervisorStrategy::Resume => SupervisorDirective::Resume,
      SupervisorStrategy::Escalate => SupervisorDirective::Escalate,
    }
  }
}

struct FixedDirectiveSupervisor {
  directive: SupervisorDirective,
}

impl FixedDirectiveSupervisor {
  fn new(strategy: SupervisorStrategy) -> Self {
    Self {
      directive: strategy.into(),
    }
  }
}

impl<M> Supervisor<M> for FixedDirectiveSupervisor {
  fn decide(&mut self, _error: &dyn core::fmt::Debug) -> SupervisorDirective {
    self.directive
  }
}

/// 動的なスーパーバイザー実装（内部型）。
pub(crate) struct DynSupervisor<M>
where
  M: Element,
{
  inner: Box<dyn Supervisor<M>>,
}

impl<M> DynSupervisor<M>
where
  M: Element,
{
  fn new(inner: Box<dyn Supervisor<M>>) -> Self {
    Self { inner }
  }
}

impl<M> Supervisor<M> for DynSupervisor<M>
where
  M: Element,
{
  fn before_handle(&mut self) {
    self.inner.before_handle();
  }

  fn after_handle(&mut self) {
    self.inner.after_handle();
  }

  fn decide(&mut self, error: &dyn core::fmt::Debug) -> SupervisorDirective {
    self.inner.decide(error)
  }
}

/// ユーザーメッセージ処理後の状態遷移指示。
///
/// メッセージ処理後に次の動作を指定する。
pub enum BehaviorDirective<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// 同じBehaviorを維持する
  Same,
  /// 新しいBehaviorへ遷移する
  Become(Behavior<U, R>),
}

/// Behaviorの内部状態を保持する構造体。
pub struct BehaviorState<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  handler: Box<ReceiveFn<U, R>>,
  supervisor: SupervisorStrategyConfig,
  signal_handler: Option<Arc<SignalFn<U, R>>>,
}

impl<U, R> BehaviorState<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  fn new(handler: Box<ReceiveFn<U, R>>, supervisor: SupervisorStrategyConfig) -> Self {
    Self {
      handler,
      supervisor,
      signal_handler: None,
    }
  }

  fn signal_handler(&self) -> Option<Arc<SignalFn<U, R>>> {
    self.signal_handler.clone()
  }

  fn set_signal_handler(&mut self, handler: Arc<SignalFn<U, R>>) {
    self.signal_handler = Some(handler);
  }
}

/// Typed Behavior 表現。Akka/Pekko Typed の `Behavior` に相当する。
///
/// アクターの振る舞いを定義する。メッセージ受信時の処理や
/// ライフサイクルイベントへの反応を記述する。
pub enum Behavior<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// メッセージ受信状態
  Receive(BehaviorState<U, R>),
  /// セットアップ処理を実行してBehaviorを生成
  Setup(Arc<SetupFn<U, R>>, Option<Arc<SignalFn<U, R>>>),
  /// 停止状態
  Stopped,
}

impl<U, R> Behavior<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// メッセージ受信ハンドラを指定して`Behavior`を構築する。
  ///
  /// # Arguments
  /// * `handler` - メッセージを受信した際の処理
  pub fn receive<F>(handler: F) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) -> BehaviorDirective<U, R> + 'static,
  {
    Self::Receive(BehaviorState::new(
      Box::new(handler),
      SupervisorStrategyConfig::default(),
    ))
  }

  /// 状態を持たないシンプルなハンドラでBehaviorを構築する。
  ///
  /// ハンドラは常に`BehaviorDirective::Same`を返す。
  ///
  /// # Arguments
  /// * `handler` - メッセージを受信した際の処理
  pub fn stateless<F>(mut handler: F) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static,
  {
    Self::Receive(BehaviorState::new(
      Box::new(move |ctx, msg| {
        handler(ctx, msg);
        BehaviorDirective::Same
      }),
      SupervisorStrategyConfig::default(),
    ))
  }

  /// Contextを使わず、メッセージのみを受け取るハンドラでBehaviorを構築する。
  ///
  /// # Arguments
  /// * `handler` - メッセージを受信した際の処理
  pub fn receive_message<F>(mut handler: F) -> Self
  where
    F: FnMut(U) -> BehaviorDirective<U, R> + 'static,
  {
    Self::receive(move |_, msg| handler(msg))
  }

  /// 停止状態のBehaviorを生成する。
  pub fn stopped() -> Self {
    Self::Stopped
  }

  /// セットアップ処理を実行してBehaviorを生成する。
  ///
  /// # Arguments
  /// * `init` - 初期化処理。Contextを受け取ってBehaviorを返す
  pub fn setup<F>(init: F) -> Self
  where
    F: for<'r, 'ctx> Fn(&mut Context<'r, 'ctx, U, R>) -> Behavior<U, R> + 'static,
  {
    Self::Setup(Arc::new(init), None)
  }

  /// スーパーバイザー設定を取得する（内部API）。
  pub(crate) fn supervisor_config(&self) -> SupervisorStrategyConfig {
    match self {
      Behavior::Receive(state) => state.supervisor.clone(),
      Behavior::Setup(_, _) | Behavior::Stopped => SupervisorStrategyConfig::default(),
    }
  }

  /// シグナルハンドラを追加する。
  ///
  /// # Arguments
  /// * `handler` - シグナル受信時の処理
  pub fn receive_signal<F>(self, handler: F) -> Self
  where
    F: for<'r, 'ctx> Fn(&mut Context<'r, 'ctx, U, R>, Signal) -> BehaviorDirective<U, R> + 'static,
  {
    let handler = Arc::new(handler) as Arc<SignalFn<U, R>>;
    self.attach_signal_arc(Some(handler))
  }

  fn attach_signal_arc(mut self, handler: Option<Arc<SignalFn<U, R>>>) -> Self {
    if let Some(handler) = handler {
      match &mut self {
        Behavior::Receive(state) => {
          state.set_signal_handler(handler);
        }
        Behavior::Setup(_, slot) => {
          *slot = Some(handler);
        }
        Behavior::Stopped => {}
      }
    }
    self
  }
}

/// Behavior DSL ビルダー。
///
/// Akka Typed風のBehavior構築APIを提供する。
pub struct Behaviors;

impl Behaviors {
  /// メッセージ受信ハンドラを指定してBehaviorを構築する。
  pub fn receive<U, R, F>(handler: F) -> Behavior<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone,
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) -> BehaviorDirective<U, R> + 'static,
  {
    Behavior::receive(handler)
  }

  /// 現在のBehaviorを維持する指示を返す。
  pub fn same<U, R>() -> BehaviorDirective<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone,
  {
    BehaviorDirective::Same
  }

  /// メッセージのみを受け取るハンドラでBehaviorを構築する。
  pub fn receive_message<U, R, F>(handler: F) -> Behavior<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone,
    F: FnMut(U) -> BehaviorDirective<U, R> + 'static,
  {
    Behavior::receive_message(handler)
  }

  /// 新しいBehaviorへ遷移する指示を返す。
  pub fn transition<U, R>(behavior: Behavior<U, R>) -> BehaviorDirective<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone,
  {
    BehaviorDirective::Become(behavior)
  }

  /// 停止状態へ遷移する指示を返す。
  pub fn stopped<U, R>() -> BehaviorDirective<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone,
  {
    BehaviorDirective::Become(Behavior::stopped())
  }

  /// Behaviorにスーパーバイザー戦略を設定するビルダーを生成する。
  pub fn supervise<U, R>(behavior: Behavior<U, R>) -> SuperviseBuilder<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone,
  {
    SuperviseBuilder { behavior }
  }

  /// セットアップ処理を実行してBehaviorを生成する。
  pub fn setup<U, R, F>(init: F) -> Behavior<U, R>
  where
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone,
    R::Signal: Clone,
    F: for<'r, 'ctx> Fn(&mut Context<'r, 'ctx, U, R>) -> Behavior<U, R> + 'static,
  {
    Behavior::setup(init)
  }
}

/// スーパーバイザー戦略を設定するビルダー。
pub struct SuperviseBuilder<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  behavior: Behavior<U, R>,
}

impl<U, R> SuperviseBuilder<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// スーパーバイザー戦略を設定する。
  ///
  /// # Arguments
  /// * `strategy` - 適用するスーパーバイザー戦略
  pub fn with_strategy(mut self, strategy: SupervisorStrategy) -> Behavior<U, R> {
    if let Behavior::Receive(state) = &mut self.behavior {
      state.supervisor = SupervisorStrategyConfig::from_strategy(strategy);
    }
    self.behavior
  }
}

/// Behavior を非Typedランタイムへ橋渡しするアダプタ。
///
/// 内部ランタイムで使用される、Behaviorとメッセージディスパッチを結びつける型。
pub struct ActorAdapter<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  behavior_factory: Arc<dyn Fn() -> Behavior<U, R> + 'static>,
  pub(super) behavior: Behavior<U, R>,
  pub(super) system_handler: Option<Box<SystemHandlerFn<U, R>>>,
}

impl<U, R> ActorAdapter<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// 新しい`ActorAdapter`を生成する。
  ///
  /// # Arguments
  /// * `behavior_factory` - Behaviorを生成するファクトリ関数
  /// * `system_handler` - システムメッセージのハンドラ（オプション）
  pub fn new<S>(behavior_factory: Arc<dyn Fn() -> Behavior<U, R> + 'static>, system_handler: Option<S>) -> Self
  where
    S: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static,
  {
    let behavior = behavior_factory();
    Self {
      behavior_factory,
      behavior,
      system_handler: system_handler.map(|h| Box::new(h) as Box<SystemHandlerFn<U, R>>),
    }
  }

  /// ユーザーメッセージを処理する。
  ///
  /// # Arguments
  /// * `ctx` - アクターコンテキスト
  /// * `message` - 処理するメッセージ
  pub fn handle_user(&mut self, ctx: &mut Context<'_, '_, U, R>, message: U) {
    self.ensure_initialized(ctx);
    match &mut self.behavior {
      Behavior::Receive(state) => {
        let handler = state.handler.as_mut();
        match handler(ctx, message) {
          BehaviorDirective::Same => {}
          BehaviorDirective::Become(next) => self.transition(next, ctx),
        }
      }
      Behavior::Stopped => {
        // ignore further user messages
      }
      Behavior::Setup(_, _) => unreachable!(),
    }
  }

  /// システムメッセージを処理する。
  ///
  /// # Arguments
  /// * `ctx` - アクターコンテキスト
  /// * `message` - 処理するシステムメッセージ
  pub fn handle_system(&mut self, ctx: &mut Context<'_, '_, U, R>, message: SystemMessage) {
    self.ensure_initialized(ctx);
    if matches!(message, SystemMessage::Stop) {
      self.transition(Behavior::stopped(), ctx);
    } else if matches!(message, SystemMessage::Restart) {
      self.behavior = (self.behavior_factory)();
      self.ensure_initialized(ctx);
    }
    if let Some(handler) = self.system_handler.as_mut() {
      handler(ctx, message);
    }
  }

  /// Guardian/Scheduler用のSystemMessageマッパーを生成する。
  pub fn create_map_system() -> Arc<MapSystemFn<DynMessage>> {
    Arc::new(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)))
  }

  /// スーパーバイザー設定を取得する（内部API）。
  pub(crate) fn supervisor_config(&self) -> SupervisorStrategyConfig {
    self.behavior.supervisor_config()
  }

  fn ensure_initialized(&mut self, ctx: &mut Context<'_, '_, U, R>) {
    loop {
      match &self.behavior {
        Behavior::Setup(init, signal_slot) => {
          let init = init.clone();
          let signal = signal_slot.clone();
          let behavior = init(ctx);
          self.behavior = behavior.attach_signal_arc(signal);
        }
        _ => break,
      }
    }
  }

  fn current_signal_handler(&self) -> Option<Arc<SignalFn<U, R>>> {
    match &self.behavior {
      Behavior::Receive(state) => state.signal_handler(),
      Behavior::Setup(_, handler) => handler.clone(),
      Behavior::Stopped => None,
    }
  }

  #[allow(dead_code)]
  fn handle_signal(&mut self, ctx: &mut Context<'_, '_, U, R>, signal: Signal) {
    if let Some(handler) = self.current_signal_handler() {
      match handler(ctx, signal) {
        BehaviorDirective::Same => {}
        BehaviorDirective::Become(next) => self.transition(next, ctx),
      }
    }
  }

  fn transition(&mut self, next: Behavior<U, R>, ctx: &mut Context<'_, '_, U, R>) {
    let previous_handler = self.current_signal_handler();
    self.behavior = next;
    self.ensure_initialized(ctx);
    if matches!(self.behavior, Behavior::Stopped) {
      let mut handler = self.current_signal_handler();
      if handler.is_none() {
        handler = previous_handler;
      }
      if let Some(handler) = handler {
        match handler(ctx, Signal::PostStop) {
          BehaviorDirective::Same => {}
          BehaviorDirective::Become(next) => self.transition(next, ctx),
        }
      }
    }
  }
}
