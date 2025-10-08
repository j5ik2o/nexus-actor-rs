#[cfg(feature = "embassy_executor")]
mod sample {
  use core::sync::atomic::{AtomicU32, Ordering};
  use embassy_executor::Executor;
  use nexus_actor_core_rs::{ActorSystem, MailboxOptions, Props};
  use nexus_actor_embedded_rs::{spawn_embassy_dispatcher, LocalMailboxRuntime};
  use static_cell::StaticCell;

  static EXECUTOR: StaticCell<Executor> = StaticCell::new();
  static SYSTEM: StaticCell<ActorSystem<u32, LocalMailboxRuntime>> = StaticCell::new();
  pub static MESSAGE_SUM: AtomicU32 = AtomicU32::new(0);

  pub fn run() {
    let executor = EXECUTOR.init(Executor::new());
    let system = SYSTEM.init_with(|| ActorSystem::new(LocalMailboxRuntime::default()));

    {
      let mut root = system.root_context();
      let actor_ref = root
        .spawn(Props::new(MailboxOptions::default(), |_, msg: u32| {
          MESSAGE_SUM.fetch_add(msg, Ordering::Relaxed);
        }))
        .expect("spawn actor");
      actor_ref.tell(1).expect("tell");
      actor_ref.tell(2).expect("tell");
    }

    executor.run(|spawner| {
      spawn_embassy_dispatcher(spawner, system).expect("spawn dispatcher");
    });
  }
}

#[cfg(feature = "embassy_executor")]
fn main() {
  sample::run();
}

#[cfg(not(feature = "embassy_executor"))]
fn main() {
  panic!("Run with --features embassy_executor to build this example");
}
