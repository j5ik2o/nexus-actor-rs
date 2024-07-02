use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::props::{Props, SpawnError};
use crate::actor::actor::{ActorError, ActorHandle};
use crate::actor::actor_system::ActorSystem;
use crate::actor::future::Future;
use crate::actor::message::{MessageHandle, ResponseHandle};
use crate::actor::message_envelope::{MessageEnvelope, ReadonlyMessageHeadersHandle};
use crate::actor::messages::ContinuationFunc;
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};

pub mod actor_context;
mod actor_context_extras;
mod receive_timeout_timer;
pub mod root_context;
mod state;

pub trait HasAny: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &(dyn Any + Send + Sync);
}

#[derive(Debug, Clone)]
pub struct HasAnyHandle(Arc<dyn HasAny>);

impl PartialEq for HasAnyHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for HasAnyHandle {}

impl std::hash::Hash for HasAnyHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn HasAny).hash(state);
  }
}

impl HasAnyHandle {
  pub fn new(has_any: Arc<dyn HasAny>) -> Self {
    HasAnyHandle(has_any)
  }
}

impl HasAny for HasAnyHandle {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self.0.as_any()
  }
}

pub trait Context:
  ExtensionContext
  + SenderContext
  + ReceiverContext
  + SpawnerContext
  + BasePart
  + StopperPart
  + Debug
  + Send
  + Sync
  + 'static {
}

#[derive(Debug, Clone)]
pub struct ContextHandle(Arc<Mutex<dyn Context>>);

impl ContextHandle {
  pub fn new_arc(context: Arc<Mutex<dyn Context>>) -> Self {
    ContextHandle(context)
  }

  pub fn new(c: impl Context + 'static) -> Self {
    ContextHandle(Arc::new(Mutex::new(c)))
  }
}

impl ExtensionContext for ContextHandle {}

#[async_trait]
impl ExtensionPart for ContextHandle {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let mut mg = self.0.lock().await;
    mg.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    let mut mg = self.0.lock().await;
    mg.set(ext).await
  }
}

impl SenderContext for ContextHandle {}

#[async_trait]
impl InfoPart for ContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_parent().await
  }

  async fn get_self(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_self().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let mg = self.0.lock().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.0.lock().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl SenderPart for ContextHandle {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_sender().await
  }

  async fn send(&mut self, pid: ExtendedPid, message: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.send(pid, message).await
  }

  async fn request(&mut self, pid: ExtendedPid, message: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.request(pid, message).await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message: MessageHandle, sender: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.request_with_custom_sender(pid, message, sender).await
  }

  async fn request_future(&self, pid: ExtendedPid, message: MessageHandle, timeout: &Duration) -> Future {
    let mg = self.0.lock().await;
    mg.request_future(pid, message, timeout).await
  }
}

#[async_trait]
impl MessagePart for ContextHandle {
  async fn get_message(&self) -> Option<MessageHandle> {
    let mg = self.0.lock().await;
    mg.get_message().await
  }

  async fn get_message_header(&self) -> ReadonlyMessageHeadersHandle {
    let mg = self.0.lock().await;
    mg.get_message_header().await
  }
}

impl ReceiverContext for ContextHandle {}

#[async_trait]
impl ReceiverPart for ContextHandle {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.receive(envelope).await
  }
}

impl SpawnerContext for ContextHandle {}

#[async_trait]
impl SpawnerPart for ContextHandle {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    let mut mg = self.0.lock().await;
    mg.spawn(props).await
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    let mut mg = self.0.lock().await;
    mg.spawn_prefix(props, prefix).await
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    let mut mg = self.0.lock().await;
    mg.spawn_named(props, id).await
  }
}

#[async_trait]
impl BasePart for ContextHandle {
  async fn get_receive_timeout(&self) -> Duration {
    let mg = self.0.lock().await;
    mg.get_receive_timeout().await
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_children().await
  }

  async fn respond(&self, response: ResponseHandle) {
    let mg = self.0.lock().await;
    mg.respond(response).await
  }

  async fn stash(&mut self) {
    let mut mg = self.0.lock().await;
    mg.stash().await
  }

  async fn watch(&mut self, pid: &ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.watch(pid).await
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.unwatch(pid).await
  }

  async fn set_receive_timeout(&mut self, d: &Duration) {
    let mut mg = self.0.lock().await;
    mg.set_receive_timeout(d).await
  }

  async fn cancel_receive_timeout(&mut self) {
    let mut mg = self.0.lock().await;
    mg.cancel_receive_timeout().await
  }

  async fn forward(&self, pid: &ExtendedPid) {
    let mg = self.0.lock().await;
    mg.forward(pid).await
  }

  async fn reenter_after(&self, f: Future, continuation: ContinuationFunc) {
    let mg = self.0.lock().await;
    mg.reenter_after(f, continuation).await
  }
}

