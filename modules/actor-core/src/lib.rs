#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod actor;

#[cfg(feature = "alloc")]
pub use actor::core_types::actor_error::CoreActorError;
#[cfg(feature = "alloc")]
pub use actor::core_types::auto_receive::{AutoReceiveMessage, TerminatedMessage};
#[cfg(feature = "alloc")]
pub use actor::core_types::mailbox::{
  CoreMailbox, CoreMailboxFuture, CoreMailboxQueueAsyncExt, CoreMailboxQueueFuture,
};
pub use actor::core_types::message::{Message, NotInfluenceReceiveTimeout, ReceiveTimeout, TerminateReason};
#[cfg(feature = "alloc")]
pub use actor::core_types::message_envelope::CoreMessageEnvelope;
#[cfg(feature = "alloc")]
pub use actor::core_types::message_handle::MessageHandle;
#[cfg(feature = "alloc")]
pub use actor::core_types::message_headers::{HeaderMap, ReadonlyMessageHeaders, ReadonlyMessageHeadersHandle};
#[cfg(feature = "alloc")]
pub use actor::core_types::pid::{CorePid, CorePidRef};
#[cfg(feature = "alloc")]
pub use actor::core_types::process::{CoreProcessHandle, ProcessFuture};
#[cfg(feature = "alloc")]
pub use actor::core_types::process_registry::{CoreAddressResolver, CoreProcessRegistry, CoreProcessRegistryFuture};
#[cfg(feature = "alloc")]
pub use actor::core_types::response::{Response, ResponseHandle};
#[cfg(feature = "alloc")]
pub use actor::core_types::restart::{CoreRestartTracker, FailureClock};
#[cfg(feature = "alloc")]
pub use actor::core_types::serialized::{CoreRootSerializable, CoreRootSerialized, CoreSerializationError};
#[cfg(feature = "alloc")]
pub use actor::core_types::system_message::{SystemMessage, UnwatchMessage, WatchMessage};

#[cfg(feature = "alloc")]
pub mod runtime;

#[cfg(feature = "alloc")]
pub use runtime::{
  AsyncMutex, AsyncNotify, AsyncRwLock, CoreJoinFuture, CoreJoinHandle, CoreRuntime, CoreRuntimeConfig,
  CoreScheduledHandle, CoreScheduledHandleRef, CoreScheduledTask, CoreScheduler, CoreSpawnError, CoreSpawner,
  CoreTaskFuture, FnCoreSpawner, FnJoinHandle, Timer,
};

#[cfg(feature = "alloc")]
pub mod supervisor;

#[cfg(feature = "alloc")]
pub mod error;

#[cfg(feature = "alloc")]
pub mod context;
