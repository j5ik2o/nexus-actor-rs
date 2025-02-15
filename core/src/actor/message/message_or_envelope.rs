use std::any::Any;
use std::fmt::Debug;
use crate::actor::message::{Message, MessageHeaders};
use crate::actor::pid::Pid;

#[derive(Debug, Clone)]
pub struct MessageOrEnvelope {
    pub(crate) message: Box<dyn Message>,
    pub(crate) header: Option<MessageHeaders>,
    pub(crate) sender: Option<Pid>,
}

impl Message for MessageOrEnvelope {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MessageOrEnvelope {
    pub fn new(message: Box<dyn Message>) -> Self {
        Self {
            message,
            header: None,
            sender: None,
        }
    }

    pub fn with_header(mut self, header: MessageHeaders) -> Self {
        self.header = Some(header);
        self
    }

    pub fn with_sender(mut self, sender: Pid) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn get_message(&self) -> &dyn Message {
        &*self.message
    }

    pub fn into_message(self) -> Box<dyn Message> {
        self.message
    }

    pub fn get_sender(&self) -> Option<&Pid> {
        self.sender.as_ref()
    }

    pub fn get_header(&self) -> Option<&MessageHeaders> {
        self.header.as_ref()
    }
}
