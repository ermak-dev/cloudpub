use anyhow::Result;
use dashmap::DashMap;
use futures::future;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, Notify};
use tokio::time;
use tracing::debug;

/// Timeout for waiting for lease to become inactive during drop
const LEASE_DROP_TIMEOUT_SECS: u64 = 5;

pub struct Lease {
    id: u32,
    guid: String,
    renter: Renter,
    pub replace_rx: watch::Receiver<bool>,
    // Notifier to wake up tasks waiting for this lease to drop
    inactive_notify: Arc<Notify>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        // Notify all waiters that this lease is now inactive
        self.inactive_notify.notify_waiters();

        // Unregister the lease from the renter
        debug!(
            "Unregistering lease for guid: {} with id: {}",
            self.guid, self.id
        );
        if let Some((_, entry)) = self
            .renter
            .leases
            .remove_if(&self.guid, |_, v| v.id == self.id)
        {
            debug!(
                "Removed lease entry for guid: {} with id: {}",
                self.guid, self.id
            );
            entry.tx.send(true).ok();
            drop(entry.tx);
        } else {
            debug!(
                "No matching lease found for guid: {} with id: {}",
                self.guid, self.id
            );
        }
    }
}

#[derive(Clone)]
pub struct LeaseEntry {
    id: u32,
    tx: watch::Sender<bool>,
    // Notifier to wake up tasks waiting for this lease to become inactive
    inactive_notify: Arc<Notify>,
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
        // First, check if there's an existing lease and shut it down before creating new one
        if self.leases.contains_key(&guid) {
            debug!("Found previous lease for guid: {}", guid);
            self.drop_lease(&guid).await?;
        }

        // Now that previous lease is gone, create and insert new lease
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        debug!(
            "Attempting to acquire lease for guid: {} with id: {}",
            guid, id
        );

        let (lease_tx, lease_rx) = watch::channel(false);
        let inactive_notify = Arc::new(Notify::new());
        let entry = LeaseEntry {
            id,
            tx: lease_tx,
            inactive_notify: inactive_notify.clone(),
        };

        self.leases.insert(guid.clone(), entry);

        debug!("Lease acquired for guid: {} with id: {}", guid, id);

