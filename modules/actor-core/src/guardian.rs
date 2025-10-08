mod child_record;
mod guardian;
mod strategy;

pub use guardian::Guardian;
pub use strategy::{AlwaysRestart, GuardianStrategy};

pub(crate) use child_record::{ChildRecord, FailureReasonDebug};

#[cfg(test)]
mod tests;