#[async_trait]
impl StopperPart for ContextHandle {
  async fn stop(&mut self, pid: &ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.stop(pid).await
  }

  async fn stop_future(&mut self, pid: &ExtendedPid) -> Future {
    let mut mg = self.0.lock().await;
    mg.stop_future(pid).await
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.poison(pid).await
  }

  async fn poison_future(&mut self, pid: &ExtendedPid) -> Future {
    let mut mg = self.0.lock().await;
    mg.poison_future(pid).await
  }
}

impl Context for ContextHandle {}

pub trait ExtensionContext: ExtensionPart + Send + Sync + 'static {}

pub trait SenderContext: InfoPart + SenderPart + MessagePart + Send + Sync + 'static {}

#[derive(Debug, Clone)]
pub struct SenderContextHandle(Arc<Mutex<dyn SenderContext>>);

impl SenderContextHandle {
  pub fn new_arc(context: Arc<Mutex<dyn SenderContext>>) -> Self {
    SenderContextHandle(context)
  }

  pub fn new(c: impl SenderContext + 'static) -> Self {
    SenderContextHandle(Arc::new(Mutex::new(c)))
  }
}

#[async_trait]
impl InfoPart for SenderContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_parent().await
  }

  async fn get_self(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_self().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let mg = self.0.lock().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.0.lock().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl SenderPart for SenderContextHandle {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_sender().await
  }

  async fn send(&mut self, pid: ExtendedPid, message: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.send(pid, message).await
  }

  async fn request(&mut self, pid: ExtendedPid, message: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.request(pid, message).await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message: MessageHandle, sender: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.request_with_custom_sender(pid, message, sender).await
  }

  async fn request_future(&self, pid: ExtendedPid, message: MessageHandle, timeout: &Duration) -> Future {
    let mg = self.0.lock().await;
    mg.request_future(pid, message, timeout).await
  }
}

#[async_trait]
impl MessagePart for SenderContextHandle {
  async fn get_message(&self) -> Option<MessageHandle> {
    let mg = self.0.lock().await;
    mg.get_message().await
  }

  async fn get_message_header(&self) -> ReadonlyMessageHeadersHandle {
    let mg = self.0.lock().await;
    mg.get_message_header().await
  }
}

impl SenderContext for SenderContextHandle {}

pub trait ReceiverContext: InfoPart + ReceiverPart + MessagePart + ExtensionPart + Send + Sync + 'static {}

#[derive(Debug, Clone)]
pub struct ReceiverContextHandle(Arc<Mutex<dyn ReceiverContext>>);

impl ReceiverContextHandle {
  pub fn new_arc(context: Arc<Mutex<dyn ReceiverContext>>) -> Self {
    ReceiverContextHandle(context)
  }

  pub fn new(c: impl ReceiverContext + 'static) -> Self {
    ReceiverContextHandle(Arc::new(Mutex::new(c)))
  }
}

#[async_trait]
impl InfoPart for ReceiverContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_parent().await
  }

  async fn get_self(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_self().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let mg = self.0.lock().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.0.lock().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl ReceiverPart for ReceiverContextHandle {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.receive(envelope).await
  }
}

#[async_trait]
impl MessagePart for ReceiverContextHandle {
  async fn get_message(&self) -> Option<MessageHandle> {
    let mg = self.0.lock().await;
    mg.get_message().await
  }

  async fn get_message_header(&self) -> ReadonlyMessageHeadersHandle {
    let mg = self.0.lock().await;
    mg.get_message_header().await
  }
}

#[async_trait]
impl ExtensionPart for ReceiverContextHandle {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let mut mg = self.0.lock().await;
    mg.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    let mut mg = self.0.lock().await;
    mg.set(ext).await
  }
}

impl ReceiverContext for ReceiverContextHandle {}

pub trait SpawnerContext: InfoPart + SpawnerPart + Send + Sync + 'static {}

#[derive(Debug, Clone)]
pub struct SpawnerContextHandle(Arc<Mutex<dyn SpawnerContext>>);

impl PartialEq for SpawnerContextHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for SpawnerContextHandle {}

impl std::hash::Hash for SpawnerContextHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const Mutex<dyn SpawnerContext>).hash(state);
  }
}

impl SpawnerContextHandle {
  pub fn new_arc(context: Arc<Mutex<dyn SpawnerContext>>) -> Self {
    SpawnerContextHandle(context)
  }

  pub fn new(c: impl SpawnerContext + 'static) -> Self {
    SpawnerContextHandle(Arc::new(Mutex::new(c)))
  }
}

#[async_trait]
impl InfoPart for SpawnerContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_parent().await
  }

  async fn get_self(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_self().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let mg = self.0.lock().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.0.lock().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl SpawnerPart for SpawnerContextHandle {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    let mut mg = self.0.lock().await;
    mg.spawn(props).await
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    let mut mg = self.0.lock().await;
    mg.spawn_prefix(props, prefix).await
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    let mut mg = self.0.lock().await;
    mg.spawn_named(props, id).await
  }
}

