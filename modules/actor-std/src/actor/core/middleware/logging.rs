use crate::actor::context::ReceiverSnapshot;
use crate::actor::core::ReceiverMiddleware;

pub struct Logger;

impl Logger {
  pub fn of_receiver() -> ReceiverMiddleware {
    ReceiverMiddleware::from_sync(|snapshot: ReceiverSnapshot| {
      let message_handle = snapshot.message().get_message_handle();
      tracing::info!("Actor got message: {:?}", message_handle);
      snapshot
    })
  }
}
