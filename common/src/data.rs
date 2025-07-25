use crate::constants::DEFAULT_CLIENT_DATA_CHANNEL_CAPACITY;
use crate::protocol::Data;
use std::fmt::{self, Debug, Formatter};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::sync::{mpsc, Notify};
use tracing::trace;

pub struct DataChannel {
    pub id: u32,
    pub closed: AtomicBool,
    pub data_tx: mpsc::Sender<Data>,
    pub capacity: AtomicU32,
    pub notify: Notify,
}

impl DataChannel {
    pub fn new_client(id: u32, data_tx: mpsc::Sender<Data>) -> DataChannel {
        Self {
            id,
            closed: AtomicBool::new(false),
            data_tx,
            capacity: AtomicU32::new(DEFAULT_CLIENT_DATA_CHANNEL_CAPACITY),
            notify: Notify::new(),
        }
    }

    pub fn add_capacity(&self, amount: u32) {
        trace!("{:?} acked, consumed {} bytes", self, amount);
        self.capacity.fetch_add(amount, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub async fn wait_for_capacity(&self, required: u32) -> Result<(), mpsc::error::SendError<()>> {
        loop {
            if self.closed.load(Ordering::SeqCst) {
                trace!("{:?} channel is closed, cannot wait for capacity", self);
                return Err(mpsc::error::SendError(()));
            }
            let current = self.capacity.load(Ordering::SeqCst);
            trace!(
                "{:?} checking capacity: {} bytes available, required: {} bytes",
                self,
                current,
                required
            );
            if current >= required {
                // Atomically subtract only if capacity hasn't changed
                match self.capacity.compare_exchange_weak(
                    current,
                    current - required,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        trace!(
                            "{:?} has sufficient capacity: {} bytes available, consuming {} bytes",
                            self,
                            current,
                            required
                        );
                        return Ok(());
                    }
                    Err(_) => continue, // Retry if another thread modified capacity
                }
            }

            trace!(
                "{:?} insufficient capacity: {} bytes available, waiting for {} more",
                self,
                current,
                required
            );

            tokio::select! {
                _ = self.notify.notified() => {
                    trace!("{:?} notified, checking capacity again", self);
                }
                _ = self.data_tx.closed() => {
                    return Err(mpsc::error::SendError(()));
                }
            }
        }
    }

    pub fn close(&self) {
        trace!("{:?} closing channel", self);
        self.closed.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub async fn closed(&self) {
        trace!("{:?} waiting for channel to close", self);
        while !self.closed.load(Ordering::SeqCst) {
            self.notify.notified().await;
        }
    }
}

impl Debug for DataChannel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataChannel")
            .field("id", &self.id)
            .field("capacity", &self.capacity)
            .finish()
    }
}