        Ok(Lease {
            guid: guid.clone(),
            id,
            renter: self.clone(),
            replace_rx: lease_rx,
            inactive_notify,
        })
    }

    pub async fn drop_lease(&self, guid: &str) -> Result<()> {
        debug!("Dropping lease for guid: {}", guid);
        if let Some(entry_ref) = self.leases.get(guid) {
            let id = entry_ref.id;
            let tx = entry_ref.tx.clone();
            let inactive_notify = entry_ref.inactive_notify.clone();
            drop(entry_ref);

            debug!(
                "Sending preempt signal to lease (guid: {}, id: {})",
                guid, id
            );

            // Send preempt signal to owner
            if tx.send(true).is_err() {
                debug!(
                    "Lease already inactive when sending preempt (guid: {}, id: {})",
                    guid, id
                );
            } else {
                debug!(
                    "Waiting for lease to become inactive (guid: {}, id: {})",
                    guid, id
                );

                // Wait for the lease holder to complete cleanup
                let wait_result = time::timeout(
                    Duration::from_secs(LEASE_DROP_TIMEOUT_SECS),
                    inactive_notify.notified(),
                )
                .await;

                match wait_result {
                    Ok(_) => {
                        debug!("Lease became inactive (guid: {}, id: {})", guid, id);
                    }
                    Err(_) => {
                        debug!(
                            "Timeout waiting for lease to become inactive (guid: {}, id: {})",
                            guid, id
                        );
                        return Err(anyhow::anyhow!("Failed to close lease"));
                    }
                }
            }

            // Remove only if it's still the same generation
            let _ = self.leases.remove_if(guid, |_, v| v.id == id);
        }
        Ok(())
    }

    pub async fn drop_all(&self) {
        debug!("Dropping all leases");
        let guids: Vec<String> = self.iter().collect();

        let handles: Vec<_> = guids
            .into_iter()
            .map(|guid| {
                let renter = self.clone();
                tokio::spawn(async move {
                    let _ = renter.drop_lease(&guid).await;
                })
            })
            .collect();

        // Wait for all drop operations to complete concurrently
        let _ = future::join_all(handles).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acquire_times_out_when_prev_not_dropped() {
        let renter = Renter::new();

        let l1 = renter.acquire_lease("g1".to_string()).await.unwrap();

        // Start a competing acquire that will time out because l1 keeps the receiver alive.
        let renter2 = renter.clone();
        let handle = tokio::spawn(async move { renter2.acquire_lease("g1".to_string()).await });

        // Wait for the timeout to occur (15 seconds)
        // We can check for the preempt signal first to confirm it was sent
        let mut rx = l1.replace_rx.clone();
        rx.changed().await.unwrap();
        assert!(
            *rx.borrow(),
            "Previous holder should have received preempt signal"
        );

        // Wait for the spawned task to complete (it should timeout)
        let res = handle.await.unwrap();
        assert!(
            res.is_err(),
            "Second acquire should timeout when previous holder does not drop"
        );

        // The original lease should still be registered because it was never dropped.
        assert!(renter.has_lease("g1"));

        // After dropping l1, the lease should be unregistered
        drop(l1);
        assert!(
            !renter.has_lease("g1"),
            "Lease should be unregistered after dropping"
        );
    }

    #[tokio::test]
    async fn acquire_succeeds_after_prev_drops() {
        let renter = Renter::new();

        let l1 = renter.acquire_lease("g2".to_string()).await.unwrap();

        // Start competing acquire; it will wait for previous to close.
        let renter2 = renter.clone();
        let handle = tokio::spawn(async move { renter2.acquire_lease("g2".to_string()).await });

        // Give the spawned task a moment to start and attempt acquisition
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Wait until l1 is told to preempt, then drop it to allow the new acquire to proceed.
        let mut rx = l1.replace_rx.clone();
        let changed = rx.changed().await;
        assert!(changed.is_ok(), "Should receive preempt signal");
        assert!(*rx.borrow(), "Preempt signal should be true");

        // Now drop l1 which should unregister the lease and close the channel
        drop(l1);

        // Give a moment for the drop to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        let l2 = handle
            .await
            .unwrap()
            .expect("Second acquire should succeed after previous drops");

        assert!(renter.has_lease("g2"), "New lease should be registered");

        drop(l2);
        assert!(
            !renter.has_lease("g2"),
            "Lease should be unregistered after dropping new lease"
        );
    }

    #[tokio::test]
    async fn drop_all_removes_all_leases() {
        let renter = Renter::new();

        // Acquire multiple leases
        let l1 = renter.acquire_lease("g1".to_string()).await.unwrap();
        let l2 = renter.acquire_lease("g2".to_string()).await.unwrap();
        let l3 = renter.acquire_lease("g3".to_string()).await.unwrap();

        assert_eq!(renter.len(), 3, "Should have 3 leases");
        assert!(renter.has_lease("g1"));
        assert!(renter.has_lease("g2"));
        assert!(renter.has_lease("g3"));

        // Keep receivers alive so we can detect preempt signals
        let mut rx1 = l1.replace_rx.clone();
        let mut rx2 = l2.replace_rx.clone();
        let mut rx3 = l3.replace_rx.clone();

        // Call drop_all
        let renter2 = renter.clone();
        let handle = tokio::spawn(async move {
            renter2.drop_all().await;
        });

        // Verify all leases receive preempt signals
        tokio::select! {
            _ = rx1.changed() => assert!(*rx1.borrow(), "l1 should receive preempt signal"),
            _ = tokio::time::sleep(Duration::from_secs(1)) => panic!("Timeout waiting for l1 preempt"),
        }
        tokio::select! {
            _ = rx2.changed() => assert!(*rx2.borrow(), "l2 should receive preempt signal"),
            _ = tokio::time::sleep(Duration::from_secs(1)) => panic!("Timeout waiting for l2 preempt"),
        }
        tokio::select! {
            _ = rx3.changed() => assert!(*rx3.borrow(), "l3 should receive preempt signal"),
            _ = tokio::time::sleep(Duration::from_secs(1)) => panic!("Timeout waiting for l3 preempt"),
        }

        // Drop the leases so drop_all can complete
        drop(l1);
        drop(l2);
        drop(l3);

        // Wait for drop_all to complete
        handle.await.unwrap();

        // Verify all leases are removed
        assert_eq!(renter.len(), 0, "Should have 0 leases after drop_all");
        assert!(!renter.has_lease("g1"));
        assert!(!renter.has_lease("g2"));
        assert!(!renter.has_lease("g3"));
    }
}
