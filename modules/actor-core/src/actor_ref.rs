use crate::context::PriorityActorRef;

/// High-level alias for the priority-aware actor reference.
pub type ActorRef<M, R> = PriorityActorRef<M, R>;
