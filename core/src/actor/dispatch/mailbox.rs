//! Mailbox trait and implementations.

use async_trait::async_trait;
use std::fmt::Debug;

use crate::actor::message::MessageHandle;

#[async_trait]
pub trait Mailbox: Debug + Send + Sync {
  async fn post_system_message(&self, message: MessageHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
  async fn post_user_message(&self, message: MessageHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
  async fn receive_system_message(&self) -> Option<MessageHandle>;
  async fn receive_user_message(&self) -> Option<MessageHandle>;
}
