use crate::actor::core_types::ActorRef;
use crate::actor::message::MessageHandle;
use async_trait::async_trait;
use std::fmt::Debug;
use std::any::Any;

/// Base context trait that provides minimal context functionality
/// This avoids circular dependencies by not depending on specific actor implementations
#[async_trait]
pub trait BaseContext: Debug + Send + Sync + 'static {
  /// Get a reference to self as actor ref
  fn self_ref(&self) -> Box<dyn ActorRef>;
  
  /// Get parent reference if exists
  fn parent_ref(&self) -> Option<Box<dyn ActorRef>>;
  
  /// Send a message to an actor
  async fn send(&self, target: &dyn ActorRef, message: MessageHandle);
  
  /// Get current message being processed
  async fn get_message(&self) -> MessageHandle;
  
  /// Get sender of current message if exists
  async fn get_sender(&self) -> Option<Box<dyn ActorRef>>;
  
  /// Spawn a child actor and return its reference
  async fn spawn_child(&self, name: &str, factory: Box<dyn ActorFactory>) -> Box<dyn ActorRef>;
  
  /// Stop a child actor
  async fn stop_child(&self, child: &dyn ActorRef);
  
  /// Get self as Any for downcasting
  fn as_any(&self) -> &dyn Any;
}

/// Factory trait for creating actors without circular dependencies
#[async_trait]
pub trait ActorFactory: Send + Sync + 'static {
  /// Create a new actor instance
  async fn create(&self) -> Box<dyn BaseActor>;
}

/// Base actor trait without context dependencies
#[async_trait]
pub trait BaseActor: Debug + Send + Sync + 'static {
  /// Get the type name of this actor
  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
  
  /// Handle a message
  async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError>;
  
  /// Called before actor starts
  async fn pre_start(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
    Ok(())
  }
  
  /// Called after actor starts
  async fn post_start(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
    Ok(())
  }
  
  /// Called before actor stops
  async fn pre_stop(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
    Ok(())
  }
  
  /// Called after actor stops
  async fn post_stop(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
    Ok(())
  }
}

/// Basic actor errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum BaseActorError {
  #[error("Actor error: {0}")]
  ActorError(String),
  
  #[error("Message handling error: {0}")]
  MessageError(String),
  
  #[error("System error: {0}")]
  SystemError(String),
}