use crate::actor::message::Message;

pub trait Task: Message {
  fn run(&self);
}