impl SpawnerContext for SpawnerContextHandle {}

#[async_trait]
pub trait ExtensionPart: Send + Sync + 'static {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle>;
  async fn set(&mut self, ext: ContextExtensionHandle);
}

#[async_trait]
pub trait InfoPart: Debug + Send + Sync + 'static {
  // Parent returns the PID for the current actors parent
  async fn get_parent(&self) -> Option<ExtendedPid>;

  // Self returns the PID for the current actor
  async fn get_self(&self) -> Option<ExtendedPid>;

  async fn set_self(&mut self, pid: ExtendedPid);

  // Actor returns the actor associated with this context
  async fn get_actor(&self) -> Option<ActorHandle>;

  async fn get_actor_system(&self) -> ActorSystem;
}

#[async_trait]
pub trait BasePart: Debug + Send + Sync + 'static {
  // ReceiveTimeout returns the current timeout
  async fn get_receive_timeout(&self) -> tokio::time::Duration;

  // Children returns a slice of the actors children
  async fn get_children(&self) -> Vec<ExtendedPid>;

  // Respond sends a response to the current `Sender`
  // If the Sender is nil, the actor will panic
  async fn respond(&self, response: ResponseHandle);

  // Stash stashes the current message on a stack for reprocessing when the actor restarts
  async fn stash(&mut self);

  // Watch registers the actor as a monitor for the specified PID
  async fn watch(&mut self, pid: &ExtendedPid);

  // Unwatch unregisters the actor as a monitor for the specified PID
  async fn unwatch(&mut self, pid: &ExtendedPid);

  // SetReceiveTimeout sets the inactivity timeout, after which a ReceiveTimeout message will be sent to the actor.
  // A duration of less than 1ms will disable the inactivity timer.
  //
  // If a message is received before the duration d, the timer will be reset. If the message conforms to
  // the not_influence_receive_timeout interface, the timer will not be reset
  async fn set_receive_timeout(&mut self, d: &tokio::time::Duration);

  async fn cancel_receive_timeout(&mut self);

  // Forward forwards current message to the given PID
  async fn forward(&self, pid: &ExtendedPid);

  async fn reenter_after(&self, f: Future, continuation: ContinuationFunc);
}

#[async_trait]
pub trait MessagePart: Debug + Send + Sync + 'static {
  // Message returns the current message to be processed
  async fn get_message(&self) -> Option<MessageHandle>;

  // MessageHeader returns the meta information for the currently processed message
  async fn get_message_header(&self) -> ReadonlyMessageHeadersHandle;
}

#[async_trait]
pub trait SenderPart: Debug + Send + Sync + 'static {
  // Sender returns the PID of actor that sent currently processed message
  async fn get_sender(&self) -> Option<ExtendedPid>;

  // Send sends a message to the given PID
  async fn send(&mut self, pid: ExtendedPid, message: MessageHandle);

  // Request sends a message to the given PID
  async fn request(&mut self, pid: ExtendedPid, message: MessageHandle);

  // RequestWithCustomSender sends a message to the given PID and also provides a Sender PID
  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message: MessageHandle, sender: ExtendedPid);

  // RequestFuture sends a message to a given PID and returns a Future
  async fn request_future(&self, pid: ExtendedPid, message: MessageHandle, timeout: &tokio::time::Duration) -> Future;
}

#[async_trait]
pub trait ReceiverPart: Debug + Send + Sync + 'static {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError>;
}

#[async_trait]
pub trait SpawnerPart: Send + Sync + 'static {
  // Spawn starts a new child actor based on props and named with a unique id
  async fn spawn(&mut self, props: Props) -> ExtendedPid;

  // SpawnPrefix starts a new child actor based on props and named using a prefix followed by a unique id
  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid;

  // SpawnNamed starts a new child actor based on props and named using the specified name
  //
  // ErrNameExists will be returned if id already exists
  //
  // Please do not use name sharing same pattern with system actors, for example "YourPrefix$1", "Remote$1", "future$1"
  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError>;
}

#[async_trait]
pub trait StopperPart: Debug + Send + Sync + 'static {
  // Stop will stop actor immediately regardless of existing user messages in mailbox.
  async fn stop(&mut self, pid: &ExtendedPid);

  // StopFuture will stop actor immediately regardless of existing user messages in mailbox, and return its future.
  async fn stop_future(&mut self, pid: &ExtendedPid) -> Future;

  // Poison will tell actor to stop after processing current user messages in mailbox.
  async fn poison(&mut self, pid: &ExtendedPid);

  // PoisonFuture will tell actor to stop after processing current user messages in mailbox, and return its future.
  async fn poison_future(&mut self, pid: &ExtendedPid) -> Future;
}
