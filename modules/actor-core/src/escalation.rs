mod composite_sink;
mod custom_sink;
mod parent_guardian_sink;
mod root_sink;
mod traits;

pub use composite_sink::CompositeEscalationSink;
pub use custom_sink::CustomEscalationSink;
pub use parent_guardian_sink::ParentGuardianSink;
pub use root_sink::{FailureEventHandler, FailureEventListener, RootEscalationSink};
pub use traits::EscalationSink;
