mod messages;
mod queue_mailbox;
mod traits;

pub use messages::{PriorityChannel, PriorityEnvelope, SystemMessage};
pub use queue_mailbox::{MailboxOptions, QueueMailbox, QueueMailboxProducer, QueueMailboxRecv};
pub use traits::{Mailbox, MailboxPair, MailboxRuntime, MailboxSignal};

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
