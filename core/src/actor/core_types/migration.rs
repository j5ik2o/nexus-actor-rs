use crate::actor::context::{ContextHandle, MessagePart};
use crate::actor::core::{Actor, ActorError, Props};
use crate::actor::core_types::{BaseActor, BaseContext, ContextAdapter, ActorBridge};
use async_trait::async_trait;
use std::fmt::Debug;

/// Helper trait for migrating from old Actor to new BaseActor
pub trait MigrateToBaseActor: BaseActor {
    /// Convert this BaseActor into a traditional Actor
    fn into_actor(self) -> MigratedActor<Self>
    where
        Self: Sized,
    {
        MigratedActor::new(self)
    }
}

/// Automatically implement MigrateToBaseActor for all BaseActors
impl<T: BaseActor> MigrateToBaseActor for T {}

/// Wrapper that adapts a BaseActor to work as a traditional Actor
#[derive(Debug)]
pub struct MigratedActor<B: BaseActor> {
    base_actor: B,
}

impl<B: BaseActor> MigratedActor<B> {
    pub fn new(base_actor: B) -> Self {
        Self { base_actor }
    }
    
    /// Get a reference to the wrapped BaseActor
    pub fn get_base_actor(&self) -> &B {
        &self.base_actor
    }
    
    /// Get a mutable reference to the wrapped BaseActor
    pub fn get_base_actor_mut(&mut self) -> &mut B {
        &mut self.base_actor
    }
}

#[async_trait]
impl<B: BaseActor> Actor for MigratedActor<B> {
    async fn handle(&mut self, context: ContextHandle) -> Result<(), ActorError> {
        use crate::actor::message::AutoReceiveMessage;
        
        let message_handle = context.get_message_handle().await;
        let arm = message_handle.to_typed::<AutoReceiveMessage>();
        
        let base_context = self.adapt_context(context.clone());
        
        match arm {
            Some(arm) => match arm {
                AutoReceiveMessage::PreStart => {
                    self.base_actor
                        .pre_start(base_context.as_ref())
                        .await
                        .map_err(|e| ActorError::ReceiveError(crate::actor::core::ErrorReason::new(e.to_string(), 0)))?;
                    Ok(())
                }
                AutoReceiveMessage::PostStart => {
                    self.base_actor
                        .post_start(base_context.as_ref())
                        .await
                        .map_err(|e| ActorError::ReceiveError(crate::actor::core::ErrorReason::new(e.to_string(), 0)))?;
                    Ok(())
                }
                AutoReceiveMessage::PreStop => {
                    self.base_actor
                        .pre_stop(base_context.as_ref())
                        .await
                        .map_err(|e| ActorError::ReceiveError(crate::actor::core::ErrorReason::new(e.to_string(), 0)))?;
                    Ok(())
                }
                AutoReceiveMessage::PostStop => {
                    self.base_actor
                        .post_stop(base_context.as_ref())
                        .await
                        .map_err(|e| ActorError::ReceiveError(crate::actor::core::ErrorReason::new(e.to_string(), 0)))?;
                    Ok(())
                }
                _ => self.receive(context).await,
            },
            _ => self.receive(context).await,
        }
    }
    
    async fn receive(&mut self, context: ContextHandle) -> Result<(), ActorError> {
        let base_context = self.adapt_context(context);
        
        self.base_actor
            .handle(base_context.as_ref())
            .await
            .map_err(|e| ActorError::ReceiveError(crate::actor::core::ErrorReason::new(e.to_string(), 0)))?;
        
        Ok(())
    }
}

impl<B: BaseActor> ActorBridge for MigratedActor<B> {}

/// Helper functions for creating Props from BaseActors
pub struct MigrationHelpers;

impl MigrationHelpers {
    /// Create Props from a BaseActor instance
    pub async fn props_from_base_actor<B: BaseActor + 'static>(base_actor: B) -> Props {
        use std::cell::RefCell;
        
        // Thread-local storage for the actor
        thread_local! {
            static ACTOR_STORAGE: RefCell<Option<Box<dyn std::any::Any>>> = RefCell::new(None);
        }
        
        // Store the actor in thread-local storage temporarily
        ACTOR_STORAGE.with(|storage| {
            *storage.borrow_mut() = Some(Box::new(base_actor));
        });
        
        Props::from_async_actor_receiver(|ctx| {
            // Retrieve the actor from thread-local storage
            let base_actor = ACTOR_STORAGE.with(|storage| {
                storage.borrow_mut().take()
                    .and_then(|boxed| boxed.downcast::<B>().ok())
                    .map(|boxed| *boxed)
            });
            
            async move {
                if let Some(base_actor) = base_actor {
                    let mut actor = MigratedActor::new(base_actor);
                    actor.handle(ctx).await
                } else {
                    Err(ActorError::ReceiveError(crate::actor::core::ErrorReason::new(
                        "Failed to retrieve BaseActor from storage".to_string(),
                        0,
                    )))
                }
            }
        })
        .await
    }
    
    /// Create Props from a function that creates a BaseActor
    pub async fn props_from_base_actor_fn<F, B>(factory: F) -> Props
    where
        F: Fn() -> B + Send + Sync + 'static,
        B: BaseActor + 'static,
    {
        Props::from_async_actor_receiver(move |ctx| {
            let base_actor = factory();
            let mut actor = MigratedActor::new(base_actor);
            async move { actor.handle(ctx).await }
        })
        .await
    }
}

/// Extension trait for ContextHandle to easily get a BaseContext
pub trait ContextHandleExt {
    /// Convert this ContextHandle to a BaseContext
    fn as_base_context(&self) -> Box<dyn BaseContext>;
}

impl ContextHandleExt for ContextHandle {
    fn as_base_context(&self) -> Box<dyn BaseContext> {
        Box::new(ContextAdapter::new(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::actor_system::ActorSystem;
    use crate::actor::context::{SpawnerPart, SenderPart};
    use crate::actor::core_types::{BaseActorError, Message};
    use crate::actor::message::MessageHandle;
    use std::any::Any;

    #[derive(Debug, Clone)]
    struct TestMessage(String);

    impl Message for TestMessage {
        fn eq_message(&self, other: &dyn Message) -> bool {
            if let Some(other_msg) = other.as_any().downcast_ref::<TestMessage>() {
                self.0 == other_msg.0
            } else {
                false
            }
        }

        fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
            self
        }

        fn get_type_name(&self) -> String {
            "TestMessage".to_string()
        }
    }

    #[derive(Debug)]
    struct TestBaseActor {
        received_count: usize,
    }

    #[async_trait]
    impl BaseActor for TestBaseActor {
        async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
            let msg = context.get_message().await;
            if let Some(_test_msg) = msg.to_typed::<TestMessage>() {
                self.received_count += 1;
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_base_actor_migration() {
        let actor_system = ActorSystem::new().await.unwrap();
        
        // Create a BaseActor
        let base_actor = TestBaseActor { received_count: 0 };
        
        // Migrate it to traditional Props
        let props = MigrationHelpers::props_from_base_actor(base_actor).await;
        
        // Spawn using traditional system
        let mut root_context = actor_system.get_root_context().await;
        let pid = root_context.spawn(props).await;
        
        // Send a message
        let msg = MessageHandle::new(TestMessage("Hello".to_string()));
        root_context.send(pid.clone(), msg).await;
        
        // Give some time for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        // The test passes if no panic occurs
    }
}