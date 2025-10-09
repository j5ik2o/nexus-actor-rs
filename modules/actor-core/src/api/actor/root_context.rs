use crate::runtime::message::DynMessage;
use crate::runtime::system::InternalRootContext;
use crate::{ActorRef, MailboxFactory, PriorityEnvelope, Props};
use alloc::boxed::Box;
use core::future::Future;
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError};

use super::{ask_with_timeout, AskFuture, AskResult, AskTimeoutFuture};

/// Context for operating root actors.
///
/// Performs actor spawning and message sending from the top level of the actor system.
/// Manages failure handling of child actors through guardian strategies.
pub struct RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>, {
  pub(crate) inner: InternalRootContext<'a, DynMessage, R, Strat>,
  pub(crate) _marker: PhantomData<U>,
}

impl<'a, U, R, Strat> RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>,
{
  /// Spawns a new actor using the specified properties.
  ///
  /// # Arguments
  ///
  /// * `props` - Properties to use for spawning the actor
  ///
  /// # Returns
  ///
  /// Reference to the spawned actor, or a mailbox error
  pub fn spawn(&mut self, props: Props<U, R>) -> Result<ActorRef<U, R>, QueueError<PriorityEnvelope<DynMessage>>> {
    let (internal_props, supervisor_cfg) = props.into_parts();
    let actor_ref = self
      .inner
      .spawn_with_supervisor(Box::new(supervisor_cfg.into_supervisor()), internal_props)?;
    Ok(ActorRef::new(actor_ref))
  }

  /// Sends a message to the specified actor and returns a Future that waits for a response.
  ///
  /// # Arguments
  ///
  /// * `target` - Target actor to send the message to
  /// * `message` - Message to send
  ///
  /// # Returns
  ///
  /// Future for receiving the response, or an error
  pub fn request_future<V, Resp>(&self, target: &ActorRef<V, R>, message: V) -> AskResult<AskFuture<Resp>>
  where
    V: Element,
    Resp: Element, {
    target.request_future(message)
  }

  /// Sends a message to the specified actor and returns a Future that waits for a response with timeout.
  ///
  /// # Arguments
  ///
  /// * `target` - Target actor to send the message to
  /// * `message` - Message to send
  /// * `timeout` - Future indicating timeout
  ///
  /// # Returns
  ///
  /// Future for receiving the response with timeout, or an error
  pub fn request_future_with_timeout<V, Resp, TFut>(
    &self,
    target: &ActorRef<V, R>,
    message: V,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    V: Element,
    Resp: Element,
    TFut: Future<Output = ()> + Unpin, {
    let future = target.request_future(message)?;
    Ok(ask_with_timeout(future, timeout))
  }

  /// Dispatches all messages.
  ///
  /// # Returns
  ///
  /// `Ok(())` on success, `Err` if a mailbox error occurs
  ///
  /// # Deprecated
  ///
  /// Deprecated since version 3.1.0. Use `dispatch_next` or `run_until` instead.
  #[deprecated(since = "3.1.0", note = "Use dispatch_next or run_until instead")]
  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    #[allow(deprecated)]
    self.inner.dispatch_all()
  }

  /// Dispatches one next message.
  ///
  /// # Returns
  ///
  /// `Ok(())` on success, `Err` if a mailbox error occurs
  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.dispatch_next().await
  }
}
