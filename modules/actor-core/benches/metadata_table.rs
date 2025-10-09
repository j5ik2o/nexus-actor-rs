#![cfg(feature = "std")]

extern crate alloc;

use alloc::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_actor_core_rs::api::{InternalMessageSender, MessageEnvelope, MessageMetadata, MessageSender};
use nexus_actor_core_rs::runtime::message::{take_metadata, DynMessage};
use nexus_actor_core_rs::PriorityEnvelope;
use nexus_utils_core_rs::{Element, QueueError};

fn noop_sender<M>() -> MessageSender<M>
where
  M: Element, {
  let dispatch =
    Arc::new(|_message: DynMessage, _priority: i8| -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> { Ok(()) });
  let internal = InternalMessageSender::new(dispatch);
  MessageSender::new(internal)
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
        let (_message, key) = user.into_parts();
        if let Some(key) = key {
          let _ = take_metadata(key).unwrap();
        }
      }
    });
  });

  group.bench_function(BenchmarkId::new("inline", "store_take"), |b| {
    b.iter(|| {
      let metadata = sample_metadata::<u32>();
      let inline = InlineUserMessage::with_metadata(black_box(42_u32), metadata);
      let (_message, metadata) = inline.into_parts();
      let _ = metadata;
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
