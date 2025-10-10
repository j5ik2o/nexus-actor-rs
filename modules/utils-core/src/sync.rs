mod arc_shared;
mod flag;
/// Helper traits for shared function and factory closures.
pub mod function;
#[cfg(feature = "alloc")]
mod rc_shared;
mod shared;
mod state;

pub use arc_shared::ArcShared;
pub use flag::Flag;
pub use function::{SharedFactory, SharedFn};
#[cfg(feature = "alloc")]
pub use rc_shared::RcShared;
pub use shared::{Shared, SharedBound};
pub use state::StateCell;
