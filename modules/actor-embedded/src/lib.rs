#![no_std]

extern crate alloc;

mod mailbox;
#[cfg(feature = "embedded_arc")]
mod mailbox_arc;
mod spawn;
mod timer;

pub use mailbox::{LocalMailbox, LocalMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use mailbox_arc::{ArcMailbox, ArcMailboxSender};
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
