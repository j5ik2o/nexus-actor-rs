use crate::runtime::context::ActorContext;
use crate::runtime::message::DynMessage;
use crate::ActorId;
use crate::ActorPath;
use crate::MailboxFactory;
use crate::PriorityEnvelope;
use crate::Supervisor;
use crate::SystemMessage;
use alloc::{boxed::Box, string::String, sync::Arc};
use core::future::Future;
use core::marker::PhantomData;
use core::time::Duration;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use super::{
  ask::create_ask_handles, ask_with_timeout, ActorRef, AskError, AskFuture, AskResult, AskTimeoutFuture, Props,
};
use crate::api::{MessageEnvelope, MessageMetadata, MessageSender};

/// Typed actor execution context wrapper.
/// 'r: lifetime of the mutable reference to ActorContext
/// 'ctx: lifetime parameter of ActorContext itself
pub struct Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>,
  metadata: Option<MessageMetadata>,
  _marker: PhantomData<U>,
}

/// セットアップ時のコンテキスト型エイリアス。
pub type SetupContext<'ctx, U, R> = Context<'ctx, 'ctx, U, R>;

/// コンテキストログのレベル。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextLogLevel {
  /// トレースレベル
  Trace,
  /// デバッグレベル
  Debug,
  /// 情報レベル
  Info,
  /// 警告レベル
  Warn,
  /// エラーレベル
  Error,
}

/// アクターのログ出力を管理する構造体。
#[derive(Clone)]
pub struct ContextLogger {
  actor_id: ActorId,
  actor_path: ActorPath,
}

impl ContextLogger {
  pub(crate) fn new(actor_id: ActorId, actor_path: &ActorPath) -> Self {
    Self {
      actor_id,
      actor_path: actor_path.clone(),
    }
  }

  /// ログ出力元のアクターIDを取得する。
  pub fn actor_id(&self) -> ActorId {
    self.actor_id
  }

  /// ログ出力元のアクターパスを取得する。
  pub fn actor_path(&self) -> &ActorPath {
    &self.actor_path
  }

