use crate::config::TransportConfig;
use crate::utils::to_socket_addr;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::fmt::{Debug, Display};
#[cfg(unix)]
use std::os::fd::RawFd;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::error;

use crate::protocol::message::Message as ProtocolMessage;

#[async_trait]
pub trait ProtobufStream {
    async fn recv_message(&mut self) -> anyhow::Result<Option<ProtocolMessage>>;
    async fn send_message(&mut self, msg: &ProtocolMessage) -> anyhow::Result<()>;
    async fn close(&mut self) -> anyhow::Result<()>;
}

#[cfg(unix)]
use anyhow::bail;

mod tcp;
pub use tcp::{Listener, NamedSocketAddr, SocketAddr, Stream, TcpTransport};

mod websocket;
pub use websocket::{WebsocketStream, WebsocketTransport};

#[cfg(feature = "rustls")]
pub mod rustls;
#[cfg(feature = "rustls")]
use rustls as tls;
#[cfg(feature = "rustls")]
pub use tls::TlsTransport;

#[derive(Clone)]
pub struct AddrMaybeCached {
    pub addr: String,
    pub socket_addr: Option<NamedSocketAddr>,
}

impl AddrMaybeCached {
    pub fn new(addr: &str) -> AddrMaybeCached {
        AddrMaybeCached {
            addr: addr.to_string(),
            socket_addr: None,
        }
    }

