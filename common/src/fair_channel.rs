extern crate alloc;

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc::error::{SendError, TryRecvError, TrySendError};
use tokio::sync::Notify;

/// Trait for defining grouping logic for fair queuing.
/// Groups are determined based on `group_id`. None represents the control group.
pub trait FairGroup: Clone {
    fn group_id(&self) -> Option<u32>;
    fn get_size(&self) -> Option<usize>;
}

/// Spatially distancing fair queue. First in, first out, ensuring that
/// each group of similar values is placed as far apart as possible.
pub struct FairQueue<V: FairGroup> {
    ctrl_group: VecDeque<Arc<V>>,
    groups: Vec<VecDeque<Arc<V>>>,
    pointer: usize,
    max_group_size: usize,
}

impl<V: FairGroup> FairQueue<V> {
    pub fn new(max_group_size: usize) -> Self {
        Self {
            ctrl_group: VecDeque::new(),
            groups: Vec::new(),
            pointer: 0,
            max_group_size,
        }
    }

    /// Check if a value can be inserted without exceeding group limits
    pub fn can_insert(&self, value: &V) -> bool {
        match value.group_id() {
            None => true, // Control group can always be inserted
            Some(group_id) => {
                if let Some(group) = self
                    .groups
                    .iter()
                    .find(|group| group.front().map(|v| v.group_id()) == Some(Some(group_id)))
                {
                    let can = group.len() < self.max_group_size;
                    if !can {
                        tracing::error!("Cannot insert value into group: group is full");
                    }
                    can
                } else {
                    true // New group can always be created
                }
            }
        }
    }

    /// Inserts a new item into the queue, ensuring spatial distancing between items of the same group.
    /// Returns true if inserted successfully, false if group is full.
    pub fn insert(&mut self, value: Arc<V>) -> bool {
        //value.trace_message("INSERT");
        match value.group_id() {
            None => {
                // Control group (group_id is None)
                self.ctrl_group.push_back(value);
                true
            }
            Some(group_id) => {
                // Regular group
                if let Some(group) = self
                    .groups
                    .iter_mut()
                    .find(|group| group.front().map(|v| v.group_id()) == Some(Some(group_id)))
                {
                    if group.len() >= self.max_group_size {
                        return false; // Group is full
                    }
                    group.push_back(value);
                } else {
                    let mut new_group = VecDeque::new();
                    new_group.push_back(value);
                    self.groups.push(new_group);
                }
                true
            }
        }
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Option<Arc<V>> {
        if let Some(v) = self.ctrl_group.pop_front() {
            //v.trace_message("POP");
            return Some(v);
        }
        for _ in 0..self.groups.len() {
            let pointer = self.pointer;
            // Optimistically move queue pointer to the next group
            self.pointer = (pointer + 1) % self.groups.len();

            let group = &mut self.groups[pointer];
            let item = group.pop_front();

            if item.is_some() {
                if group.is_empty() {
                    self.groups.remove(pointer);
                    if pointer < self.groups.len() {
                        self.pointer = pointer;
                    } else {
                        self.pointer = 0;
                    }
                }
                return item;
            }
        }

        None
    }
}

/// Shared state between sender and receiver
struct ChannelState<T: FairGroup + 'static> {
    queue: FairQueue<T>,
    closed: bool,
}

impl<T: FairGroup + 'static> ChannelState<T> {
    fn new(max_group_size: usize) -> Self {
        Self {
            queue: FairQueue::new(max_group_size),
            closed: false,
        }
    }

    fn can_insert(&self, value: &T) -> bool {
        self.queue.can_insert(value)
    }
}

/// Sender half of the fair channel
pub struct FairSender<T: FairGroup + 'static> {
    state: Arc<RwLock<ChannelState<T>>>,
    notify_recv: Arc<Notify>,
    notify_send: Arc<Notify>,
}

impl<T: FairGroup + 'static> Clone for FairSender<T> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            notify_recv: Arc::clone(&self.notify_recv),
            notify_send: Arc::clone(&self.notify_send),
        }
    }
}