  /// トレースレベルのログを出力する。
  pub fn trace<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Trace, message);
  }

  /// デバッグレベルのログを出力する。
  pub fn debug<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Debug, message);
  }

  /// 情報レベルのログを出力する。
  pub fn info<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Info, message);
  }

  /// 警告レベルのログを出力する。
  pub fn warn<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Warn, message);
  }

  /// エラーレベルのログを出力する。
  pub fn error<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Error, message);
  }

  fn emit<F>(&self, level: ContextLogLevel, message: F)
  where
    F: FnOnce() -> String, {
    #[cfg(feature = "tracing")]
    {
      let text = message();
      match level {
        ContextLogLevel::Trace => tracing::event!(
          target: "nexus::actor",
          tracing::Level::TRACE,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Debug => tracing::event!(
          target: "nexus::actor",
          tracing::Level::DEBUG,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Info => tracing::event!(
          target: "nexus::actor",
          tracing::Level::INFO,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Warn => tracing::event!(
          target: "nexus::actor",
          tracing::Level::WARN,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Error => tracing::event!(
          target: "nexus::actor",
          tracing::Level::ERROR,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
      }
    }

    #[cfg(not(feature = "tracing"))]
    {
      let _ = level;
      let _ = message;
    }
  }
}

impl<'r, 'ctx, U, R> Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub(super) fn new(inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>) -> Self {
    Self {
      inner,
      metadata: None,
      _marker: PhantomData,
    }
  }

  /// 現在のメッセージに付随するメタデータを取得する。
  ///
  /// # 戻り値
  /// メタデータが存在する場合は`Some(&MessageMetadata)`、存在しない場合は`None`
  pub fn message_metadata(&self) -> Option<&MessageMetadata> {
    self.metadata.as_ref()
  }

  pub(super) fn with_metadata(
    inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>,
    metadata: MessageMetadata,
  ) -> Self {
    Self {
      inner,
      metadata: Some(metadata),
      _marker: PhantomData,
    }
  }

  /// このアクターのアクターIDを取得する。
  ///
  /// # 戻り値
  /// アクターID
  pub fn actor_id(&self) -> ActorId {
    self.inner.actor_id()
  }

  /// このアクターのアクターパスを取得する。
  ///
  /// # 戻り値
  /// アクターパスへの参照
  pub fn actor_path(&self) -> &ActorPath {
    self.inner.actor_path()
  }

  /// このアクターを監視しているアクターのIDリストを取得する。
  ///
  /// # 戻り値
  /// 監視者のアクターIDスライス
  pub fn watchers(&self) -> &[ActorId] {
    self.inner.watchers()
  }

  /// このアクター用のロガーを取得する。
  ///
  /// # 戻り値
  /// コンテキストロガー
  pub fn log(&self) -> ContextLogger {
    ContextLogger::new(self.actor_id(), self.actor_path())
  }

  /// 自分自身にメッセージを送信する。
  ///
  /// # 引数
  /// * `message` - 送信するメッセージ
  ///
  /// # 戻り値
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn send_to_self(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(MessageEnvelope::user(message));
    self.inner.send_to_self_with_priority(dyn_message, DEFAULT_PRIORITY)
  }

  /// 自分自身にシステムメッセージを送信する。
  ///
  /// # 引数
  /// * `message` - 送信するシステムメッセージ
  ///
  /// # 戻り値
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn send_system_to_self(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let envelope =
      PriorityEnvelope::from_system(message.clone()).map(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)));
    self.inner.send_envelope_to_self(envelope)
  }

  /// 自分自身への参照を取得する。
  ///
  /// # 戻り値
  /// 自分自身への`ActorRef`
  pub fn self_ref(&self) -> ActorRef<U, R> {
    ActorRef::new(self.inner.self_ref())
  }

  /// 外部メッセージ型を内部メッセージ型に変換するアダプターを作成する。
  ///
  /// # 引数
  /// * `f` - メッセージ変換関数
  ///
  /// # 戻り値
  /// メッセージアダプター参照
  pub fn message_adapter<Ext, F>(&self, f: F) -> MessageAdapterRef<Ext, U, R>
  where
    Ext: Element,
    F: Fn(Ext) -> U + Send + Sync + 'static, {
    MessageAdapterRef::new(self.self_ref(), Arc::new(f))
  }

  /// 監視者を登録する。
  ///
  /// # 引数
  /// * `watcher` - 監視者のアクターID
  pub fn register_watcher(&mut self, watcher: ActorId) {
    self.inner.register_watcher(watcher);
  }

  /// 監視者の登録を解除する。
  ///
  /// # 引数
  /// * `watcher` - 監視者のアクターID
  pub fn unregister_watcher(&mut self, watcher: ActorId) {
    self.inner.unregister_watcher(watcher);
  }

  /// 受信タイムアウトのサポートがあるかを判定する。
  ///
  /// # 戻り値
  /// サポートされている場合は`true`、それ以外は`false`
  pub fn has_receive_timeout_support(&self) -> bool {
    self.inner.has_receive_timeout_scheduler()
  }

  /// 受信タイムアウトを設定する。
  ///
  /// # 引数
  /// * `duration` - タイムアウト時間
  ///
  /// # 戻り値
  /// 設定に成功した場合は`true`、それ以外は`false`
  pub fn set_receive_timeout(&mut self, duration: Duration) -> bool {
    self.inner.set_receive_timeout(duration)
  }

  /// 受信タイムアウトをキャンセルする。
  ///
  /// # 戻り値
  /// キャンセルに成功した場合は`true`、それ以外は`false`
  pub fn cancel_receive_timeout(&mut self) -> bool {
    self.inner.cancel_receive_timeout()
  }

  /// 内部コンテキストへの可変参照を取得する。
  ///
  /// # 戻り値
  /// 内部`ActorContext`への可変参照
  pub fn inner(&mut self) -> &mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>> {
    self.inner
  }

  pub(crate) fn self_dispatcher(&self) -> MessageSender<U>
  where
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    self.self_ref().to_dispatcher()
  }

  /// 送信者情報付きでメッセージをリクエストする。
  ///
  /// 自分自身を送信者として設定してメッセージを送信する。
  ///
  /// # 引数
  /// * `target` - メッセージの送信先アクター
  /// * `message` - 送信するメッセージ
  ///
  /// # 戻り値
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn request<V>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    V: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let metadata = MessageMetadata::new().with_sender(self.self_dispatcher());
    target.tell_with_metadata(message, metadata)
  }

  /// 指定した送信者情報付きでメッセージをリクエストする。
  ///
  /// # 引数
  /// * `target` - メッセージの送信先アクター
  /// * `message` - 送信するメッセージ
  /// * `sender` - 送信者として設定するアクター
  ///
  /// # 戻り値
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn request_with_sender<V, S>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
    sender: &ActorRef<S, R>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    V: Element,
    S: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let metadata = MessageMetadata::new().with_sender(sender.to_dispatcher());
    target.tell_with_metadata(message, metadata)
  }

  /// 元のメタデータを保持したままメッセージを転送する。
  ///
  /// 現在のメッセージのメタデータ（送信者情報）をそのまま使用してメッセージを転送する。
  ///
  /// # 引数
  /// * `target` - メッセージの転送先アクター
  /// * `message` - 転送するメッセージ
  ///
  /// # 戻り値
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn forward<V>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    V: Element, {
    let metadata = self.message_metadata().cloned().unwrap_or_default();
    target.tell_with_metadata(message, metadata)
  }

  /// 現在のメッセージの送信者に応答する。
  ///
  /// # 引数
  /// * `message` - 応答メッセージ
  ///
  /// # 戻り値
  /// 送信成功時は`Ok(())`、失敗時は`AskError`
  ///
  /// # エラー
  /// - `AskError::MissingResponder` - 応答先が見つからない場合
  /// - `AskError::SendFailed` - メッセージ送信に失敗した場合
  pub fn respond<Resp>(&mut self, message: Resp) -> AskResult<()>
  where
    Resp: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let metadata = self.message_metadata().cloned().ok_or(AskError::MissingResponder)?;
    metadata.respond_with(self, message)
  }

  /// ターゲットアクターに問い合わせを行い、応答を待つFutureを返す。
  ///
  /// メッセージファクトリを使用してレスポンダーを含むメッセージを構築する。
  ///
  /// # 引数
  /// * `target` - 問い合わせ先アクター
  /// * `factory` - レスポンダーを使用してメッセージを生成する関数
  ///
  /// # 戻り値
  /// 応答を受け取るための`AskFuture`、またはエラー
  pub fn ask<V, Resp, F>(&mut self, target: &ActorRef<V, R>, factory: F) -> AskResult<AskFuture<Resp>>
  where
    V: Element,
    Resp: Element,
    F: FnOnce(MessageSender<Resp>) -> V,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let (future, responder) = create_ask_handles::<Resp>();
    let responder_for_message = MessageSender::new(responder.internal());
    let message = factory(responder_for_message);
    let metadata = MessageMetadata::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    target.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// タイムアウト付きで問い合わせを行い、応答を待つFutureを返す。
  ///
  /// # 引数
  /// * `target` - 問い合わせ先アクター
  /// * `factory` - レスポンダーを使用してメッセージを生成する関数
  /// * `timeout` - タイムアウト制御用のFuture
  ///
  /// # 戻り値
  /// タイムアウト付き応答を受け取るための`AskTimeoutFuture`、またはエラー
  pub fn ask_with_timeout<V, Resp, F, TFut>(
    &mut self,
    target: &ActorRef<V, R>,
    factory: F,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    V: Element,
    Resp: Element,
    F: FnOnce(MessageSender<Resp>) -> V,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp>();
    let responder_for_message = MessageSender::new(responder.internal());
    let message = factory(responder_for_message);
    let metadata = MessageMetadata::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    match target.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }

  /// ターゲットアクターに問い合わせを行い、応答を待つFutureを返す。
  ///
  /// # 引数
  /// * `target` - 問い合わせ先アクター
  /// * `message` - 送信するメッセージ
  ///
  /// # 戻り値
  /// 応答を受け取るための`AskFuture`、またはエラー
  pub fn request_future<V, Resp>(&mut self, target: &ActorRef<V, R>, message: V) -> AskResult<AskFuture<Resp>>
  where
    V: Element,
    Resp: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    target.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// タイムアウト付きで問い合わせを行い、応答を待つFutureを返す。
  ///
  /// # 引数
  /// * `target` - 問い合わせ先アクター
  /// * `message` - 送信するメッセージ
  /// * `timeout` - タイムアウト制御用のFuture
  ///
  /// # 戻り値
  /// タイムアウト付き応答を受け取るための`AskTimeoutFuture`、またはエラー
  pub fn request_future_with_timeout<V, Resp, TFut>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    V: Element,
    Resp: Element,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    match target.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }

  /// 子アクターを生成し、`ActorRef` を返す。
  pub fn spawn_child<V>(&mut self, props: Props<V, R>) -> ActorRef<V, R>
  where
    V: Element, {
    let (internal_props, supervisor_cfg) = props.into_parts();
    let actor_ref = self
      .inner
      .spawn_child_from_props(Box::new(supervisor_cfg.into_supervisor()), internal_props);
    ActorRef::new(actor_ref)
  }
}

