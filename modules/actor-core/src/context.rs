#![cfg(feature = "alloc")]

pub mod core;

pub use core::{
  CoreActorContext, CoreActorContextSnapshot, CoreMailboxFactory, CoreProps, CorePropsFactory,
  CoreSupervisorStrategyHandle,
};
