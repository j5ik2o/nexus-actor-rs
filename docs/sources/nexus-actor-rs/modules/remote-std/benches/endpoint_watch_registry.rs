use std::sync::Weak;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use nexus_actor_std_rs::actor::core::PidSet;
use nexus_actor_std_rs::generated::actor::Pid;
use nexus_remote_std_rs::{EndpointWatcher, WatchRegistry};
use tokio::runtime::Runtime;

fn make_pid(seq: usize) -> Pid {
  Pid {
    address: format!("bench-address-{seq}"),
    id: format!("bench-pid-{seq}"),
    request_id: seq as u32,
  }
}

fn bench_endpoint_watch_registry(c: &mut Criterion) {
  let factory = Runtime::new().expect("tokio runtime");
  let mut group = c.benchmark_group("endpoint_watch_registry");
  group.sample_size(15);
  group.warm_up_time(Duration::from_millis(300));
  group.measurement_time(Duration::from_secs(3));

  let loads = [64usize, 512usize, 4096usize];
  for &load in &loads {
    group.throughput(Throughput::Elements(load as u64));

    group.bench_function(BenchmarkId::new("watcher_add_remove", load), |b| {
      b.to_async(&runtime).iter(|| async move {
        let watcher = EndpointWatcher::new(Weak::new(), "bench-endpoint".to_string());
        let watcher_id = "bench-watcher";
        let mut pids = Vec::with_capacity(load);
        for idx in 0..load {
          let pid = make_pid(idx);
          watcher.add_watch_pid(watcher_id, pid.clone()).await;
          pids.push(pid);
        }
        for pid in &pids {
          watcher.remove_watch_pid(watcher_id, pid).await;
        }
        watcher.prune_if_empty(watcher_id).await;
      });
    });

    group.bench_function(BenchmarkId::new("registry_add_remove", load), |b| {
      b.to_async(&runtime).iter(|| async move {
        let registry = WatchRegistry::new();
        let watcher_id = "bench-watcher";
        let mut pids = Vec::with_capacity(load);
        for idx in 0..load {
          let pid = make_pid(idx);
          let _ = registry.watch(watcher_id, pid.clone()).await;
          pids.push(pid);
        }
        for pid in &pids {
          let _ = registry.unwatch(watcher_id, pid).await;
        }
        registry.prune_if_empty(watcher_id).await;
      });
    });

    group.bench_function(BenchmarkId::new("pid_set_add_remove", load), |b| {
      b.to_async(&runtime).iter(|| async move {
        let pid_set = PidSet::new();
        let mut pids = Vec::with_capacity(load);
        for idx in 0..load {
          let pid = make_pid(idx);
          pid_set.add(pid.clone()).await;
          pids.push(pid);
        }
        for pid in &pids {
          pid_set.remove(pid).await;
        }
        pid_set.is_empty().await;
      });
    });
  }

  group.finish();
}

criterion_group!(benches, bench_endpoint_watch_registry);
criterion_main!(benches);
