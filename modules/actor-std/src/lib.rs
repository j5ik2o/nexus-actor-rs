mod mailbox;
mod spawn;
mod state;
mod timer;

pub use mailbox::TokioMailbox;
pub use spawn::TokioSpawner;
pub use state::ArcStateCell;
pub use timer::TokioTimer;

pub mod prelude {
  pub use super::{ArcStateCell, TokioMailbox, TokioSpawner, TokioTimer};
  pub use nexus_actor_core_rs::actor_loop;
}

#[cfg(test)]
mod tests {
  use super::*;
  use core::time::Duration;
  use nexus_actor_core_rs::{actor_loop, Spawn, StateCell};
  use std::sync::Arc;

  #[tokio::test(flavor = "current_thread")]
  async fn test_actor_loop_updates_state() {
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
}
