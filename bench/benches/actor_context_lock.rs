//! ActorContext 周辺のロック挙動を計測するベンチマーク (PoC スケルトン)

use criterion::{criterion_group, criterion_main, Criterion};

fn actor_context_lock_bench(_c: &mut Criterion) {
  // TODO: ActorContextExtras のロック待ち計測ロジックを実装する。
}

criterion_group!(benches, actor_context_lock_bench);
criterion_main!(benches);
