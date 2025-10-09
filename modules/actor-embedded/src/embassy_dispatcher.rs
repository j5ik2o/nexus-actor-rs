#![cfg(feature = "embassy_executor")]

use embassy_executor::Spawner;
use nexus_actor_core_rs::guardian::GuardianStrategy;
use nexus_actor_core_rs::{ActorSystem, MailboxFactory, PriorityEnvelope};
use nexus_utils_core_rs::Element;

/// Helper to register `ActorSystem::run_forever` with Embassy's `Spawner`.
///
/// # Usage Example
/// ```ignore
/// static SYSTEM: StaticCell<ActorSystem<MessageEnvelope<MyMsg>, LocalMailboxFactory>> = StaticCell::new();
/// let system = SYSTEM.init_with(|| ActorSystem::new(LocalMailboxFactory::default()));
/// spawn_embassy_dispatcher(&spawner, system).unwrap();
/// ```
///
/// The `ActorSystem` must be a `'static` mutable reference. Use `StaticCell` or similar,
/// initialize it at runtime, and then delegate to an Embassy task.
pub fn spawn_embassy_dispatcher<M, R, Strat>(
  spawner: &Spawner,
  system: &'static mut ActorSystem<M, R, Strat>,
) -> Result<(), embassy_executor::SpawnError>
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R> + 'static,
{
  spawner.spawn(async move {
    match system.run_forever().await {
      Ok(_) => unreachable!("run_forever must not resolve with Ok"),
      Err(err) => {
        let _ = err;
        #[cfg(debug_assertions)]
        panic!("Embassy dispatcher terminated unexpectedly");
      }
    }
  })
}
