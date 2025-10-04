use core::future::Future;

use nexus_actor_core_rs::Spawn;

pub struct ImmediateSpawner;

impl Spawn for ImmediateSpawner {
  fn spawn(&self, _fut: impl Future<Output = ()> + 'static) {}
}
