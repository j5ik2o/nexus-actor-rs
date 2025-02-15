//! Mailbox queue trait and implementations.

use std::fmt::Debug;
use async_trait::async_trait;

use crate::actor::message::MessageHandle;
use nexus_actor_utils_rs::collections::{Element, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

#[async_trait]
pub trait MailboxQueue: QueueBase<MessageHandle> + QueueReader<MessageHandle> + QueueWriter<MessageHandle> + Send + Sync + Debug {
    async fn capacity(&self) -> QueueSize;
    async fn len(&self) -> QueueSize;
    async fn is_empty(&self) -> bool;
    async fn poll(&mut self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>>;
    async fn offer(&mut self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>>;
}

impl Element for MessageHandle {}
