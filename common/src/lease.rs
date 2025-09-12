use anyhow::{Context, Result};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;
use tracing::debug;

pub struct Lease {
    id: u32,
    guid: String,
    renter: Renter,
    pub replace_rx: watch::Receiver<bool>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.renter.unregister_lease(&self.guid, self.id);
    }
}

#[derive(Clone)]
pub struct LeaseEntry {
    id: u32,
    tx: watch::Sender<bool>,
}

#[derive(Clone)]
pub struct Renter {
    leases: Arc<DashMap<String, LeaseEntry>>,
    counter: Arc<AtomicU32>,
}

impl Default for Renter {
    fn default() -> Self {
        Self::new()
    }
}

impl Renter {
    pub fn new() -> Self {
        Self {
            leases: Arc::new(DashMap::new()),
            counter: Arc::new(AtomicU32::new(1)),
        }
    }

    pub async fn acquire_lease(&self, guid: String) -> Result<Lease> {
        // Acquire lease
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let (lease_tx, lease_rx) = watch::channel(false);
        let entry = LeaseEntry { id, tx: lease_tx };

        if let Some(prev) = self.leases.insert(guid.clone(), entry) {
            let _ = prev.tx.send(true); // preempt previous owner
            time::timeout(Duration::from_secs(1), prev.tx.closed())
                .await
                .context("Failed to close previous lease")?;
        }

        debug!("Lease acquired for guid: {}", guid);

        Ok(Lease {
            guid: guid.clone(),
            id,
            renter: self.clone(),
            replace_rx: lease_rx,
        })
    }

    pub async fn drop_lease(&self, guid: &str) {
        debug!("Dropping lease for guid: {}", guid);
        if let Some(entry_ref) = self.leases.get(guid) {
            let id = entry_ref.id;
            let tx = entry_ref.tx.clone();
            drop(entry_ref);

            // Preempt the current owner
            tx.send(true).ok();
            // Wait for the lease holder to clean up
            let _ = time::timeout(Duration::from_secs(1), tx.closed()).await;

            // Remove only if it's still the same generation
            let _ = self.leases.remove_if(guid, |_, v| v.id == id);
        }
    }

    fn unregister_lease(&self, guid: &str, id: u32) {
        debug!("Unregistering lease for guid: {}", guid);
        if let Some((_, entry)) = self.leases.remove_if(guid, |_, v| v.id == id) {
            entry.tx.send(true).ok();
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = String> {
        self.leases
            .iter()
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub fn has_lease(&self, guid: &str) -> bool {
        self.leases.contains_key(guid)
    }

    pub fn len(&self) -> usize {
        self.leases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.leases.is_empty()
    }
}
