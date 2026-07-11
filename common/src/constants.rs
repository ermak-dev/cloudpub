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
pub const MESSAGE_TIMEOUT_SECS: u64 = 60;

/// Deadline for establishing a connection: TCP connect, TLS handshake and the
/// WebSocket upgrade. None of those steps carry a protocol-level message, so
/// `MESSAGE_TIMEOUT_SECS` does not cover them - without this a peer that
/// accepts the connection and then goes silent parks the dial forever.
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 30;

/// WebSocket read buffer chunk size (tungstenite `read_buffer_size`).
///
/// tungstenite's `FrameCodec::read_in` zero-fills its input buffer up to
/// `len + read_buffer_size` on every read cycle (see frame/mod.rs `resize(.., 0)`).
/// The 128 KiB default means each read memset-zeroes 128 KiB regardless of the
/// actual payload - profiling the server showed this zero-fill at ~8% of the
/// endpoint thread's CPU. 16 KiB caps the wasted zeroing while staying a
/// reasonable per-syscall read chunk for bulk tunnel traffic.
pub const WEBSOCKET_READ_BUFFER_SIZE: usize = 16 * 1024;

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