impl MessageMetadata {
  /// メタデータを使って応答メッセージを送信する。
  ///
  /// # 引数
  /// * `ctx` - 現在のコンテキスト
  /// * `message` - 送信する応答メッセージ
  ///
  /// # 戻り値
  /// 送信成功時は`Ok(())`、失敗時は`AskError`
  ///
  /// # エラー
  /// - `AskError::MissingResponder` - 応答先が見つからない場合
  /// - `AskError::SendFailed` - メッセージ送信に失敗した場合
  pub fn respond_with<Resp, U, R>(&self, ctx: &mut Context<'_, '_, U, R>, message: Resp) -> AskResult<()>
  where
    Resp: Element,
    U: Element,
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let dispatcher = self.dispatcher_for::<Resp>().ok_or(AskError::MissingResponder)?;
    let dispatch_metadata = MessageMetadata::new().with_sender(ctx.self_dispatcher());
    let envelope = MessageEnvelope::user_with_metadata(message, dispatch_metadata);
    dispatcher.dispatch_envelope(envelope).map_err(AskError::from)
  }
}

/// メッセージアダプターへの参照。
///
/// 外部メッセージ型を内部メッセージ型に変換してターゲットアクターに送信する。
#[derive(Clone)]
pub struct MessageAdapterRef<Ext, U, R>
where
  Ext: Element,
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  target: ActorRef<U, R>,
  adapter: Arc<dyn Fn(Ext) -> U + Send + Sync>,
}

impl<Ext, U, R> MessageAdapterRef<Ext, U, R>
where
  Ext: Element,
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub(crate) fn new(target: ActorRef<U, R>, adapter: Arc<dyn Fn(Ext) -> U + Send + Sync>) -> Self {
    Self { target, adapter }
  }

  /// 外部メッセージを変換してターゲットアクターに送信する。
  ///
  /// # 引数
  /// * `message` - 送信する外部メッセージ
  ///
  /// # 戻り値
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn tell(&self, message: Ext) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let mapped = (self.adapter)(message);
    self.target.tell(mapped)
  }

  /// 外部メッセージを変換し、指定された優先度でターゲットアクターに送信する。
  ///
  /// # 引数
  /// * `message` - 送信する外部メッセージ
  /// * `priority` - メッセージの優先度
  ///
  /// # 戻り値
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn tell_with_priority(&self, message: Ext, priority: i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let mapped = (self.adapter)(message);
    self.target.tell_with_priority(mapped, priority)
  }

  /// ターゲットアクターへの参照を取得する。
  ///
  /// # 戻り値
  /// ターゲット`ActorRef`への参照
  pub fn target(&self) -> &ActorRef<U, R> {
    &self.target
  }
}
