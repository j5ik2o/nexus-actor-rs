//! A crate that provides actor system implementation for the Tokio asynchronous runtime.
//!
//! This crate provides components such as mailboxes, timers, and spawners
//! that run on the Tokio runtime, making the functionality of `nexus-actor-core-rs`
//! available in standard asynchronous runtime environments.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![deny(clippy::redundant_clone)]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enum)]
#![deny(clippy::no_effect)]
#![deny(clippy::drop_copy)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::wrong_pub_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]

/// A failure event bridge module utilizing Tokio's broadcast channel.
#[cfg(any(feature = "rt-multi-thread", feature = "rt-current-thread"))]
pub mod failure_event_bridge;
mod failure_event_hub;
mod receive_timeout;
mod runtime_driver;
mod spawn;
mod timer;
mod tokio_mailbox;
mod tokio_priority_mailbox;

pub use failure_event_hub::{FailureEventHub, FailureEventSubscription};
pub use nexus_utils_std_rs::{ArcShared, ArcStateCell, Shared, SharedFactory, SharedFn};
pub use receive_timeout::TokioReceiveTimeoutSchedulerFactory;
pub use runtime_driver::TokioSystemHandle;
pub use spawn::TokioSpawner;
pub use timer::TokioTimer;
pub use tokio_mailbox::{TokioMailbox, TokioMailboxFactory, TokioMailboxSender};
pub use tokio_priority_mailbox::{TokioPriorityMailbox, TokioPriorityMailboxFactory, TokioPriorityMailboxSender};

use nexus_actor_core_rs::{ActorSystemConfig, ReceiveTimeoutFactoryShared};

/// A prelude module that provides commonly used re-exported types and traits.
pub mod prelude {
  pub use super::{
    ArcShared, ArcStateCell, Shared, SharedFactory, SharedFn, TokioMailbox, TokioMailboxFactory, TokioMailboxSender,
    TokioPriorityMailbox, TokioPriorityMailboxFactory, TokioPriorityMailboxSender, TokioSpawner, TokioSystemHandle,
    TokioTimer,
  };
  pub use nexus_actor_core_rs::actor_loop;
}

/// Configures a receive timeout scheduler within an `ActorSystemConfig`.
///
/// Call this helper before `ActorSystem::new_with_config` to enable receive timeout
/// handling using Tokio timers.
pub fn install_receive_timeout_scheduler(config: &mut ActorSystemConfig<TokioMailboxFactory>) {
  config.receive_timeout_factory = Some(ReceiveTimeoutFactoryShared::new(
    TokioReceiveTimeoutSchedulerFactory::new(),
  ));
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::time::Duration;
  use nexus_actor_core_rs::MailboxOptions;
  use nexus_actor_core_rs::{actor_loop, ActorSystem, Context, Props, Spawn, StateCell, SystemMessage};
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

  async fn run_receive_timeout_triggers() {
    let factory = TokioMailboxFactory;
    let mut config = ActorSystemConfig::default();
    install_receive_timeout_scheduler(&mut config);
    let mut system: ActorSystem<u32, _> = ActorSystem::new_with_config(factory, config);

    let timeout_log: Arc<Mutex<Vec<SystemMessage>>> = Arc::new(Mutex::new(Vec::new()));
    let props = Props::with_system_handler(
      MailboxOptions::default(),
      move |ctx: &mut Context<'_, '_, u32, TokioMailboxFactory>, msg| {
        if msg == 1 {
          ctx.set_receive_timeout(Duration::from_millis(10));
        }
      },
      Some({
        let timeout_clone = timeout_log.clone();
        move |_: &mut Context<'_, '_, u32, TokioMailboxFactory>, sys: SystemMessage| {
          if matches!(sys, SystemMessage::ReceiveTimeout) {
            timeout_clone.lock().unwrap().push(sys);
          }
        }
      }),
    );

    let mut root = system.root_context();
    let actor_ref = root.spawn(props).expect("spawn receive-timeout actor");

    actor_ref.tell(1).expect("tell");
    root.dispatch_next().await.expect("dispatch user");

    tokio::time::sleep(Duration::from_millis(30)).await;
    root.dispatch_next().await.expect("dispatch timeout");

    let log = timeout_log.lock().unwrap();
    assert!(!log.is_empty(), "ReceiveTimeout が少なくとも 1 回は発火する想定");
    assert!(
      log.iter().all(|sys| matches!(sys, SystemMessage::ReceiveTimeout)),
      "ReceiveTimeout 以外のシグナルは届かない想定"
    );
  }

  #[tokio::test(flavor = "current_thread")]
  async fn typed_actor_system_handles_user_messages() {
    run_typed_actor_system_handles_user_messages().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn typed_actor_system_handles_user_messages_multi_thread() {
    run_typed_actor_system_handles_user_messages().await;
  }

  #[tokio::test(flavor = "current_thread")]
  async fn receive_timeout_triggers() {
    run_receive_timeout_triggers().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn receive_timeout_triggers_multi_thread() {
    run_receive_timeout_triggers().await;
  }
}
