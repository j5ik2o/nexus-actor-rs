#[cfg(test)]
mod tests {
  use crate::actor::ActorError;
  use crate::actor::ErrorReason;
  use crate::actor::dispatch::bounded::BoundedMailboxQueue;
  use crate::actor::dispatch::dispatcher::{DispatcherHandle, TokioRuntimeContextDispatcher};
  use crate::actor::dispatch::mailbox::Mailbox;
  use crate::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
  use crate::actor::dispatch::unbounded::unbounded_mpsc_mailbox_creator;
  use crate::actor::message::MessageHandle;
  use async_trait::async_trait;
  use nexus_actor_utils_rs::collections::{QueueReader, QueueWriter, RingQueue};
  use rand::rngs::SmallRng;
  use rand::Rng;
  use rand::SeedableRng;
  use std::env;
  use std::sync::Arc;
  use std::time::Duration;
  use tokio::sync::RwLock;
  use tokio::time::sleep;
  use tracing_subscriber::EnvFilter;

  #[derive(Debug)]
  struct TestMessageInvoker {
    count: usize,
    max: usize,
    assert_flg: bool,
  }

  impl TestMessageInvoker {
    fn new(max: usize) -> Self {
      Self {
        count: 0,
        max,
        assert_flg: false,
      }
    }

    fn get_count(&self) -> usize {
      self.count
    }

    fn is_assert_flg(&self) -> bool {
      self.assert_flg
    }
  }

  #[async_trait]
  impl MessageInvoker for TestMessageInvoker {
    async fn invoke_system_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
      self.count += 1;
      if self.count == self.max {
        self.assert_flg = true;
      }
      Ok(())
    }

    async fn invoke_user_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
      self.count += 1;
      if self.count == self.max {
        self.assert_flg = true;
      }
      Ok(())
    }

    async fn escalate_failure(&mut self, _: ErrorReason, _: MessageHandle) {}
  }

  #[tokio::test]
  async fn test_unbounded_mpsc_mailbox_user_message_consistency() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let max = 100;
    let c = 10;

    let mbox_producer = unbounded_mpsc_mailbox_creator();
    let message_invoker = Arc::new(RwLock::new(TestMessageInvoker::new(max)));
    let mut mailbox = mbox_producer.run().await;

    let dispatcher = TokioRuntimeContextDispatcher::new().unwrap();

    mailbox
      .register_handlers(
        Some(MessageInvokerHandle::new(message_invoker.clone())),
        Some(DispatcherHandle::new(dispatcher.clone())),
      )
      .await;

    let mut join_handles = Vec::new();
    let mut thread_rng = rand::rng();
    let rng = SmallRng::from_rng(&mut thread_rng);

    for j in 0..c {
      let cmax = max / c;
      let mailbox = mailbox.clone();
      let mut rng = rng.clone();

      let h = tokio::spawn(async move {
        for i in 0..cmax {
          if rng.random_range(0..10) == 0 {
            let wait_time = rng.random_range(0..1000);
            sleep(Duration::from_millis(wait_time)).await;
          }
          mailbox
            .post_user_message(MessageHandle::new(format!("{} {}", j, i)))
            .await;
        }
      });
      join_handles.push(h);
    }

    for h in join_handles {
      h.await.unwrap();
    }

    sleep(Duration::from_secs(1)).await;

    {
      let mg = message_invoker.read().await;
      assert_eq!(mg.get_count(), max);
      assert!(mg.is_assert_flg());
    }
  }

  #[tokio::test]
  async fn test_unbounded_mpsc_mailbox_system_message_consistency() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let max = 100;
    let c = 10;

    let mbox_producer = unbounded_mpsc_mailbox_creator();
    let message_invoker = Arc::new(RwLock::new(TestMessageInvoker::new(max)));
    let mut mailbox = mbox_producer.run().await;

    let dispatcher = TokioRuntimeContextDispatcher::new().unwrap();

    mailbox
      .register_handlers(
        Some(MessageInvokerHandle::new(message_invoker.clone())),
        Some(DispatcherHandle::new(dispatcher.clone())),
      )
      .await;

    let mut join_handles = Vec::new();
    let mut thread_rng = rand::rng();
    let rng = SmallRng::from_rng(&mut thread_rng);

    for j in 0..c {
      let cmax = max / c;
      let mailbox = mailbox.clone();
      let mut rng = rng.clone();

      let h = tokio::spawn(async move {
        for i in 0..cmax {
          if rng.random_range(0..10) == 0 {
            let wait_time = rng.random_range(0..1000);
            sleep(Duration::from_millis(wait_time)).await;
          }
          mailbox
            .post_system_message(MessageHandle::new(format!("{} {}", j, i)))
            .await;
        }
      });
      join_handles.push(h);
    }

    for h in join_handles {
      h.await.unwrap();
    }

    sleep(Duration::from_secs(1)).await;

    {
      let mg = message_invoker.read().await;
      assert_eq!(mg.get_count(), max);
      assert!(mg.is_assert_flg());
    }
  }

  #[tokio::test]
  async fn test_bounded_mailbox() {
    let size = 3;
    let mut m = BoundedMailboxQueue::new(RingQueue::new(size), size, false);
    m.offer(MessageHandle::new("1".to_string())).await.unwrap();
    m.offer(MessageHandle::new("2".to_string())).await.unwrap();
    m.offer(MessageHandle::new("3".to_string())).await.unwrap();
    let result = m.poll().await.unwrap();
    let value = result.unwrap().to_typed::<String>().unwrap();
    assert_eq!(value, "1".to_string());
  }

  #[tokio::test]
  async fn test_bounded_dropping_mailbox() {
    let size = 3;
    let mut m = BoundedMailboxQueue::new(RingQueue::new(size), size, true);
    m.offer(MessageHandle::new("1".to_string())).await.unwrap();
    m.offer(MessageHandle::new("2".to_string())).await.unwrap();
    m.offer(MessageHandle::new("3".to_string())).await.unwrap();
    m.offer(MessageHandle::new("4".to_string())).await.unwrap();
    let result = m.poll().await.unwrap();
    let value = result.unwrap().to_typed::<String>().unwrap();
    assert_eq!(value, "2".to_string());
  }
}
