use nexus_utils_core_rs::{Element, PriorityMessage, DEFAULT_PRIORITY};

use crate::ActorId;
use crate::FailureInfo;

/// 制御メッセージかどうかを区別するチャネル種別。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorityChannel {
  Regular,
  Control,
}

/// protoactor-go の `SystemMessage` を参考にした制御メッセージ種別。
/// フィールド構成は現段階の要件に絞り、必要に応じて拡張する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessage {
  Watch(ActorId),
  Unwatch(ActorId),
  Stop,
  Failure(FailureInfo),
  Restart,
  Suspend,
  Resume,
  Escalate(FailureInfo),
  ReceiveTimeout,
}

impl SystemMessage {
  /// 推奨優先度。protoactor-go の優先度テーブルをベースに設定。
  pub fn priority(&self) -> i8 {
    match self {
      SystemMessage::Watch(_) | SystemMessage::Unwatch(_) => DEFAULT_PRIORITY + 5,
      SystemMessage::Stop => DEFAULT_PRIORITY + 10,
      SystemMessage::Failure(_) => DEFAULT_PRIORITY + 12,
      SystemMessage::Restart => DEFAULT_PRIORITY + 11,
      SystemMessage::Suspend | SystemMessage::Resume => DEFAULT_PRIORITY + 9,
      SystemMessage::Escalate(_) => DEFAULT_PRIORITY + 13,
      SystemMessage::ReceiveTimeout => DEFAULT_PRIORITY + 8,
    }
  }
}

impl Element for SystemMessage {}

/// Envelope型: 優先度付きメッセージを格納し、`PriorityMessage` を実装する。
#[allow(dead_code)]
#[derive(Debug)]
pub struct PriorityEnvelope<M> {
  message: M,
  priority: i8,
  channel: PriorityChannel,
  system_message: Option<SystemMessage>,
}

#[allow(dead_code)]
impl<M> PriorityEnvelope<M> {
  pub fn new(message: M, priority: i8) -> Self {
    Self::with_channel(message, priority, PriorityChannel::Regular)
  }

  pub fn with_channel(message: M, priority: i8, channel: PriorityChannel) -> Self {
    Self {
      message,
      priority,
      channel,
      system_message: None,
    }
  }

  pub fn control(message: M, priority: i8) -> Self {
    Self::with_channel(message, priority, PriorityChannel::Control)
  }

  pub fn message(&self) -> &M {
    &self.message
  }

  pub fn priority(&self) -> i8 {
    self.priority
  }

  pub fn channel(&self) -> PriorityChannel {
    self.channel
  }

  pub fn is_control(&self) -> bool {
    matches!(self.channel, PriorityChannel::Control)
  }

  pub fn system_message(&self) -> Option<&SystemMessage> {
    self.system_message.as_ref()
  }

  pub fn into_parts(self) -> (M, i8) {
    (self.message, self.priority)
  }

  pub fn into_parts_with_channel(self) -> (M, i8, PriorityChannel) {
    (self.message, self.priority, self.channel)
  }

  pub fn map<N>(self, f: impl FnOnce(M) -> N) -> PriorityEnvelope<N> {
    PriorityEnvelope {
      message: f(self.message),
      priority: self.priority,
      channel: self.channel,
      system_message: self.system_message,
    }
  }

  pub fn map_priority(mut self, f: impl FnOnce(i8) -> i8) -> Self {
    self.priority = f(self.priority);
    self
  }
}

#[allow(dead_code)]
impl<M> PriorityEnvelope<M>
where
  M: Element,
{
  pub fn with_default_priority(message: M) -> Self {
    Self::new(message, DEFAULT_PRIORITY)
  }
}

impl PriorityEnvelope<SystemMessage> {
  /// SystemMessage 用の Envelope ヘルパー。
  pub fn from_system(message: SystemMessage) -> Self {
    let priority = message.priority();
    let system_clone = message.clone();
    let mut envelope = PriorityEnvelope::with_channel(message, priority, PriorityChannel::Control);
    envelope.system_message = Some(system_clone);
    envelope
  }
}

impl<M> PriorityMessage for PriorityEnvelope<M>
where
  M: Element,
{
  fn get_priority(&self) -> Option<i8> {
    Some(self.priority)
  }
}

impl<M> Element for PriorityEnvelope<M> where M: Element {}
