#![cfg(feature = "std")]

extern crate alloc;

#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Arc;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_actor_core_rs::{
  take_metadata, DynMessage, InternalMessageSender, MessageEnvelope, MessageMetadata, MessageSender,
  MetadataStorageMode, PriorityEnvelope, SingleThread, ThreadSafe,
};
use nexus_utils_core_rs::sync::ArcShared;

#[cfg(target_has_atomic = "ptr")]
type NoopDispatchFn = dyn Fn(DynMessage, i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> + Send + Sync;

#[cfg(not(target_has_atomic = "ptr"))]
type NoopDispatchFn = dyn Fn(DynMessage, i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>;
use nexus_utils_core_rs::{Element, QueueError};

fn noop_sender<M, C>() -> MessageSender<M, C>
where
  M: Element,
  C: MetadataStorageMode, {
  let dispatch_impl: Arc<NoopDispatchFn> = Arc::new(|_, _| Ok(()));
  let dispatch = ArcShared::from_arc(dispatch_impl);
  let internal = InternalMessageSender::<C>::new(dispatch);
  MessageSender::from_internal(internal)
}

fn sample_metadata<M, C>() -> MessageMetadata<C>
where
  M: Element,
  C: MetadataStorageMode, {
  MessageMetadata::<C>::new().with_sender(noop_sender::<M, C>())
}

fn bench_side_table(c: &mut Criterion) {
  let mut group = c.benchmark_group("metadata_storage");

  bench_mode::<ThreadSafe>(&mut group, "thread_safe");
  bench_mode::<SingleThread>(&mut group, "single_thread");

  group.finish();
}

fn bench_mode<C>(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>, label: &str)
where
  C: MetadataStorageMode, {
  group.bench_function(BenchmarkId::new(label, "side_table"), |b| {
    b.iter(|| {
      let metadata = sample_metadata::<u32, C>();
      let envelope = MessageEnvelope::user_with_metadata(black_box(42_u32), metadata);
      if let MessageEnvelope::User(user) = envelope {
        let (message, key) = user.into_parts();
        black_box(message);
        if let Some(key) = key {
          let metadata = take_metadata::<C>(key).expect("metadata present");
          black_box(metadata);
        }
      }
    });
  });

  group.bench_function(BenchmarkId::new(label, "inline"), |b| {
    b.iter(|| {
      let metadata = sample_metadata::<u32, C>();
      let inline = InlineUserMessage::<_, C>::with_metadata(black_box(42_u32), metadata);
      let (message, metadata) = inline.into_parts();
      black_box(message);
      black_box(metadata);
    });
  });
}

struct InlineUserMessage<U, C>
where
  U: Element,
  C: MetadataStorageMode, {
  message: U,
  metadata: Option<MessageMetadata<C>>,
}

impl<U, C> InlineUserMessage<U, C>
where
  U: Element,
  C: MetadataStorageMode,
{
  fn with_metadata(message: U, metadata: MessageMetadata<C>) -> Self {
    Self {
      message,
      metadata: Some(metadata),
    }
  }

  fn into_parts(self) -> (U, Option<MessageMetadata<C>>) {
    (self.message, self.metadata)
  }
}

criterion_group!(benches, bench_side_table);
criterion_main!(benches);
