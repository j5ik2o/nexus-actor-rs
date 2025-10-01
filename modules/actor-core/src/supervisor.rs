#![cfg(feature = "alloc")]

pub mod core;

pub use core::{
  CoreSupervisor, CoreSupervisorContext, CoreSupervisorDirective, CoreSupervisorFuture, CoreSupervisorStrategy,
  CoreSupervisorStrategyFuture,
};
