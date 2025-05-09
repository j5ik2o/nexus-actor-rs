use crate::actor::context::ReceiverContextHandle;
use crate::actor::core::{ReceiverMiddleware, ReceiverMiddlewareChain};
use crate::actor::message::MessageEnvelope;

pub struct Logger;

impl Logger {
  pub fn of_receiver() -> ReceiverMiddleware {
    ReceiverMiddleware::new(|next| {
      ReceiverMiddlewareChain::new(move |context_handle: ReceiverContextHandle, env: MessageEnvelope| {
        let cloned_next = next.clone();
        async move {
          let message_handle = env.get_message_handle();
          tracing::info!("Actor got message: {:?}", message_handle);
          cloned_next.run(context_handle.clone(), env.clone()).await
        }
      })
    })
  }
}
