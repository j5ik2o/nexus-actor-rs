pub mod deadline_timer;

pub use deadline_timer::{
  DeadlineTimer, DeadlineTimerError, DeadlineTimerExpired, DeadlineTimerKey, DeadlineTimerKeyAllocator, TimerDeadline,
};
