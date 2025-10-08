mod traits;
mod messages;
mod queue_mailbox;

pub use traits::{Mailbox, MailboxPair, MailboxRuntime, MailboxSignal};
pub use messages::{PriorityChannel, PriorityEnvelope, SystemMessage};
pub use queue_mailbox::{MailboxOptions, QueueMailbox, QueueMailboxProducer, QueueMailboxRecv};

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
