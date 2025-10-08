mod composite_sink;
mod custom_sink;
mod parent_guardian_sink;
mod root_sink;
mod traits;

pub(crate) use composite_sink::CompositeEscalationSink;
pub(crate) use custom_sink::CustomEscalationSink;
pub(crate) use parent_guardian_sink::ParentGuardianSink;
pub use root_sink::{FailureEventHandler, FailureEventListener, RootEscalationSink};
pub use traits::EscalationSink;
