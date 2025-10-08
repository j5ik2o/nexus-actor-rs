use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use nexus_utils_std_rs::collections::{
  MpscBoundedChannelQueue, MpscUnboundedChannelQueue, QueueReader, QueueWriter, RingQueue,
};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{SendError, TryRecvError};
use tokio::sync::RwLock;

mod legacy {
  use super::*;

  #[derive(Debug)]
  struct BoundedInner<E> {
    receiver: mpsc::Receiver<E>,
    count: usize,
    capacity: usize,
    is_closed: bool,
  }

  #[derive(Debug, Clone)]
  pub struct MpscBoundedLegacy<E> {
    sender: mpsc::Sender<E>,
    inner: Arc<RwLock<BoundedInner<E>>>,
  }

  impl<T> MpscBoundedLegacy<T> {
    pub fn new(capacity: usize) -> Self {
      let (sender, receiver) = mpsc::channel(capacity);
      Self {
        sender,
        inner: Arc::new(RwLock::new(BoundedInner {
          receiver,
          count: 0,
          capacity,
          is_closed: false,
        })),
      }
    }

    pub async fn offer(&mut self, element: T) -> Result<(), SendError<T>> {
      {
        let inner = self.inner.read().await;
        if inner.is_closed || inner.count >= inner.capacity {
          return Err(SendError(element));
        }
      }

      match self.sender.try_send(element) {
        Ok(()) => {
          let mut inner = self.inner.write().await;
          inner.count += 1;
          Ok(())
        }
        Err(mpsc::error::TrySendError::Full(e)) => Err(SendError(e)),
        Err(mpsc::error::TrySendError::Closed(e)) => Err(SendError(e)),
      }
    }

    pub async fn poll(&mut self) -> Result<Option<T>, TryRecvError> {
      let mut inner = self.inner.write().await;
      if inner.is_closed {
        return Err(TryRecvError::Disconnected);
      }
      match inner.receiver.try_recv() {
        Ok(val) => {
          inner.count = inner.count.saturating_sub(1);
          Ok(Some(val))
        }
        Err(err) => Err(err),
      }
    }
  }

  #[derive(Debug)]
  struct UnboundedInner<E> {
    receiver: mpsc::UnboundedReceiver<E>,
    count: usize,
    is_closed: bool,
  }

  #[derive(Debug, Clone)]
  pub struct MpscUnboundedLegacy<E> {
    sender: mpsc::UnboundedSender<E>,
    inner: Arc<RwLock<UnboundedInner<E>>>,
  }

  impl<T> MpscUnboundedLegacy<T> {
    pub fn new() -> Self {
      let (sender, receiver) = mpsc::unbounded_channel();
      Self {
        sender,
        inner: Arc::new(RwLock::new(UnboundedInner {
          receiver,
          count: 0,
          is_closed: false,
        })),
      }
    }

    pub async fn offer(&mut self, element: T) -> Result<(), SendError<T>> {
      {
        let inner = self.inner.read().await;
        if inner.is_closed {
          return Err(SendError(element));
        }
      }
      match self.sender.send(element) {
        Ok(()) => {
          let mut inner = self.inner.write().await;
          inner.count += 1;
          Ok(())
        }
        Err(err) => Err(err),
      }
    }

    pub async fn poll(&mut self) -> Result<Option<T>, TryRecvError> {
      let mut inner = self.inner.write().await;
      if inner.is_closed {
        return Err(TryRecvError::Disconnected);
      }
      match inner.receiver.try_recv() {
        Ok(val) => {
          inner.count = inner.count.saturating_sub(1);
          Ok(Some(val))
        }
        Err(err) => Err(err),
      }
    }
  }
}

fn bench_ring_queue(c: &mut Criterion) {
  let mut group = c.benchmark_group("ring_queue_offer_poll");
  group.sample_size(10);
  group.warm_up_time(Duration::from_millis(200));
  group.measurement_time(Duration::from_secs(2));

  for &load in &[1_000usize, 10_000usize] {
    group.throughput(Throughput::Elements(load as u64));
    group.bench_function(format!("ring_queue_current_{load}"), |b| {
      b.iter(|| {
        let mut queue = RingQueue::new(load + 1);
        for i in 0..load {
          queue.offer(i).unwrap();
        }
        for _ in 0..load {
          queue.poll().unwrap();
        }
      });
    });
  }

  group.finish();
}

fn bench_async_queue(c: &mut Criterion) {
  let factory = Runtime::new().expect("tokio runtime");
  let mut group = c.benchmark_group("mpsc_queue_offer_poll");
  group.sample_size(10);
  group.warm_up_time(Duration::from_millis(200));
  group.measurement_time(Duration::from_secs(2));

  for &load in &[1_000usize, 10_000usize] {
    group.throughput(Throughput::Elements(load as u64));

    group.bench_function(format!("unbounded_current_{load}"), |b| {
      b.iter(|| {
        let mut queue = MpscUnboundedChannelQueue::new();
        for i in 0..load {
          queue.offer(i).unwrap();
        }
        for _ in 0..load {
          queue.poll().unwrap();
        }
      });
    });

    group.bench_function(format!("unbounded_legacy_{load}"), |b| {
      b.to_async(&runtime).iter(|| async move {
        let mut queue = legacy::MpscUnboundedLegacy::new();
        for i in 0..load {
          queue.offer(i).await.unwrap();
        }
        for _ in 0..load {
          queue.poll().await.unwrap();
        }
      });
    });

    group.bench_function(format!("bounded_current_{load}"), |b| {
      b.iter(|| {
        let mut queue = MpscBoundedChannelQueue::new(load + 1);
        for i in 0..load {
          queue.offer(i).unwrap();
        }
        for _ in 0..load {
          queue.poll().unwrap();
        }
      });
    });

    group.bench_function(format!("bounded_legacy_{load}"), |b| {
      b.to_async(&runtime).iter(|| async move {
        let mut queue = legacy::MpscBoundedLegacy::new(load + 1);
        for i in 0..load {
          queue.offer(i).await.unwrap_or_else(|e| panic!("offer failed: {e}"));
        }
        for _ in 0..load {
          queue.poll().await.unwrap();
        }
      });
    });
  }

  group.finish();
}

criterion_group!(benches, bench_ring_queue, bench_async_queue);
criterion_main!(benches);
