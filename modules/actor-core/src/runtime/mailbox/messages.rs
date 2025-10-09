use nexus_utils_core_rs::{Element, PriorityMessage, DEFAULT_PRIORITY};

use crate::ActorId;
use crate::FailureInfo;

/// Channel type to distinguish control messages.
///
/// Used for message priority control within the mailbox.
/// Control messages are processed with higher priority than regular messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorityChannel {
  /// Regular application messages
  Regular,
  /// System control messages (stop, restart, etc.)
  Control,
}

/// Control message types based on protoactor-go's `SystemMessage`.
///
/// Special messages used for internal control of the actor system.
/// Field structure is kept focused on current requirements and can be extended as needed.
///
/// Each variant has a different priority and is used at various stages of the actor lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessage {
  /// Start watching another actor
  Watch(ActorId),
  /// Stop watching another actor
  Unwatch(ActorId),
  /// Instruct actor to stop
  Stop,
  /// Notify of a failure occurrence
  Failure(FailureInfo),
  /// Instruct actor to restart
  Restart,
  /// Suspend actor message processing
  Suspend,
  /// Resume actor message processing
  Resume,
  /// Escalate failure to parent actor
  Escalate(FailureInfo),
  /// Notify receive timeout
  ReceiveTimeout,
}

impl SystemMessage {
  /// Gets the recommended priority. Based on protoactor-go's priority table.
  ///
  /// # Returns
  /// Message priority (higher values mean higher priority)
  ///
  /// # Priority Order
  /// - Escalate: Highest priority (DEFAULT_PRIORITY + 13)
  /// - Failure: DEFAULT_PRIORITY + 12
  /// - Restart: DEFAULT_PRIORITY + 11
  /// - Stop: DEFAULT_PRIORITY + 10
  /// - Suspend/Resume: DEFAULT_PRIORITY + 9
  /// - ReceiveTimeout: DEFAULT_PRIORITY + 8
  /// - Watch/Unwatch: DEFAULT_PRIORITY + 5
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

/// Envelope type: Stores priority messages and implements `PriorityMessage`.
///
/// Wraps messages with priority and channel information.
/// Used for prioritized message processing within mailboxes.
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
  /// Creates an envelope on the regular channel with specified priority.
  ///
  /// # Arguments
  /// - `message`: Message to wrap
  /// - `priority`: Message priority
  pub fn new(message: M, priority: i8) -> Self {
    Self::with_channel(message, priority, PriorityChannel::Regular)
  }

  /// Creates an envelope with specified channel and priority.
  ///
  /// # Arguments
  /// - `message`: Message to wrap
  /// - `priority`: Message priority
  /// - `channel`: Message channel type
  pub fn with_channel(message: M, priority: i8, channel: PriorityChannel) -> Self {
    Self {
      message,
      priority,
      channel,
      system_message: None,
    }
  }

  /// Creates a control channel envelope.
  ///
  /// # Arguments
  /// - `message`: Message to wrap
  /// - `priority`: Message priority
  pub fn control(message: M, priority: i8) -> Self {
    Self::with_channel(message, priority, PriorityChannel::Control)
  }

  /// Gets a reference to the internal message.
  pub fn message(&self) -> &M {
    &self.message
  }

  /// Gets the message priority.
  pub fn priority(&self) -> i8 {
    self.priority
  }

  /// Gets the message channel type.
  pub fn channel(&self) -> PriorityChannel {
    self.channel
  }

  /// Checks if this is a control message.
  ///
  /// # Returns
  /// `true` for control channel, `false` for regular channel
  pub fn is_control(&self) -> bool {
    matches!(self.channel, PriorityChannel::Control)
  }

  /// Gets the associated system message if any.
  pub fn system_message(&self) -> Option<&SystemMessage> {
    self.system_message.as_ref()
  }

  /// Decomposes the envelope to get the message and priority.
  ///
  /// # Returns
  /// Tuple of `(message, priority)`
  pub fn into_parts(self) -> (M, i8) {
    (self.message, self.priority)
  }

  /// Decomposes the envelope to get the message, priority, and channel.
  ///
  /// # Returns
  /// Tuple of `(message, priority, channel)`
  pub fn into_parts_with_channel(self) -> (M, i8, PriorityChannel) {
    (self.message, self.priority, self.channel)
  }

  /// Applies a function to the message and converts to an envelope of a new type.
  ///
  /// Priority and channel information are preserved.
  ///
  /// # Arguments
  /// - `f`: Function to transform the message
  pub fn map<N>(self, f: impl FnOnce(M) -> N) -> PriorityEnvelope<N> {
    PriorityEnvelope {
      message: f(self.message),
      priority: self.priority,
      channel: self.channel,
      system_message: self.system_message,
    }
  }

  /// Applies a function to modify the priority.
  ///
  /// # Arguments
  /// - `f`: Function to transform the priority
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
  /// Creates a regular channel envelope with default priority.
  ///
  /// # Arguments
  /// - `message`: Message to wrap
  pub fn with_default_priority(message: M) -> Self {
    Self::new(message, DEFAULT_PRIORITY)
  }
}

impl PriorityEnvelope<SystemMessage> {
  /// Envelope helper for SystemMessage.
  ///
  /// Automatically wraps system messages with appropriate priority and control channel.
  ///
  /// # Arguments
  /// - `message`: System message to wrap
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
