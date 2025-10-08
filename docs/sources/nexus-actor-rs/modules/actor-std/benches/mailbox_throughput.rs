use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use nexus_actor_std_rs::actor::dispatch::dispatcher::{CoreSchedulerDispatcher, DispatcherHandle};
use nexus_actor_std_rs::actor::dispatch::message_invoker::{MessageInvoker, MessageInvokerHandle};
use nexus_actor_std_rs::actor::dispatch::{self, Mailbox, MailboxQueueKind};
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_actor_std_rs::runtime::tokio_core_runtime;
use std::env;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use nexus_actor_std_rs::actor::core::{ActorError, ErrorReason};

#[derive(Debug)]
struct CountingInvoker;

#[async_trait::async_trait]
impl MessageInvoker for CountingInvoker {
  async fn invoke_system_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn invoke_user_message(&mut self, _: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn escalate_failure(&mut self, _: ErrorReason, _: MessageHandle) {}

  async fn record_mailbox_queue_latency(&mut self, _: MailboxQueueKind, _: Duration) {}
}

fn bench_mailbox_process(c: &mut Criterion) {
  let factory = Runtime::new().expect("tokio runtime");
  let mut group = c.benchmark_group("mailbox_process");
  group.sample_size(10);
  group.warm_up_time(Duration::from_millis(200));
  group.measurement_time(Duration::from_secs(2));

  for &load in &[100usize, 1000usize] {
    group.throughput(Throughput::Elements(load as u64));
    for &interval in &[1usize, 8, 64] {
      let bench_name = format!("unbounded_mpsc_{load}_snap{interval}");
      group.bench_function(bench_name, |b| {
        b.to_async(&runtime).iter(|| async move {
          env::set_var("MAILBOX_QUEUE_SNAPSHOT_INTERVAL", interval.to_string());
          let producer = dispatch::unbounded_mpsc_mailbox_creator();
          let mut mailbox = producer.run().await;

          let invoker_handle = MessageInvokerHandle::new(Arc::new(RwLock::new(CountingInvoker)));
          let dispatcher_handle = DispatcherHandle::new(CoreSchedulerDispatcher::from_runtime(tokio_core_runtime()));

          mailbox
            .register_handlers(Some(invoker_handle.clone()), Some(dispatcher_handle.clone()))
            .await;

          mailbox.start().await;

          for i in 0..load {
            mailbox
              .post_user_message(MessageHandle::new(format!("payload-{i}")))
              .await;
          }

          mailbox.process_messages().await;
          env::remove_var("MAILBOX_QUEUE_SNAPSHOT_INTERVAL");
        });
      });
    }
  }

  group.finish();
}

criterion_group!(benches, bench_mailbox_process);
criterion_main!(benches);
