#![no_std]

extern crate alloc;

#[cfg(feature = "embedded_arc")]
mod arc_mailbox;
#[cfg(feature = "embedded_arc")]
mod arc_priority_mailbox;
mod local_mailbox;
mod spawn;
mod timer;

#[cfg(feature = "embedded_arc")]
pub use arc_mailbox::{ArcMailbox, ArcMailboxRuntime, ArcMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use arc_priority_mailbox::{ArcPriorityMailbox, ArcPriorityMailboxRuntime, ArcPriorityMailboxSender};
pub use local_mailbox::{LocalMailbox, LocalMailboxRuntime, LocalMailboxSender};
#[cfg(feature = "embedded_arc")]
pub use nexus_utils_embedded_rs::sync::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
#[cfg(feature = "embedded_rc")]
pub use nexus_utils_embedded_rs::sync::{RcShared, RcStateCell};
pub use spawn::ImmediateSpawner;
pub use timer::ImmediateTimer;

pub mod prelude {
  #[cfg(feature = "embedded_arc")]
  pub use super::{
    ArcCsStateCell, ArcLocalStateCell, ArcMailbox, ArcMailboxRuntime, ArcMailboxSender, ArcPriorityMailbox,
    ArcPriorityMailboxRuntime, ArcPriorityMailboxSender, ArcShared, ArcStateCell,
  };
  pub use super::{ImmediateSpawner, ImmediateTimer, LocalMailbox, LocalMailboxRuntime, LocalMailboxSender};
  #[cfg(feature = "embedded_rc")]
  pub use super::{RcShared, RcStateCell};
}
