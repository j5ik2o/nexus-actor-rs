#[cfg(test)]
mod test {
  use std::sync::Arc;

  use crate::actor::actor::ActorError;
  use crate::actor::actor::ActorInnerError;
  use crate::actor::actor::Task;
  use crate::actor::dispatch::default_mailbox::DefaultMailbox;
  use crate::actor::dispatch::dispatcher::{CurrentThreadDispatcher, DispatcherHandle};
  use crate::actor::dispatch::mailbox::Mailbox;
  use crate::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
  use crate::actor::message::Message;
  use crate::actor::message::MessageHandle;
  use crate::util::queue::MpscUnboundedChannelQueue;
  use async_trait::async_trait;
  use nexus_acto_message_derive_rs::Message;
  use tokio::sync::Mutex;

  // TestMessageInvoker implementation
  #[derive(Debug, Clone, PartialEq)]
  enum ReceivedMessage {
    System,
    User,
    Task,
    Failure(String),
  }

  #[derive(Debug)]
  struct TestMessageInvoker {
    received: Arc<Mutex<Vec<ReceivedMessage>>>,
  }

  impl TestMessageInvoker {
    fn new() -> Self {
      TestMessageInvoker {
        received: Arc::new(Mutex::new(Vec::new())),
      }
    }

    async fn get_received(&self) -> Vec<ReceivedMessage> {
      self.received.lock().await.clone()
    }
  }

  #[async_trait]
  impl MessageInvoker for TestMessageInvoker {
    async fn invoke_system_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
      self.received.lock().await.push(ReceivedMessage::System);
      Ok(())
    }

    async fn invoke_user_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError> {
      if message_handle.is_typed::<TestTask>() {
        self.received.lock().await.push(ReceivedMessage::Task);
      } else {
        self.received.lock().await.push(ReceivedMessage::User);
      }
      Ok(())
    }

    async fn escalate_failure(&mut self, reason: ActorInnerError, _: MessageHandle) {
      let reason_msg = if reason.is_type::<&str>() {
        reason.clone().take::<&str>().unwrap().to_string()
      } else if reason.is_type::<String>() {
        reason.clone().take::<String>().unwrap()
      } else {
        "Unknown error".to_string()
      };
      self.received.lock().await.push(ReceivedMessage::Failure(reason_msg));
    }
  }

  // Test message types
  #[derive(Debug, Clone, PartialEq, Eq, Message)]
  struct TestSystemMessage;

  #[derive(Debug, Clone, PartialEq, Eq, Message)]
  struct TestUserMessage;

  #[derive(Debug, Clone, PartialEq, Eq, Message)]
  struct TestTask;

  #[async_trait]
  impl Task for TestTask {
    async fn run(&self) {}
  }

  #[tokio::test]
  async fn test_mailbox_with_test_invoker() {
    let mut mailbox = DefaultMailbox::new(MpscUnboundedChannelQueue::new(), MpscUnboundedChannelQueue::new());
    let invoker = Arc::new(Mutex::new(TestMessageInvoker::new()));
    let invoker_handle = MessageInvokerHandle::new(invoker.clone());
    let dispatcher = Arc::new(CurrentThreadDispatcher::new().unwrap());
    let dispatcher_handle = DispatcherHandle::new_arc(dispatcher.clone());
    mailbox
      .register_handlers(Some(invoker_handle.clone()), Some(dispatcher_handle.clone()))
      .await;

    let system_message = MessageHandle::new(TestSystemMessage);
    let user_message = MessageHandle::new(TestUserMessage);
    let task = MessageHandle::new(TestTask);

    mailbox.post_system_message(system_message).await;
    mailbox.post_user_message(user_message).await;
    mailbox.post_user_message(task).await;

    // メッセージが処理されるのを少し待つ
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let invoker_mg = invoker.lock().await;
    let received = invoker_mg.get_received().await;
    assert_eq!(received.len(), 3);
    assert_eq!(received[0], ReceivedMessage::System);
    assert_eq!(received[1], ReceivedMessage::User);
    assert_eq!(received[2], ReceivedMessage::Task);
  }
}
