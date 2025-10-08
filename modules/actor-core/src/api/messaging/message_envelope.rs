use crate::SystemMessage;
use nexus_utils_core_rs::Element;

/// Typed envelope combining user and system messages.
#[derive(Debug, Clone)]
pub enum MessageEnvelope<U> {
  User(U),
  System(SystemMessage),
}

impl<U> Element for MessageEnvelope<U> where U: Element {}
