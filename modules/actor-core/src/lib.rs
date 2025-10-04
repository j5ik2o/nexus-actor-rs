#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::time::Duration;

mod mailbox;
mod shared;
mod spawn;
mod timer;

pub use mailbox::Mailbox;
pub use shared::Shared;
pub use spawn::Spawn;
pub use timer::Timer;

/// Minimal actor loop that waits for messages, handles them, and yields control.
///
/// This is a reference implementation shared by both std and embedded runtimes.
pub async fn actor_loop<M, MB, T, F>(mailbox: &MB, timer: &T, mut handler: F)
where
  MB: Mailbox<M>,
  T: Timer,
  F: FnMut(M), {
  loop {
    let message = mailbox.recv().await;
    handler(message);
    timer.sleep(Duration::from_millis(0)).await;
  }
}
