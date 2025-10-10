use super::{Shared, SharedBound};

#[cfg(target_has_atomic = "ptr")]
type SharedFnTarget<Args, Output> = dyn Fn(Args) -> Output + Send + Sync + 'static;

#[cfg(not(target_has_atomic = "ptr"))]
type SharedFnTarget<Args, Output> = dyn Fn(Args) -> Output + 'static;

/// Trait alias for shared function pointers used in actor-core.
pub trait SharedFn<Args, Output>: Shared<SharedFnTarget<Args, Output>> + SharedBound {}

impl<T, Args, Output> SharedFn<Args, Output> for T where T: Shared<SharedFnTarget<Args, Output>> + SharedBound {}

/// Trait alias for shared factories (Send + Sync closures returning a type).
pub trait SharedFactory<Args, Output>: Shared<SharedFnTarget<Args, Output>> + SharedBound {}

impl<T, Args, Output> SharedFactory<Args, Output> for T where T: Shared<SharedFnTarget<Args, Output>> + SharedBound {}
