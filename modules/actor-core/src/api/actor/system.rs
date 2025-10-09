use alloc::sync::Arc;
use core::convert::Infallible;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};

use super::root_context::RootContext;
use super::{ActorSystemHandles, ActorSystemParts, Spawn, Timer};
use crate::api::guardian::AlwaysRestart;
use crate::runtime::message::DynMessage;
use crate::runtime::system::InternalActorSystem;
use crate::ReceiveTimeoutSchedulerFactory;
use crate::{FailureEventListener, FailureEventStream, MailboxFactory, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError};

/// アクターシステムの主要なインスタンス。
///
/// アクターの生成、管理、メッセージディスパッチを担当する。
pub struct ActorSystem<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>, {
  inner: InternalActorSystem<DynMessage, R, Strat>,
  shutdown: ShutdownToken,
  _marker: PhantomData<U>,
}

/// アクターシステムの実行ランナー。
///
/// `ActorSystem`をラップし、非同期ランタイムで実行するためのインターフェースを提供する。
pub struct ActorSystemRunner<U, R, Strat = AlwaysRestart>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>, {
  system: ActorSystem<U, R, Strat>,
  _marker: PhantomData<U>,
}

impl<U, R> ActorSystem<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// メールボックスファクトリを指定して新しいアクターシステムを作成する。
  ///
  /// # 引数
  /// * `mailbox_factory` - メールボックスを生成するファクトリ
  pub fn new(mailbox_factory: R) -> Self {
    Self {
      inner: InternalActorSystem::new(mailbox_factory),
      shutdown: ShutdownToken::default(),
      _marker: PhantomData,
    }
  }

  /// 障害イベントリスナーを設定する。
  ///
  /// # 引数
  /// * `listener` - 障害イベントを受信するリスナー（オプション）
  pub fn set_failure_event_listener(&mut self, listener: Option<FailureEventListener>) {
    self.inner.set_root_event_listener(listener);
  }

  /// パーツからアクターシステムとハンドルを構築する。
  ///
  /// # 引数
  /// * `parts` - アクターシステムのパーツ
  ///
  /// # 戻り値
  /// `(ActorSystem, ActorSystemHandles)`のタプル
  pub fn from_parts<S, T, E>(parts: ActorSystemParts<R, S, T, E>) -> (Self, ActorSystemHandles<S, T, E>)
  where
    S: Spawn,
    T: Timer,
    E: FailureEventStream, {
    let (mailbox_factory, handles) = parts.split();
    let mut system = Self::new(mailbox_factory);
    system.set_failure_event_listener(Some(handles.event_stream.listener()));
    (system, handles)
  }
}

impl<U, R, Strat> ActorSystem<U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>,
{
  /// シャットダウントークンを取得する。
  ///
  /// # 戻り値
  /// シャットダウントークンのクローン
  pub fn shutdown_token(&self) -> ShutdownToken {
    self.shutdown.clone()
  }

  /// このシステムをランナーに変換する。
  ///
  /// ランナーは非同期ランタイムでの実行に適したインターフェースを提供する。
  ///
  /// # 戻り値
  /// アクターシステムランナー
  pub fn into_runner(self) -> ActorSystemRunner<U, R, Strat> {
    ActorSystemRunner {
      system: self,
      _marker: PhantomData,
    }
  }

  /// 受信タイムアウトスケジューラーファクトリを設定する。
  ///
  /// # 引数
  /// * `factory` - 受信タイムアウトスケジューラーを生成するファクトリ（オプション）
  pub fn set_receive_timeout_scheduler_factory(
    &mut self,
    factory: Option<Arc<dyn ReceiveTimeoutSchedulerFactory<DynMessage, R>>>,
  ) {
    self.inner.set_receive_timeout_factory(factory);
  }

