#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod actor;

pub use actor::core_types::message::{Message, NotInfluenceReceiveTimeout, ReceiveTimeout, TerminateReason};

#[cfg(feature = "alloc")]
pub mod runtime;
