use alloc::sync::Arc;

use crate::runtime::context::InternalActorRef;
use crate::runtime::message::{store_metadata, take_metadata, DynMessage, MetadataKey};
use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope};
use core::marker::PhantomData;
use core::mem::{forget, ManuallyDrop};
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

type SendFn = dyn Fn(DynMessage, i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> + Send + Sync;

/// 送信先を抽象化した内部ディスパッチャ。Ask 応答などで利用する。
#[derive(Clone)]
pub struct InternalMessageSender {
  inner: Arc<SendFn>,
  drop_hook: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl core::fmt::Debug for InternalMessageSender {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_str("MessageSender(..)")
  }
}

impl InternalMessageSender {
  /// 送信関数を指定して新しい`InternalMessageSender`を作成する。
  ///
  /// # Arguments
  /// * `inner` - メッセージ送信を実行する関数
  pub fn new(inner: Arc<SendFn>) -> Self {
    Self { inner, drop_hook: None }
  }

  /// ドロップフックを持つ`InternalMessageSender`を作成する（内部API）。
  ///
  /// # Arguments
  /// * `inner` - メッセージ送信を実行する関数
  /// * `drop_hook` - ドロップ時に実行されるフック関数
  pub(crate) fn with_drop_hook(inner: Arc<SendFn>, drop_hook: Arc<dyn Fn() + Send + Sync>) -> Self {
    Self {
      inner,
      drop_hook: Some(drop_hook),
    }
  }

  /// デフォルト優先度でメッセージを送信する。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  ///
  /// # Returns
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn send_default(&self, message: DynMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.send_with_priority(message, DEFAULT_PRIORITY)
  }

  /// 指定された優先度でメッセージを送信する。
  ///
  /// # Arguments
  /// * `message` - 送信するメッセージ
  /// * `priority` - メッセージの優先度
  ///
  /// # Returns
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn send_with_priority(
    &self,
    message: DynMessage,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    (self.inner)(message, priority)
  }

  /// 内部アクター参照から`InternalMessageSender`を作成する（内部API）。
  ///
  /// # Arguments
  /// * `actor_ref` - 送信先のアクター参照
  pub(crate) fn from_internal_ref<R>(actor_ref: InternalActorRef<DynMessage, R>) -> Self
  where
    R: MailboxFactory + Clone + 'static,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let sender = actor_ref.clone();
    Self::new(Arc::new(move |message, priority| {
      sender.try_send_with_priority(message, priority)
    }))
  }
}

impl Drop for InternalMessageSender {
  fn drop(&mut self) {
    if let Some(hook) = &self.drop_hook {
      hook();
    }
  }
}

/// 型安全なディスパッチャ。内部ディスパッチャをラップし、ユーザーメッセージを自動的にエンベロープ化する。
#[derive(Clone)]
pub struct MessageSender<M>
where
  M: Element, {
  inner: InternalMessageSender,
  _marker: PhantomData<fn(M)>,
}

impl<M> core::fmt::Debug for MessageSender<M>
where
  M: Element,
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("MessageSender").finish()
  }
}

impl<M> MessageSender<M>
where
  M: Element,
{
  /// 内部センダーから型付き`MessageSender`を作成する（内部API）。
  ///
  /// # Arguments
  /// * `inner` - 内部メッセージセンダー
  pub(crate) fn new(inner: InternalMessageSender) -> Self {
    Self {
      inner,
      _marker: PhantomData,
    }
  }

  /// 内部センダーから型付き`MessageSender`を作成する。
  ///
  /// # Arguments
  /// * `inner` - 内部メッセージセンダー
  pub fn from_internal(inner: InternalMessageSender) -> Self {
    Self::new(inner)
  }

  /// ユーザーメッセージをディスパッチする。
  ///
  /// # Arguments
  /// * `message` - 送信するユーザーメッセージ
  ///
  /// # Returns
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn dispatch_user(&self, message: M) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.dispatch_envelope(MessageEnvelope::user(message))
  }

  /// メッセージエンベロープをディスパッチする。
  ///
  /// # Arguments
  /// * `envelope` - 送信するメッセージエンベロープ
  ///
  /// # Returns
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn dispatch_envelope(
    &self,
    envelope: MessageEnvelope<M>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(envelope);
    self.inner.send_default(dyn_message)
  }

  /// 指定された優先度でメッセージエンベロープをディスパッチする。
  ///
  /// # Arguments
  /// * `envelope` - 送信するメッセージエンベロープ
  /// * `priority` - メッセージの優先度
  ///
  /// # Returns
  /// 送信成功時は`Ok(())`、失敗時はキューエラー
  pub fn dispatch_with_priority(
    &self,
    envelope: MessageEnvelope<M>,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(envelope);
    self.inner.send_with_priority(dyn_message, priority)
  }

  /// 内部センダーのクローンを取得する。
  ///
  /// # Returns
  /// 内部メッセージセンダーのクローン
  pub fn internal(&self) -> InternalMessageSender {
    self.inner.clone()
  }

  /// 内部センダーに変換し、所有権を移動する。
  ///
  /// # Returns
  /// 内部メッセージセンダー
  pub fn into_internal(self) -> InternalMessageSender {
    self.inner
  }
}

