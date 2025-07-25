use backoff::ExponentialBackoff;
use std::time::Duration;

/// Application-layer heartbeat interval in secs
pub const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 30;
pub const DEFAULT_HEARTBEAT_TIMEOUT_SECS: u64 = 40;

pub const DATA_BUFFER_SIZE: usize = 16384; // 16 KiB, a reasonable size for data buffers
pub const DATA_CHANNEL_SIZE: usize = 1024; // Packets in data channel
pub const CONTROL_CHANNEL_SIZE: usize = 1024; // Packets in control channel

/// Client
pub const DEFAULT_CLIENT_RETRY_INTERVAL_SECS: u64 = 60;
pub const DEFAULT_CLIENT_DATA_CHANNEL_CAPACITY: u32 = DATA_BUFFER_SIZE as u32 * 4;

/// Server
pub const BACKLOG_SIZE: usize = 1024; // The capacity TCP incoming conn backlog
pub const HANDSHAKE_TIMEOUT: u64 = 5; // Timeout for transport handshake

/// TCP
pub const DEFAULT_NODELAY: bool = true;
pub const DEFAULT_KEEPALIVE_SECS: u64 = 20;
pub const DEFAULT_KEEPALIVE_INTERVAL: u64 = 8;

// FIXME: Determine reasonable size
/// UDP MTU. Currently far larger than necessary
pub const UDP_BUFFER_SIZE: usize = 2048;
pub const UDP_SENDQ_SIZE: usize = 1024;
pub const UDP_TIMEOUT: u64 = 60;

// Pingora service param
pub const LISTENERS_PER_FD: usize = 1;

pub fn listen_backoff() -> ExponentialBackoff {
    ExponentialBackoff {
        max_elapsed_time: None,
        max_interval: Duration::from_secs(1),
        ..Default::default()
    }
}

pub fn run_control_chan_backoff(interval: u64) -> ExponentialBackoff {
    ExponentialBackoff {
        randomization_factor: 0.2,
        max_elapsed_time: None,
        multiplier: 3.0,
        max_interval: Duration::from_secs(interval),
        ..Default::default()
    }
}