  /// ルートコンテキストを取得する。
  ///
  /// ルートコンテキストはアクターシステムのトップレベルでアクターを生成するために使用される。
  ///
  /// # 戻り値
  /// ルートコンテキストへの可変参照
  pub fn root_context(&mut self) -> RootContext<'_, U, R, Strat> {
    RootContext {
      inner: self.inner.root_context(),
      _marker: PhantomData,
    }
  }

  /// 指定された条件が満たされるまでメッセージディスパッチを実行する。
  ///
  /// # 引数
  /// * `should_continue` - 継続条件を判定するクロージャ。`true`を返す限り実行を続ける
  ///
  /// # 戻り値
  /// 正常終了時は`Ok(())`、キューエラー時は`Err`
  pub async fn run_until<F>(&mut self, should_continue: F) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    F: FnMut() -> bool, {
    self.inner.run_until(should_continue).await
  }

  /// メッセージディスパッチを永続的に実行する。
  ///
  /// この関数は正常には終了しない。エラー発生時のみ返る。
  ///
  /// # 戻り値
  /// `Infallible`（正常終了しない）またはキューエラー
  pub async fn run_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.run_forever().await
  }

  /// 指定された条件が満たされるまでメッセージディスパッチをブロッキングで実行する。
  ///
  /// この関数は標準ライブラリが有効な場合のみ使用可能。
  ///
  /// # 引数
  /// * `should_continue` - 継続条件を判定するクロージャ。`true`を返す限り実行を続ける
  ///
  /// # 戻り値
  /// 正常終了時は`Ok(())`、キューエラー時は`Err`
  #[cfg(feature = "std")]
  pub fn blocking_dispatch_loop<F>(
    &mut self,
    should_continue: F,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    F: FnMut() -> bool, {
    self.inner.blocking_dispatch_loop(should_continue)
  }

  /// メッセージディスパッチをブロッキングで永続的に実行する。
  ///
  /// この関数は標準ライブラリが有効な場合のみ使用可能。正常には終了しない。
  ///
  /// # 戻り値
  /// `Infallible`（正常終了しない）またはキューエラー
  #[cfg(feature = "std")]
  pub fn blocking_dispatch_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.blocking_dispatch_forever()
  }

  /// 次のメッセージを1つディスパッチする。
  ///
  /// キューが空の場合は新しいメッセージが到着するまで待機する。
  ///
  /// # 戻り値
  /// 正常終了時は`Ok(())`、キューエラー時は`Err`
  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.dispatch_next().await
  }

  /// Ready キューに溜まったメッセージを同期的に処理し、空になるまで繰り返す。
  /// 新たにメッセージが到着するまで待機は行わない。
  pub fn run_until_idle(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let shutdown = self.shutdown.clone();
    self.inner.run_until_idle(|| !shutdown.is_triggered())
  }
}

impl<U, R, Strat> ActorSystemRunner<U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>,
{
  /// シャットダウントークンを取得する。
  ///
  /// # 戻り値
  /// シャットダウントークンのクローン
  pub fn shutdown_token(&self) -> ShutdownToken {
    self.system.shutdown.clone()
  }

  /// メッセージディスパッチを永続的に実行する。
  ///
  /// この関数は正常には終了しない。エラー発生時のみ返る。
  ///
  /// # 戻り値
  /// `Infallible`（正常終了しない）またはキューエラー
  pub async fn run_forever(mut self) -> Result<Infallible, QueueError<PriorityEnvelope<DynMessage>>> {
    self.system.run_forever().await
  }

  /// ランナーをFutureとして実行する。
  ///
  /// `run_forever`のエイリアス。非同期ランタイムでの実行に適した名前を提供する。
  ///
  /// # 戻り値
  /// `Infallible`（正常終了しない）またはキューエラー
  pub async fn into_future(self) -> Result<Infallible, QueueError<PriorityEnvelope<DynMessage>>> {
    self.run_forever().await
  }

  /// ランナーから内部のアクターシステムを取り出す。
  ///
  /// # 戻り値
  /// 内部のアクターシステム
  pub fn into_inner(self) -> ActorSystem<U, R, Strat> {
    self.system
  }
}

/// アクターシステムのシャットダウンを制御するトークン。
///
/// 複数のスレッドやタスクから共有でき、シャットダウン状態を協調的に管理する。
#[derive(Clone)]
pub struct ShutdownToken {
  inner: Arc<AtomicBool>,
}

impl ShutdownToken {
  /// 新しいシャットダウントークンを作成する。
  ///
  /// 初期状態ではシャットダウンはトリガーされていない。
  ///
  /// # 戻り値
  /// 新しいシャットダウントークン
  pub fn new() -> Self {
    Self {
      inner: Arc::new(AtomicBool::new(false)),
    }
  }

  /// シャットダウンをトリガーする。
  ///
  /// この操作は複数のスレッドから安全に呼び出せる。
  /// 一度トリガーされると、状態をリセットすることはできない。
  pub fn trigger(&self) {
    self.inner.store(true, Ordering::SeqCst);
  }

  /// シャットダウンがトリガーされているかどうかを確認する。
  ///
  /// # 戻り値
  /// シャットダウンがトリガーされている場合は`true`、そうでない場合は`false`
  pub fn is_triggered(&self) -> bool {
    self.inner.load(Ordering::SeqCst)
  }
}

impl Default for ShutdownToken {
  fn default() -> Self {
    Self::new()
  }
}
