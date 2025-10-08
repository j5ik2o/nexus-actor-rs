mod child_record;
mod core;
mod strategy;
#[cfg(test)]
mod tests;

pub(crate) use child_record::{ChildRecord, FailureReasonDebug};
pub(crate) use core::Guardian;
pub use strategy::{AlwaysRestart, GuardianStrategy};