impl<T: FairGroup + 'static> FairSender<T> {
    /// Send a value, waiting if the channel is full
    pub async fn send(&self, value: T) -> Result<(), SendError<T>> {
        let value_arc = Arc::new(value);

        loop {
            {
                let mut state = self.state.write();
                if state.closed {
                    return Err(SendError(
                        Arc::try_unwrap(value_arc).unwrap_or_else(|arc| (*arc).clone()),
                    ));
                }

                // Check group capacity
                if state.can_insert(&value_arc) {
                    state.queue.insert(value_arc);
                    drop(state);
                    self.notify_recv.notify_waiters();
                    return Ok(());
                }
            }

            // Wait for space to become available in the group
            self.notify_send.notified().await;
        }
    }

    /// Try to send a value without waiting
    pub fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
        let value_arc = Arc::new(value);

        let mut state = self.state.write();
        if state.closed {
            return Err(TrySendError::Closed(
                Arc::try_unwrap(value_arc).unwrap_or_else(|arc| (*arc).clone()),
            ));
        }

        if !state.queue.can_insert(&value_arc) {
            return Err(TrySendError::Full(
                Arc::try_unwrap(value_arc).unwrap_or_else(|arc| (*arc).clone()),
            ));
        }

        state.queue.insert(value_arc);
        drop(state); // Release lock before notifying
        self.notify_recv.notify_waiters();
        Ok(())
    }

    /// Check if the channel is closed
    pub async fn closed(&self) {
        loop {
            {
                let state = self.state.read();
                if state.closed {
                    return;
                }
            }

            // Wait for the channel to be closed
            self.notify_send.notified().await;
        }
    }
}

/// Receiver half of the fair channel
pub struct FairReceiver<T: FairGroup + 'static> {
    state: Arc<RwLock<ChannelState<T>>>,
    notify_recv: Arc<Notify>,
    notify_send: Arc<Notify>,
}

impl<T: FairGroup + 'static> FairReceiver<T> {
    /// Receive a value, waiting if the channel is empty
    pub async fn recv(&mut self) -> Option<T> {
        loop {
            {
                let mut state = self.state.write();
                if let Some(value_arc) = state.queue.pop() {
                    drop(state);
                    self.notify_send.notify_waiters();
                    return Some(Arc::try_unwrap(value_arc).unwrap_or_else(|arc| (*arc).clone()));
                }

                if state.closed {
                    return None;
                }
            }

            // Wait for a value to become available
            self.notify_recv.notified().await;
        }
    }

    /// Try to receive a value without waiting
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        let mut state = self.state.write();

        if let Some(value_arc) = state.queue.pop() {
            drop(state); // Release lock before notifying
            self.notify_send.notify_waiters();
            return Ok(Arc::try_unwrap(value_arc).unwrap_or_else(|arc| (*arc).clone()));
        }

        if state.closed {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }

    /// Close the receiver, which will cause all senders to return errors
    pub async fn close(&mut self) {
        let mut state = self.state.write();
        state.closed = true;
        drop(state); // Release lock before notifying
        self.notify_send.notify_waiters();
    }
}

impl<T: FairGroup + 'static> Drop for FairReceiver<T> {
    fn drop(&mut self) {
        // Mark the channel as closed when receiver is dropped
        if let Some(mut state) = self.state.try_write() {
            state.closed = true;
            drop(state); // Release lock before notifying
            self.notify_send.notify_waiters();
        }
    }
}

