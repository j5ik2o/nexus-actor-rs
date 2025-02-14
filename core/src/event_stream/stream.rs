use super::event::Event;
use super::subscription::Subscription;
use dashmap::DashMap;
use std::any::TypeId;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Default)]
pub struct EventStream {
    subscriptions: Arc<DashMap<TypeId, Vec<Arc<Subscription>>>>,
}

impl EventStream {
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(DashMap::new()),
        }
    }

    pub fn subscribe<E: Event + 'static>(&self, subscription: Arc<Subscription>) {
        let type_id = TypeId::of::<E>();
        self.subscriptions
            .entry(type_id)
            .or_insert_with(Vec::new)
            .push(subscription);
    }

    pub fn unsubscribe<E: Event + 'static>(&self, subscription: &Arc<Subscription>) {
        if let Some(mut subs) = self.subscriptions.get_mut(&TypeId::of::<E>()) {
            subs.retain(|s| !Arc::ptr_eq(s, subscription));
        }
    }

    pub fn publish<E: Event + 'static>(&self, event: E) {
        if let Some(subs) = self.subscriptions.get(&TypeId::of::<E>()) {
            let event = Box::new(event) as Box<dyn Event>;
            for subscription in subs.iter() {
                subscription.handle(event.clone());
            }
        }
    }
}

#[derive(Debug)]
pub struct EventStreamHandle {
    inner: Arc<RwLock<EventStream>>,
}

impl EventStreamHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(EventStream::new())),
        }
    }

    pub async fn subscribe<E: Event + 'static>(&self, subscription: Arc<Subscription>) {
        let stream = self.inner.read().await;
        stream.subscribe::<E>(subscription);
    }

    pub async fn unsubscribe<E: Event + 'static>(&self, subscription: &Arc<Subscription>) {
        let stream = self.inner.read().await;
        stream.unsubscribe::<E>(subscription);
    }

    pub async fn publish<E: Event + 'static>(&self, event: E) {
        let stream = self.inner.read().await;
        stream.publish(event);
    }
}

impl Default for EventStreamHandle {
    fn default() -> Self {
        Self::new()
    }
}
