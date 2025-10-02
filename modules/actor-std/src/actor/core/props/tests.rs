use super::*;
use crate::actor::dispatch;
use crate::actor::dispatch::MailboxProducer;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
struct TestActor;

#[async_trait]
impl Actor for TestActor {
  async fn handle(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[tokio::test]
async fn test_core_props_mailbox_factory_creates_core_mailbox() {
  let invocation_count = Arc::new(AtomicUsize::new(0));
  let base_producer = dispatch::unbounded_mailbox_creator();
  let counting_producer = MailboxProducer::new({
    let invocation_count = invocation_count.clone();
    let base_producer = base_producer.clone();
    move || {
      let invocation_count = invocation_count.clone();
      let base_producer = base_producer.clone();
      async move {
        invocation_count.fetch_add(1, Ordering::SeqCst);
        base_producer.run().await
      }
    }
  });

  let props =
    Props::from_sync_actor_producer_with_opts(|_| TestActor, [Props::with_mailbox_producer(counting_producer)]).await;
  let core_props = props.core_props();
  let factory = core_props.mailbox_factory().expect("mailbox_factory not set");

  let core_mailbox = factory().await;

  assert_eq!(invocation_count.load(Ordering::SeqCst), 1);

  core_mailbox.start().await;
  core_mailbox.process_messages().await;
  let user_count = core_mailbox.user_messages_count().await;
  let system_count = core_mailbox.system_messages_count().await;

  assert_eq!(user_count, 0);
  assert_eq!(system_count, 0);
}
