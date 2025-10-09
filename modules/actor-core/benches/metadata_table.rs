#![cfg(feature = "std")]

extern crate alloc;

use alloc::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_actor_core_rs::{
  take_metadata, InternalMessageSender, MessageEnvelope, MessageMetadata, MessageSender, PriorityEnvelope,
};
use nexus_utils_core_rs::{Element, QueueError};

fn noop_sender<M>() -> MessageSender<M>
where
  M: Element, {
  let dispatch = Arc::new(|_, _| -> Result<(), QueueError<PriorityEnvelope<_>>> { Ok(()) });
  let internal = InternalMessageSender::new(dispatch);
  MessageSender::from_internal(internal)
}

fn sample_metadata<M>() -> MessageMetadata
where
  M: Element, {
  MessageMetadata::new().with_sender(noop_sender::<M>())
}

fn bench_side_table(c: &mut Criterion) {
  let mut group = c.benchmark_group("metadata_storage");
  group.bench_function(BenchmarkId::new("side_table", "store_take"), |b| {
    b.iter(|| {
      let metadata = sample_metadata::<u32>();
      let envelope = MessageEnvelope::user_with_metadata(black_box(42_u32), metadata);
      if let MessageEnvelope::User(user) = envelope {
        let (message, key) = user.into_parts();
        black_box(message);
        if let Some(key) = key {
          let metadata = take_metadata(key).expect("metadata present");
          black_box(metadata);
        }
      }
    });
  });

  group.bench_function(BenchmarkId::new("inline", "store_take"), |b| {
    b.iter(|| {
      let metadata = sample_metadata::<u32>();
      let inline = InlineUserMessage::with_metadata(black_box(42_u32), metadata);
      let (message, metadata) = inline.into_parts();
      black_box(message);
      black_box(metadata);
    });
  });

  group.finish();
}

struct InlineUserMessage<U>
where
  U: Element, {
  message: U,
  metadata: Option<MessageMetadata>,
}

impl<U> InlineUserMessage<U>
where
  U: Element,
{
  fn with_metadata(message: U, metadata: MessageMetadata) -> Self {
    Self {
      message,
      metadata: Some(metadata),
    }
  }

  fn into_parts(self) -> (U, Option<MessageMetadata>) {
    (self.message, self.metadata)
  }
}

criterion_group!(benches, bench_side_table);
criterion_main!(benches);