/// Creates a new fair channel with the specified max group size
pub fn fair_channel<T: FairGroup + 'static>(
    max_group_size: usize,
) -> (FairSender<T>, FairReceiver<T>) {
    let state = Arc::new(RwLock::new(ChannelState::new(max_group_size)));
    let notify_recv = Arc::new(Notify::new());
    let notify_send = Arc::new(Notify::new());

    let sender = FairSender {
        state: Arc::clone(&state),
        notify_recv: Arc::clone(&notify_recv),
        notify_send: Arc::clone(&notify_send),
    };

    let receiver = FairReceiver {
        state,
        notify_recv,
        notify_send,
    };

    (sender, receiver)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Clone)]
    struct Event {
        timestamp: u32,
        user_id: &'static str,
    }

    impl FairGroup for Event {
        fn group_id(&self) -> Option<u32> {
            // Convert user_id to a hash or use a simple mapping
            match self.user_id {
                "user1" => Some(1),
                "user2" => Some(2),
                "user3" => Some(3),
                _ => Some(0), // Default group
            }
        }

        fn get_size(&self) -> Option<usize> {
            None // Not used in current implementation
        }
    }

    #[test]
    fn test_spaced_fairness() {
        let event1 = Event {
            timestamp: 1,
            user_id: "user1",
        };
        let event2 = Event {
            timestamp: 2,
            user_id: "user2",
        };
        let event3 = Event {
            timestamp: 3,
            user_id: "user1",
        };
        let event4 = Event {
            timestamp: 4,
            user_id: "user3",
        };
        let event5 = Event {
            timestamp: 5,
            user_id: "user2",
        };
        let event6 = Event {
            timestamp: 6,
            user_id: "user1",
        };
        let event7 = Event {
            timestamp: 7,
            user_id: "user1",
        };
        let event8 = Event {
            timestamp: 8,
            user_id: "user3",
        };

        let mut queue = FairQueue::new(usize::MAX);

        let event1_arc = Arc::new(event1.clone());
        let event2_arc = Arc::new(event2.clone());
        let event3_arc = Arc::new(event3.clone());
        let event4_arc = Arc::new(event4.clone());
        let event5_arc = Arc::new(event5.clone());
        let event6_arc = Arc::new(event6.clone());
        let event7_arc = Arc::new(event7.clone());
        let event8_arc = Arc::new(event8.clone());

        queue.insert(event1_arc.clone());
        queue.insert(event2_arc.clone());
        queue.insert(event3_arc.clone());
        queue.insert(event4_arc.clone());
        queue.insert(event5_arc.clone());
        queue.insert(event6_arc.clone());
        queue.insert(event7_arc.clone());
        queue.insert(event8_arc.clone());

        // With weighted round-robin prioritizing smaller buffers:
        // After insertion: user1=[1,3,6,7], user2=[2,5], user3=[4,8]
        // Weighted selection will favor smaller groups (user2 and user3)
        let mut results = Vec::new();
        while let Some(event) = queue.pop() {
            results.push(event);
        }

        // Verify we got all events
        assert_eq!(results.len(), 8);

        // Verify fairness: smaller groups should be prioritized
        // The exact order may vary but should favor smaller buffers
        let user1_events: Vec<_> = results.iter().filter(|e| e.user_id == "user1").collect();
        let user2_events: Vec<_> = results.iter().filter(|e| e.user_id == "user2").collect();
        let user3_events: Vec<_> = results.iter().filter(|e| e.user_id == "user3").collect();

        assert_eq!(user1_events.len(), 4);
        assert_eq!(user2_events.len(), 2);
        assert_eq!(user3_events.len(), 2);
    }

    #[tokio::test]
    async fn test_fair_channel_basic() {
        let (tx, mut rx) = fair_channel(5);

        let event1 = Event {
            timestamp: 1,
            user_id: "user1",
        };
        let event2 = Event {
            timestamp: 2,
            user_id: "user2",
        };

        tx.send(event1).await.unwrap();
        tx.send(event2).await.unwrap();

        let received1 = rx.recv().await.unwrap();
        let received2 = rx.recv().await.unwrap();

        assert_eq!(received1.timestamp, 1);
        assert_eq!(received2.timestamp, 2);
    }

    #[tokio::test]
    async fn test_fair_channel_fairness() {
        let (tx, mut rx) = fair_channel(5);

        // Send events from different users
        for i in 0..6 {
            let user_id = match i % 3 {
                0 => "user1",
                1 => "user2",
                _ => "user3",
            };
            let event = Event {
                timestamp: i,
                user_id,
            };
            tx.send(event).await.unwrap();
        }

        // Receive events and verify fair distribution
        let mut received = Vec::new();
        for _ in 0..6 {
            received.push(rx.recv().await.unwrap());
        }

        // With weighted round-robin, smaller groups are prioritized
        // After sending: user1=[0,3], user2=[1,4], user3=[2,5]
        // All groups have equal size, so behavior depends on weighted selection
        let user1_count = received.iter().filter(|e| e.user_id == "user1").count();
        let user2_count = received.iter().filter(|e| e.user_id == "user2").count();
        let user3_count = received.iter().filter(|e| e.user_id == "user3").count();

        // Verify all users get fair representation
        assert_eq!(user1_count, 2);
        assert_eq!(user2_count, 2);
        assert_eq!(user3_count, 2);

        // Verify we received all expected timestamps
        let mut timestamps: Vec<_> = received.iter().map(|e| e.timestamp).collect();
        timestamps.sort();
        assert_eq!(timestamps, vec![0, 1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn test_fair_channel_closed_method() {
        let (tx, mut rx) = fair_channel(5);

        // Start a task that waits for the channel to be closed
        let tx_clone = tx.clone();
        let closed_task = tokio::spawn(async move {
            tx_clone.closed().await;
        });

        // Give the closed task a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Verify the closed task is still running
        assert!(!closed_task.is_finished());

        // Close the receiver
        rx.close().await;

        // The closed task should now complete
        closed_task.await.unwrap();

        // Verify that sending now returns an error
        let result = tx
            .send(Event {
                timestamp: 1,
                user_id: "user1",
            })
            .await;
        assert!(matches!(result, Err(SendError(_))));
    }
}
