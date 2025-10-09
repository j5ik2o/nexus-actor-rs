use crate::runtime::context::InternalActorRef;
use crate::runtime::message::DynMessage;
use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope};
use core::future::Future;
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use super::{ask::create_ask_handles, ask_with_timeout, AskError, AskFuture, AskResult, AskTimeoutFuture};
use crate::api::{InternalMessageSender, MessageEnvelope, MessageMetadata, MessageSender};

/// 型付きアクター参照。
///
/// メールボックスにユーザーメッセージやシステムメッセージを送信したり、
/// `ask` 系 API で応答を受け取るときに利用する。
#[derive(Clone)]
pub struct ActorRef<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  inner: InternalActorRef<DynMessage, R>,
  _marker: PhantomData<U>,
}

impl<U, R> ActorRef<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// 内部参照から新しい `ActorRef` を生成する。
  ///
  /// # Arguments
  /// * `inner` - 内部アクター参照
  pub(crate) fn new(inner: InternalActorRef<DynMessage, R>) -> Self {
    Self {
      inner,
      _marker: PhantomData,
    }
  }

  /// ユーザーメッセージを動的メッセージへラップする。
  ///
  /// # Arguments
  /// * `message` - ラップするユーザーメッセージ
  pub(crate) fn wrap_user(message: U) -> DynMessage {
    DynMessage::new(MessageEnvelope::user(message))
  }

  /// メタデータ付きのユーザーメッセージを動的メッセージへラップする。
  ///
  /// # Arguments
  /// * `message` - ラップするユーザーメッセージ
  /// * `metadata` - メッセージに付随するメタデータ
  pub(crate) fn wrap_user_with_metadata(message: U, metadata: MessageMetadata) -> DynMessage {
    DynMessage::new(MessageEnvelope::user_with_metadata(message, metadata))
  }

  /// 既にラップ済みのメッセージを優先度付きで送信する。
  ///
  /// # Arguments
  /// * `dyn_message` - 動的メッセージ
  /// * `priority` - メッセージの優先度
  fn send_envelope(
    &self,
    dyn_message: DynMessage,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.try_send_with_priority(dyn_message, priority)
  }

  /// メタデータ付きでメッセージを送信する（内部API）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `metadata` - メッセージに付随するメタデータ
  pub(crate) fn tell_with_metadata(
    &self,
    message: U,
    metadata: MessageMetadata,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = Self::wrap_user_with_metadata(message, metadata);
    self.send_envelope(dyn_message, DEFAULT_PRIORITY)
  }

  /// メッセージを送信する（Fire-and-Forget）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  ///
  /// # Returns
  /// 送信成功時は`Ok(())`、失敗時はエラー
  pub fn tell(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self
      .inner
      .try_send_with_priority(Self::wrap_user(message), DEFAULT_PRIORITY)
  }

  /// 優先度を指定してメッセージを送信する。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `priority` - メッセージの優先度（値が小さいほど高優先度）
  ///
  /// # Returns
  /// 送信成功時は`Ok(())`、失敗時はエラー
  pub fn tell_with_priority(&self, message: U, priority: i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.try_send_with_priority(Self::wrap_user(message), priority)
  }

  /// システムメッセージを送信する。
  ///
  /// # Arguments
  /// * `message` - 送信するシステムメッセージ
  ///
  /// # Returns
  /// 送信成功時は`Ok(())`、失敗時はエラー
  pub fn send_system(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let envelope =
      PriorityEnvelope::from_system(message.clone()).map(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)));
    self.inner.try_send_envelope(envelope)
  }

  /// このアクター参照をメッセージディスパッチャに変換する。
  ///
  /// # Returns
  /// メッセージ送信用のディスパッチャ
  pub fn to_dispatcher(&self) -> MessageSender<U>
  where
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let internal = InternalMessageSender::from_internal_ref(self.inner.clone());
    MessageSender::new(internal)
  }

  /// 送信元アクターを指定してリクエストを送信する（内部API）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `sender` - 送信元アクターの参照
  #[allow(dead_code)]
  pub(crate) fn request_from<S>(
    &self,
    message: U,
    sender: &ActorRef<S, R>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    S: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    self.request_with_dispatcher(message, sender.to_dispatcher())
  }

  /// ディスパッチャを指定してリクエストを送信する（内部API）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `sender` - 送信元ディスパッチャ
  #[allow(dead_code)]
  pub(crate) fn request_with_dispatcher<S>(
    &self,
    message: U,
    sender: MessageSender<S>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    S: Element, {
    let metadata = MessageMetadata::new().with_sender(sender);
    self.tell_with_metadata(message, metadata)
  }

  /// 応答チャネルを内部で生成し、`message` を送って `AskFuture` を返す（内部API）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  ///
  /// # Returns
  /// 応答を待ち受ける`AskFuture`
  pub(crate) fn request_future<Resp>(&self, message: U) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element, {
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new().with_responder(responder);
    self.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// 送信元のアクター参照を指定して `ask` を発行する（内部API）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `sender` - 送信元アクターの参照
  #[allow(dead_code)]
  pub(crate) fn request_future_from<Resp, S>(&self, message: U, sender: &ActorRef<S, R>) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element,
    S: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    self.request_future_with_dispatcher(message, sender.to_dispatcher())
  }

  /// 任意のディスパッチャを送信元として `ask` を発行する（内部API）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `sender` - 送信元ディスパッチャ
  #[allow(dead_code)]
  pub(crate) fn request_future_with_dispatcher<Resp, S>(
    &self,
    message: U,
    sender: MessageSender<S>,
  ) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element,
    S: Element, {
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new().with_sender(sender).with_responder(responder);
    self.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// タイムアウト付きで`ask`を発行する（内部API）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `timeout` - タイムアウト制御用のFuture
  #[allow(dead_code)]
  pub(crate) fn request_future_with_timeout<Resp, TFut>(
    &self,
    message: U,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    Resp: Element,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new().with_responder(responder);
    match self.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }

  /// 送信元を指定してタイムアウト付き`ask`を発行する（内部API）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `sender` - 送信元アクターの参照
  /// * `timeout` - タイムアウト制御用のFuture
  #[allow(dead_code)]
  pub(crate) fn request_future_with_timeout_from<Resp, S, TFut>(
    &self,
    message: U,
    sender: &ActorRef<S, R>,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    Resp: Element,
    S: Element,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    self.request_future_with_timeout_dispatcher(message, sender.to_dispatcher(), timeout)
  }

  /// ディスパッチャを指定してタイムアウト付き`ask`を発行する（内部API）。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `sender` - 送信元ディスパッチャ
  /// * `timeout` - タイムアウト制御用のFuture
  #[allow(dead_code)]
  pub(crate) fn request_future_with_timeout_dispatcher<Resp, S, TFut>(
    &self,
    message: U,
    sender: MessageSender<S>,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    Resp: Element,
    S: Element,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new().with_sender(sender).with_responder(responder);
    match self.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }

  /// ファクトリ関数でメッセージを構築し、`ask`パターンで送信する。
  ///
  /// 応答用のディスパッチャをファクトリに渡してメッセージを構築できる。
  ///
  /// # Arguments
  /// * `factory` - 応答用ディスパッチャを受け取りメッセージを生成する関数
  ///
  /// # Returns
  /// 応答を待ち受ける`AskFuture`
  pub fn ask_with<Resp, F>(&self, factory: F) -> AskResult<AskFuture<Resp>>
  where
    Resp: Element,
    F: FnOnce(MessageSender<Resp>) -> U, {
    let (future, responder) = create_ask_handles::<Resp>();
    let responder_for_message = MessageSender::new(responder.internal());
    let message = factory(responder_for_message);
    let metadata = MessageMetadata::new().with_responder(responder);
    self.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  /// タイムアウト付きでファクトリ関数を使って`ask`を発行する。
  ///
  /// # Arguments
  /// * `factory` - 応答用ディスパッチャを受け取りメッセージを生成する関数
  /// * `timeout` - タイムアウト制御用のFuture
  ///
  /// # Returns
  /// タイムアウト制御付きの`AskTimeoutFuture`
  pub fn ask_with_timeout<Resp, F, TFut>(&self, factory: F, timeout: TFut) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    Resp: Element,
    F: FnOnce(MessageSender<Resp>) -> U,
    TFut: Future<Output = ()> + Unpin, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp>();
    let responder_for_message = MessageSender::new(responder.internal());
    let message = factory(responder_for_message);
    let metadata = MessageMetadata::new().with_responder(responder);
    match self.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }
}
