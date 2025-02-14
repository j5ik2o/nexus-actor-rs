use crate::actor::message::Message;
use crate::generated::actor::DeadLetterResponse;
use std::any::Any;

impl Message for DeadLetterResponse {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<DeadLetterResponse>();
    match (self, msg) {
      (DeadLetterResponse { target: self_target }, Some(DeadLetterResponse { target: other_target })) => {
        self_target == other_target
      }
      _ => false,
    }
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn message_type(&self) -> &'static str {
    "DeadLetterResponse"
  }
}
