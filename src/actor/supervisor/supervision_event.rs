use std::any::Any;
use std::sync::Arc;

use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor_system::ActorSystem;
use crate::actor::log::P_LOG;
use crate::actor::message::message::Message;
use crate::actor::supervisor::directive::Directive;
use crate::event_stream::event_handler::EventHandler;
use crate::event_stream::subscription::Subscription;
use crate::log::log_field::LogField;

#[derive(Debug, Clone)]
pub struct SupervisorEvent {
  pub child: ExtendedPid,
  pub reason: ActorInnerError,
  pub directive: Directive,
}

impl Message for SupervisorEvent {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<SupervisorEvent>()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

pub async fn subscribe_supervision(actor_system: &ActorSystem) -> Subscription {
  actor_system
    .get_event_stream()
    .await
    .subscribe(move |evt| {
      let evt = evt.as_any().downcast_ref::<SupervisorEvent>().cloned().map(Arc::new);
      async move {
        if let Some(supervisor_event) = evt {
          P_LOG
            .debug_with_fields(
              "[SUPERVISION]",
              [
                LogField::display("actor", supervisor_event.child.clone()),
                LogField::display("directive", supervisor_event.directive.clone()),
                LogField::display("reason", supervisor_event.reason.clone()),
              ],
            )
            .await;
        }
      }
    })
    .await
}
