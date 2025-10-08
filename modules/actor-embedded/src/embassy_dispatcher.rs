#![cfg(feature = "embassy_executor")]

use embassy_executor::Spawner;
use nexus_actor_core_rs::guardian::GuardianStrategy;
use nexus_actor_core_rs::{ActorSystem, MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::Element;

/// Embassy の `Spawner` に `ActorSystem::run_forever` を登録するヘルパ。
///
/// # 使用例
/// ```ignore
/// static SYSTEM: StaticCell<ActorSystem<MessageEnvelope<MyMsg>, LocalMailboxRuntime>> = StaticCell::new();
/// let system = SYSTEM.init_with(|| ActorSystem::new(LocalMailboxRuntime::default()));
/// spawn_embassy_dispatcher(&spawner, system).unwrap();
/// ```
///
/// `ActorSystem` は `'static` な可変参照である必要があります。`StaticCell` などを利用し、
/// 実行時に初期化したあと Embassy タスクへ移譲してください。
pub fn spawn_embassy_dispatcher<M, R, Strat>(
  spawner: &Spawner,
  system: &'static mut ActorSystem<M, R, Strat>,
) -> Result<(), embassy_executor::SpawnError>
where
  M: Element + 'static,
  R: MailboxRuntime + Clone + 'static,
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
