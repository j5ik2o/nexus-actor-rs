#![no_std]

extern crate alloc;

#[cfg(feature = "embedded_arc")]
mod arc_mailbox;
mod local_mailbox;
mod spawn;
mod timer;

#[cfg(feature = "embedded_arc")]
pub use arc_mailbox::{ArcMailbox, ArcMailboxSender};
pub use local_mailbox::{LocalMailbox, LocalMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use nexus_utils_embedded_rs::sync::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
#[cfg(feature = "embedded_rc")]
pub use nexus_utils_embedded_rs::sync::{RcShared, RcStateCell};
pub use spawn::ImmediateSpawner;
pub use timer::ImmediateTimer;

pub mod prelude {
  #[cfg(feature = "embedded_arc")]
  pub use super::{ArcCsStateCell, ArcLocalStateCell, ArcMailbox, ArcMailboxSender, ArcShared, ArcStateCell};
  pub use super::{ImmediateSpawner, ImmediateTimer, LocalMailbox, LocalMailboxSender};
  #[cfg(feature = "embedded_rc")]
  pub use super::{RcShared, RcStateCell};
}
