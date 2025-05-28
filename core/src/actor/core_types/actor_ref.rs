use crate::actor::message::MessageHandle;
use async_trait::async_trait;
use std::fmt::Debug;

/// Basic interface for actor references
/// This trait provides the minimal interface needed to send messages to actors
#[async_trait]
pub trait ActorRef: Debug + Send + Sync + 'static {
  /// Get the unique identifier for this actor
  fn get_id(&self) -> String;
  
  /// Get the address of the actor system hosting this actor
  fn get_address(&self) -> String;
  
  /// Send a message to this actor
  async fn tell(&self, message: MessageHandle);
  
  /// Send a message and wait for a response
  async fn request(&self, message: MessageHandle, timeout: std::time::Duration) -> Result<MessageHandle, ActorRefError>;
  
  /// Check if this actor reference is still valid
  fn is_alive(&self) -> bool;
}

/// Errors that can occur when using ActorRef
#[derive(Debug, Clone, thiserror::Error)]
pub enum ActorRefError {
  #[error("Actor not found")]
  ActorNotFound,
  
  #[error("Request timeout")]
  Timeout,
  
  #[error("Actor system shutdown")]
  SystemShutdown,
}

/// Trait for sending messages
#[async_trait]
pub trait MessageSender: Debug + Send + Sync + 'static {
  /// Send a message without waiting for response
  async fn send(&self, message: MessageHandle);
  
  /// Send a message with sender information
  async fn send_with_sender(&self, message: MessageHandle, sender: Box<dyn ActorRef>);
}

/// Trait for basic context operations
#[async_trait]
pub trait ContextProvider: Debug + Send + Sync + 'static {
  /// Get reference to self
  fn get_self(&self) -> Box<dyn ActorRef>;
  
  /// Get reference to parent actor if exists
  fn get_parent(&self) -> Option<Box<dyn ActorRef>>;
  
  /// Get references to all children
  async fn get_children(&self) -> Vec<Box<dyn ActorRef>>;
  
  /// Get actor system name
  fn get_actor_system_name(&self) -> String;
}