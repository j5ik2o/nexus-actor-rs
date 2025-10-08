#[cfg(any(feature = "rt-multi-thread", feature = "rt-current-thread"))]
pub mod failure_event_bridge;
mod failure_event_hub;
mod runtime_driver;
mod spawn;
mod timer;
mod tokio_mailbox;
mod tokio_priority_mailbox;

pub use failure_event_hub::{FailureEventHub, FailureEventSubscription};
pub use nexus_utils_std_rs::{ArcShared, ArcStateCell};
pub use runtime_driver::TokioSystemHandle;
pub use spawn::TokioSpawner;
pub use timer::TokioTimer;
pub use tokio_mailbox::{TokioMailbox, TokioMailboxFactory, TokioMailboxSender};
pub use tokio_priority_mailbox::{TokioPriorityMailbox, TokioPriorityMailboxFactory, TokioPriorityMailboxSender};

pub mod prelude {
  pub use super::{
    ArcShared, ArcStateCell, TokioMailbox, TokioMailboxFactory, TokioMailboxSender, TokioPriorityMailbox,
    TokioPriorityMailboxFactory, TokioPriorityMailboxSender, TokioSpawner, TokioSystemHandle, TokioTimer,
  };
  pub use nexus_actor_core_rs::actor_loop;
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::time::Duration;
  use nexus_actor_core_rs::MailboxOptions;
  use nexus_actor_core_rs::{actor_loop, ActorSystem, Props, Spawn, StateCell};
  use std::sync::{Arc, Mutex};

  async fn run_test_actor_loop_updates_state() {
    let (mailbox, sender) = TokioMailbox::new(8);
    let mailbox = Arc::new(mailbox);
    let state = ArcStateCell::new(0_u32);

    let actor_state = state.clone();
    let actor_mailbox = mailbox.clone();

    let spawner = TokioSpawner;

    spawner.spawn(async move {
      let timer = TokioTimer;
      actor_loop(actor_mailbox.as_ref(), &timer, move |msg: u32| {
        let mut guard = actor_state.borrow_mut();
        *guard += msg;
      })
      .await;
    });

    sender.send(4_u32).await.expect("send message");
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert_eq!(*state.borrow(), 4_u32);
  }

  #[tokio::test(flavor = "current_thread")]
  async fn test_actor_loop_updates_state() {
    run_test_actor_loop_updates_state().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn test_actor_loop_updates_state_multi_thread() {
    run_test_actor_loop_updates_state().await;
  }

  async fn run_typed_actor_system_handles_user_messages() {
    let factory = TokioMailboxFactory;
    let mut system: ActorSystem<u32, _> = ActorSystem::new(factory);

    let log: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let log_clone = log.clone();

    let props = Props::new(MailboxOptions::default(), move |_, msg: u32| {
      log_clone.lock().unwrap().push(msg);
    });

    let mut root = system.root_context();
    let actor_ref = root.spawn(props).expect("spawn typed actor");

    actor_ref.tell(99).expect("tell");
    root.dispatch_next().await.expect("dispatch next");

    assert_eq!(log.lock().unwrap().as_slice(), &[99]);
  }

  #[tokio::test(flavor = "current_thread")]
  async fn typed_actor_system_handles_user_messages() {
    run_typed_actor_system_handles_user_messages().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn typed_actor_system_handles_user_messages_multi_thread() {
    run_typed_actor_system_handles_user_messages().await;
  }
}
