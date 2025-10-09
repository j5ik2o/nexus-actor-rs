use super::Shared;

/// Trait alias for shared function pointers used in actor-core.
pub trait SharedFn<Args, Output>: Shared<dyn Fn(Args) -> Output + Send + Sync> + Send + Sync {}

impl<T, Args, Output> SharedFn<Args, Output> for T where T: Shared<dyn Fn(Args) -> Output + Send + Sync> + Send + Sync {}

/// Trait alias for shared factories (Send + Sync closures returning a type).
pub trait SharedFactory<Args, Output>: Shared<dyn Fn(Args) -> Output + Send + Sync> + Send + Sync {}

impl<T, Args, Output> SharedFactory<Args, Output> for T where
  T: Shared<dyn Fn(Args) -> Output + Send + Sync> + Send + Sync
{
}