/// メッセージに付随するメタデータ（内部表現）。
#[derive(Debug, Clone, Default)]
pub struct InternalMessageMetadata {
  sender: Option<InternalMessageSender>,
  responder: Option<InternalMessageSender>,
}

impl InternalMessageMetadata {
  /// 送信者と応答者を指定して新しいメタデータを作成する。
  ///
  /// # Arguments
  /// * `sender` - 送信者のセンダー（オプション）
  /// * `responder` - 応答者のセンダー（オプション）
  pub fn new(sender: Option<InternalMessageSender>, responder: Option<InternalMessageSender>) -> Self {
    Self { sender, responder }
  }

  /// 送信者のセンダーへの参照を取得する。
  ///
  /// # Returns
  /// 送信者が存在する場合は`Some(&InternalMessageSender)`、存在しない場合は`None`
  pub fn sender(&self) -> Option<&InternalMessageSender> {
    self.sender.as_ref()
  }

  /// 送信者のセンダーのクローンを取得する。
  ///
  /// # Returns
  /// 送信者が存在する場合は`Some(InternalMessageSender)`、存在しない場合は`None`
  pub fn sender_cloned(&self) -> Option<InternalMessageSender> {
    self.sender.clone()
  }

  /// 応答者のセンダーへの参照を取得する。
  ///
  /// # Returns
  /// 応答者が存在する場合は`Some(&InternalMessageSender)`、存在しない場合は`None`
  pub fn responder(&self) -> Option<&InternalMessageSender> {
    self.responder.as_ref()
  }

  /// 応答者のセンダーのクローンを取得する。
  ///
  /// # Returns
  /// 応答者が存在する場合は`Some(InternalMessageSender)`、存在しない場合は`None`
  pub fn responder_cloned(&self) -> Option<InternalMessageSender> {
    self.responder.clone()
  }

  /// 送信者を設定して自身を返す（ビルダーパターン）。
  ///
  /// # Arguments
  /// * `sender` - 設定する送信者のセンダー
  pub fn with_sender(mut self, sender: Option<InternalMessageSender>) -> Self {
    self.sender = sender;
    self
  }

  /// 応答者を設定して自身を返す（ビルダーパターン）。
  ///
  /// # Arguments
  /// * `responder` - 設定する応答者のセンダー
  pub fn with_responder(mut self, responder: Option<InternalMessageSender>) -> Self {
    self.responder = responder;
    self
  }
}

/// 外部 API 向けの型付きメタデータ。
#[derive(Debug, Clone, Default)]
pub struct MessageMetadata {
  inner: InternalMessageMetadata,
}

impl MessageMetadata {
  /// 新しい空のメタデータを作成する。
  pub fn new() -> Self {
    Self::default()
  }

  /// 送信者を設定して自身を返す（ビルダーパターン）。
  ///
  /// # Arguments
  /// * `sender` - 設定する送信者のセンダー
  pub fn with_sender<U>(mut self, sender: MessageSender<U>) -> Self
  where
    U: Element, {
    self.inner = self.inner.with_sender(Some(sender.into_internal()));
    self
  }

  /// 応答者を設定して自身を返す（ビルダーパターン）。
  ///
  /// # Arguments
  /// * `responder` - 設定する応答者のセンダー
  pub fn with_responder<U>(mut self, responder: MessageSender<U>) -> Self
  where
    U: Element, {
    self.inner = self.inner.with_responder(Some(responder.into_internal()));
    self
  }

  /// 指定された型の送信者センダーを取得する。
  ///
  /// # Returns
  /// 送信者が存在する場合は`Some(MessageSender<U>)`、存在しない場合は`None`
  pub fn sender_as<U>(&self) -> Option<MessageSender<U>>
  where
    U: Element, {
    self.inner.sender_cloned().map(MessageSender::new)
  }

