#![cfg(feature = "alloc")]

pub mod core;

pub use core::{
  default_core_supervisor_strategy, CoreSupervisor, CoreSupervisorContext, CoreSupervisorDirective,
  CoreSupervisorFuture, CoreSupervisorStrategy, CoreSupervisorStrategyFuture,
};