    pub async fn resolve(&mut self) -> Result<()> {
        match to_socket_addr(&self.addr).await {
            Ok(s) => {
                self.socket_addr = Some(NamedSocketAddr::Inet(s));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

impl Display for AddrMaybeCached {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.socket_addr.as_ref() {
            Some(s) => f.write_fmt(format_args!("{}", s)),
            None => f.write_str(&self.addr),
        }
    }
}

/// Specify a transport layer, like TCP, TLS
#[async_trait]
pub trait Transport: Debug + Send + Sync {
    type Acceptor: Send + Sync;
    type RawStream: Send + Sync;
    type Stream: 'static + AsyncRead + AsyncWrite + ProtobufStream + Unpin + Send + Sync + Debug;

    fn new(config: &TransportConfig) -> Result<Self>
    where
        Self: Sized;
    /// Get the stream id, which is used to identify the transport layer
    #[cfg(unix)]
    fn as_raw_fd(conn: &Self::Stream) -> RawFd;
    /// Provide the transport with socket options, which can be handled at the need of the transport
    fn hint(conn: &Self::Stream, opts: SocketOpts);
    async fn bind(&self, addr: NamedSocketAddr) -> Result<Self::Acceptor>;
    /// accept must be cancel safe
    async fn accept(&self, a: &Self::Acceptor) -> Result<(Self::RawStream, SocketAddr)>;
    async fn handshake(&self, conn: Self::RawStream) -> Result<Self::Stream>;
    async fn connect(&self, addr: &AddrMaybeCached) -> Result<Self::Stream>;

    fn get_header(&self, _name: &str) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Keepalive {
    // tcp_keepalive_time if the underlying protocol is TCP
    pub keepalive_secs: u64,
    // tcp_keepalive_intvl if the underlying protocol is TCP
    pub keepalive_interval: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct SocketOpts {
    // None means do not change
    pub nodelay: Option<bool>,
    // keepalive must be Some or None at the same time, or the behavior will be platform-dependent
    pub keepalive: Option<Keepalive>,
    // SO_PRIORITY
    pub priority: Option<u8>,
}

impl SocketOpts {
    /// Socket options for the control channel
    pub fn for_control_channel() -> SocketOpts {
        SocketOpts {
            nodelay: Some(true), // Always set nodelay for the control channel
            keepalive: None,
            priority: Some(0), // Set high priority for the control channel
        }
    }

    pub fn for_data_channel() -> SocketOpts {
        SocketOpts {
            nodelay: Some(true), // Always set nodelay for the data channel
            keepalive: None,
            priority: Some(0),
        }
    }
}

#[cfg(unix)]
pub fn set_reuse(s: &dyn std::os::fd::AsRawFd) -> Result<()> {
    use libc;
    use std::{io, mem};
    unsafe {
        let optval: libc::c_int = 1;
        let ret = libc::setsockopt(
            s.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_REUSEPORT | libc::SO_REUSEADDR,
            &optval as *const _ as *const libc::c_void,
            mem::size_of_val(&optval) as libc::socklen_t,
        );
        if ret != 0 {
            bail!("Set sock option failed: {:?}", io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn set_low_latency(s: &dyn std::os::fd::AsRawFd) -> Result<()> {
    use libc;
    use std::{io, mem};

    unsafe {
        let fd = s.as_raw_fd();
        // 1. TCP_NODELAY - Disable Nagle's algorithm for immediate packet sending
        let nodelay: libc::c_int = 1;
        let ret = libc::setsockopt(
            fd,
            libc::IPPROTO_TCP,
            libc::TCP_NODELAY,
            &nodelay as *const _ as *const libc::c_void,
            mem::size_of_val(&nodelay) as libc::socklen_t,
        );
        if ret != 0 {
            bail!(
                "Failed to set TCP_NODELAY: {:?}",
                io::Error::last_os_error()
            );
        }

        // 2. TCP_QUICKACK - Enable quick ACK mode to reduce ACK delay
        let quickack: libc::c_int = 1;
        let ret = libc::setsockopt(
            fd,
            libc::IPPROTO_TCP,
            libc::TCP_QUICKACK,
            &quickack as *const _ as *const libc::c_void,
            mem::size_of_val(&quickack) as libc::socklen_t,
        );
        if ret != 0 {
            bail!(
                "Failed to set TCP_QUICKACK: {:?}",
                io::Error::last_os_error()
            );
        }
    }
    Ok(())
}

// Set socket priority: 0 - lowest (default), 7 - higest
#[cfg(target_os = "linux")]
pub fn set_priority(s: &dyn std::os::fd::AsRawFd, priority: libc::c_int) -> Result<()> {
    use libc;
    use std::{io, mem};

    unsafe {
        let fd = s.as_raw_fd();

        // 1. SO_PRIORITY - Set high priority for the socket
        let ret = libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PRIORITY,
            &priority as *const _ as *const libc::c_void,
            mem::size_of_val(&priority) as libc::socklen_t,
        );
        if ret != 0 {
            bail!(
                "Failed to set SO_PRIORITY: {:?}",
                io::Error::last_os_error()
            );
        }
    }

    Ok(())
}

impl SocketOpts {
    pub fn apply(&self, conn: &Stream) {
        if let Some(v) = self.keepalive {
            let keepalive_duration = Duration::from_secs(v.keepalive_secs);
            let keepalive_interval = Duration::from_secs(v.keepalive_interval);
            if let Err(e) = tcp::try_set_tcp_keepalive(conn, keepalive_duration, keepalive_interval)
                .with_context(|| "Failed to set keepalive")
            {
                error!("{:#}", e);
            }
        }

        match conn {
            Stream::Tcp(conn) => {
                #[cfg(unix)]
                if let Err(e) = set_reuse(conn) {
                    error!("{:#}", e);
                }
                if let Some(nodelay) = self.nodelay {
                    #[cfg(not(target_os = "linux"))]
                    if let Err(e) = conn
                        .set_nodelay(nodelay)
                        .with_context(|| "Failed to set nodelay")
                    {
                        error!("{:#}", e);
                    }
                    #[cfg(target_os = "linux")]
                    if nodelay {
                        if let Err(e) = set_low_latency(conn) {
                            error!("Failed to set low latency options: {:#}", e);
                        }
                    }
                }
                #[cfg(target_os = "linux")]
                if let Some(priority) = self.priority {
                    if let Err(e) = set_priority(conn, priority as libc::c_int) {
                        error!("Failed to set socket priority: {:#}", e);
                    }
                }
            }
            #[cfg(unix)]
            Stream::Unix(_conn) =>
            {
                #[cfg(target_os = "linux")]
                if let Some(priority) = self.priority {
                    if let Err(e) = set_priority(_conn, priority as libc::c_int) {
                        error!("Failed to set socket priority: {:#}", e);
                    }
                }
            }
        }
    }
}