  /// 指定された型の応答者センダーを取得する。
  ///
  /// # Returns
  /// 応答者が存在する場合は`Some(MessageSender<U>)`、存在しない場合は`None`
  pub fn responder_as<U>(&self) -> Option<MessageSender<U>>
  where
    U: Element, {
    self.inner.responder_cloned().map(MessageSender::new)
  }

  /// 指定された型のディスパッチャーを取得する（応答者優先）。
  ///
  /// 応答者が存在する場合はそれを返し、存在しない場合は送信者を返す。
  ///
  /// # Returns
  /// ディスパッチャーが存在する場合は`Some(MessageSender<U>)`、存在しない場合は`None`
  pub fn dispatcher_for<U>(&self) -> Option<MessageSender<U>>
  where
    U: Element, {
    self.responder_as::<U>().or_else(|| self.sender_as::<U>())
  }

  /// メタデータが空かどうかを判定する。
  ///
  /// # Returns
  /// 送信者も応答者も存在しない場合は`true`、それ以外は`false`
  pub fn is_empty(&self) -> bool {
    self.inner.sender.is_none() && self.inner.responder.is_none()
  }
}

/// ユーザーメッセージとメタデータを保持するラッパー。
#[derive(Debug, Clone)]
pub struct UserMessage<U> {
  message: ManuallyDrop<U>,
  metadata_key: Option<MetadataKey>,
}

impl<U> UserMessage<U> {
  /// メッセージのみを持つ新しい`UserMessage`を作成する。
  ///
  /// # Arguments
  /// * `message` - ユーザーメッセージ
  pub fn new(message: U) -> Self {
    Self {
      message: ManuallyDrop::new(message),
      metadata_key: None,
    }
  }

  /// メッセージとメタデータを持つ新しい`UserMessage`を作成する。
  ///
  /// メタデータが空の場合は、メタデータなしで作成される。
  ///
  /// # Arguments
  /// * `message` - ユーザーメッセージ
  /// * `metadata` - メッセージメタデータ
  pub fn with_metadata(message: U, metadata: MessageMetadata) -> Self {
    if metadata.is_empty() {
      Self::new(message)
    } else {
      let key = store_metadata(metadata);
      Self {
        message: ManuallyDrop::new(message),
        metadata_key: Some(key),
      }
    }
  }

  /// メッセージへの参照を取得する。
  ///
  /// # Returns
  /// ユーザーメッセージへの参照
  pub fn message(&self) -> &U {
    &*self.message
  }

  /// メタデータキーを取得する。
  ///
  /// # Returns
  /// メタデータが存在する場合は`Some(MetadataKey)`、存在しない場合は`None`
  pub fn metadata_key(&self) -> Option<MetadataKey> {
    self.metadata_key
  }

  /// メッセージとメタデータキーに分解する。
  ///
  /// # Returns
  /// `(メッセージ, メタデータキー)`のタプル
  pub fn into_parts(mut self) -> (U, Option<MetadataKey>) {
    let key = self.metadata_key.take();
    let message = unsafe { ManuallyDrop::take(&mut self.message) };
    forget(self);
    (message, key)
  }
}

impl<U> From<U> for UserMessage<U> {
  fn from(message: U) -> Self {
    Self::new(message)
  }
}

impl<U> Drop for UserMessage<U> {
  fn drop(&mut self) {
    if let Some(key) = self.metadata_key.take() {
      let _ = take_metadata(key);
    }
    unsafe {
      ManuallyDrop::drop(&mut self.message);
    }
  }
}

/// ユーザーメッセージとシステムメッセージを統合した型付きエンベロープ。
#[derive(Debug, Clone)]
pub enum MessageEnvelope<U> {
  /// ユーザーメッセージを保持するバリアント。
  User(UserMessage<U>),
  /// システムメッセージを保持するバリアント。
  System(SystemMessage),
}

impl<U> MessageEnvelope<U>
where
  U: Element,
{
  /// ユーザーメッセージのエンベロープを作成する。
  ///
  /// # Arguments
  /// * `message` - ユーザーメッセージ
  pub fn user(message: U) -> Self {
    MessageEnvelope::User(UserMessage::new(message))
  }

  /// メタデータ付きユーザーメッセージのエンベロープを作成する。
  ///
  /// # Arguments
  /// * `message` - ユーザーメッセージ
  /// * `metadata` - メッセージメタデータ
  pub fn user_with_metadata(message: U, metadata: MessageMetadata) -> Self {
    MessageEnvelope::User(UserMessage::with_metadata(message, metadata))
  }
}

impl<U> Element for MessageEnvelope<U> where U: Element {}
