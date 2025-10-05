pub mod synchronized;

pub use synchronized::{
  BoxFuture, GuardHandle, Synchronized, SynchronizedMutexBackend, SynchronizedRw, SynchronizedRwBackend,
};
