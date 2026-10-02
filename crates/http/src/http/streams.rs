//! Admission and lifetime controls for long-lived response bodies.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::sync::watch;
use uuid::Uuid;

#[derive(Clone)]
pub struct StreamControl {
    inner: Arc<Inner>,
}

struct Inner {
    counts: Mutex<HashMap<(Uuid, Uuid), usize>>,
    max_total: usize,
    max_per_principal: usize,
    lifetime: Duration,
    shutdown: watch::Sender<bool>,
}

impl Default for StreamControl {
    fn default() -> Self {
        Self::new(128, 4, Duration::from_secs(15 * 60))
    }
}

impl StreamControl {
    pub fn new(max_total: usize, max_per_principal: usize, lifetime: Duration) -> Self {
        assert!(max_total > 0 && max_per_principal > 0 && !lifetime.is_zero());
        Self {
            inner: Arc::new(Inner {
                counts: Mutex::new(HashMap::new()),
                max_total,
                max_per_principal,
                lifetime,
                shutdown: watch::channel(false).0,
            }),
        }
    }

    pub fn shutdown(&self) {
        self.inner.shutdown.send_replace(true);
    }

    pub(super) fn acquire(&self, workspace: Uuid, user: Uuid) -> Option<StreamPermit> {
        let mut counts = self.inner.counts.lock().unwrap_or_else(|e| e.into_inner());
        let key = (workspace, user);
        if *self.inner.shutdown.borrow()
            || counts.values().sum::<usize>() >= self.inner.max_total
            || counts.get(&key).copied().unwrap_or(0) >= self.inner.max_per_principal
        {
            return None;
        }
        *counts.entry(key).or_default() += 1;
        Some(StreamPermit {
            inner: self.inner.clone(),
            key,
        })
    }

    pub(super) fn subscribe(&self) -> watch::Receiver<bool> {
        self.inner.shutdown.subscribe()
    }

    pub(super) fn lifetime(&self) -> Duration {
        self.inner.lifetime
    }
}

pub(super) struct StreamPermit {
    inner: Arc<Inner>,
    key: (Uuid, Uuid),
}

impl Drop for StreamPermit {
    fn drop(&mut self) {
        let mut counts = self.inner.counts.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(count) = counts.get_mut(&self.key) {
            *count -= 1;
            if *count == 0 {
                counts.remove(&self.key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_global_and_per_principal_and_release_on_drop() {
        let control = StreamControl::new(2, 1, Duration::from_secs(1));
        let workspace = Uuid::new_v4();
        let user = Uuid::new_v4();
        let first = control.acquire(workspace, user).unwrap();
        assert!(control.acquire(workspace, user).is_none());
        let second = control.acquire(workspace, Uuid::new_v4()).unwrap();
        assert!(control.acquire(workspace, Uuid::new_v4()).is_none());
        drop(first);
        assert!(control.acquire(workspace, user).is_some());
        drop(second);
        assert!(control.inner.counts.lock().unwrap().is_empty());
        control.shutdown();
        assert!(control.acquire(workspace, user).is_none());
    }
}
