mod messages;
mod queue_mailbox;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(test)]
mod tests;
pub mod traits;

#[cfg(any(test, feature = "test-support"))]
pub use messages::PriorityChannel;
pub use messages::{PriorityEnvelope, SystemMessage};
pub use queue_mailbox::{MailboxOptions, QueueMailbox, QueueMailboxProducer, QueueMailboxRecv};
pub use traits::{Mailbox, MailboxFactory, MailboxPair, MailboxSignal};
