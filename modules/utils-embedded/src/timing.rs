//! Module that aggregates timer implementations for embedded systems.
//!
//! Exposes `ManualDeadlineTimer` to enable runtimes to manage deadlines via software counters.

mod manual_deadline_timer;

pub use manual_deadline_timer::ManualDeadlineTimer;
