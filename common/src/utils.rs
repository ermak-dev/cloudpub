use anyhow::{anyhow, Context, Result};
use backoff::backoff::Backoff;
use backoff::Notify;
use std::future::Future;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use tokio::net::{lookup_host, TcpListener, TcpSocket, ToSocketAddrs, UdpSocket};
use tokio::sync::watch;
use tracing::{debug, trace};

use crate::protocol::v2::message::Message as ProtocolMessage;
use futures::future::{BoxFuture, FutureExt};

pub fn box_future<F, T>(future: F) -> BoxFuture<'static, T>
where
    F: Future<Output = T> + Send + 'static,
    T: 'static,
{
    future.boxed()
}

pub async fn to_socket_addr<A: ToSocketAddrs>(addr: A) -> Result<std::net::SocketAddr> {
    lookup_host(addr)
        .await?
        .next()
        .ok_or_else(|| anyhow!("Failed to lookup the host"))
}

pub fn host_port_pair(s: &str) -> Result<(&str, u16)> {
    let semi = s.rfind(':').expect("missing semicolon");
    Ok((&s[..semi], s[semi + 1..].parse()?))
}

/// Create a UDP socket and connect to `addr`
pub async fn udp_connect<A: ToSocketAddrs>(addr: A) -> Result<UdpSocket> {
    let addr = to_socket_addr(addr).await?;

    let bind_addr = match addr {
        std::net::SocketAddr::V4(_) => "0.0.0.0:0",
        std::net::SocketAddr::V6(_) => ":::0",
    };

    let s = UdpSocket::bind(bind_addr).await?;
    s.connect(addr).await?;
    Ok(s)
}

// Wrapper of retry_notify
pub async fn retry_notify_with_deadline<I, E, Fn, Fut, B, N>(
    backoff: B,
    operation: Fn,
    notify: N,
    deadline: &mut watch::Receiver<bool>,
) -> Result<I>
where
    E: std::error::Error + Send + Sync + 'static,
    B: Backoff,
    Fn: FnMut() -> Fut,
    Fut: Future<Output = std::result::Result<I, backoff::Error<E>>>,
    N: Notify<E>,
{
    tokio::select! {
        v = backoff::future::retry_notify(backoff, operation, notify) => {
            v.map_err(anyhow::Error::new)
        }
        _ = deadline.changed() => {
            Err(anyhow!("shutdown"))
        }
    }
}

pub async fn find_free_tcp_port() -> Result<u16> {
    let tcp_listener = TcpListener::bind("0.0.0.0:0").await?;
    let port = tcp_listener.local_addr()?.port();
    Ok(port)
}

pub async fn find_free_udp_port() -> Result<u16> {
    let udp_listener = UdpSocket::bind("0.0.0.0:0").await?;
    let port = udp_listener.local_addr()?.port();
    Ok(port)
}

pub async fn is_udp_port_available(bind_addr: &str, port: u16) -> Result<bool> {
    match UdpSocket::bind((bind_addr, port)).await {
        Ok(_) => Ok(true),
        Err(ref e) if e.kind() == io::ErrorKind::AddrInUse => Ok(false),
        Err(e) => Err(e).context("Failed to check UDP port")?,
    }
}

pub async fn is_tcp_port_available(bind_addr: &str, port: u16) -> Result<bool> {
    let tcp_socket = TcpSocket::new_v4()?;
    let bind_addr: SocketAddr = format!("{}:{}", bind_addr, port)
        .parse()
        .with_context(|| format!("Failed to parse bind address: {}:{}", bind_addr, port))?;
    debug!("Check port: {}", bind_addr);
    match tcp_socket.bind(bind_addr) {
        Ok(_) => Ok(true),
        Err(ref e) if e.kind() == io::ErrorKind::AddrInUse => Ok(false),
        Err(e) => Err(e).context("Failed to check TCP port")?,
    }
}

pub fn get_version_number(version: &str) -> i64 {
    let mut n = 0;
    for x in version.split(".") {
        n = n * 10000 + x.parse::<i64>().unwrap_or(0);
    }
    n
}

pub fn get_platform() -> String {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let platform = "linux-x86_64".to_string();
    #[cfg(all(target_os = "linux", target_arch = "arm"))]
    let platform = "linux-armv7".to_string();
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let platform = "linux-aarch64".to_string();
    #[cfg(all(target_os = "linux", target_arch = "x86"))]
    let platform = "linux-i686".to_string();
    #[cfg(all(target_os = "linux", target_arch = "mips", target_endian = "big"))]
    let platform = "linux-mips".to_string();
    #[cfg(all(target_os = "linux", target_arch = "mips", target_endian = "little"))]
    let platform = "linux-mipsel".to_string();
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let platform = "windows-x86_64".to_string();
    #[cfg(all(target_os = "windows", target_arch = "x86"))]
    let platform = "windows-i686".to_string();
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let platform = "macos-x86_64".to_string();
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let platform = "macos-aarch64".to_string();
    platform
}

pub fn split_host_port(host_and_port: &str, default_port: u16) -> (String, u16) {
    let parts = host_and_port.split(':');
    let parts: Vec<&str> = parts.collect();
    let host = parts[0].to_string();
    let port = if parts.len() > 1 {
        parts[1].parse::<u16>().unwrap_or(default_port)
    } else {
        default_port
    };
    (host, port)
}

pub fn socket_addr_to_proto(addr: &SocketAddr) -> crate::protocol::v2::SocketAddr {
    match addr {
        SocketAddr::V4(addr_v4) => crate::protocol::v2::SocketAddr {
            addr: Some(crate::protocol::v2::socket_addr::Addr::V4(
                crate::protocol::v2::SocketAddrV4 {
                    ip: u32::from(*addr_v4.ip()),
                    port: addr_v4.port() as u32,
                },
            )),
        },
        SocketAddr::V6(addr_v6) => crate::protocol::v2::SocketAddr {
            addr: Some(crate::protocol::v2::socket_addr::Addr::V6(
                crate::protocol::v2::SocketAddrV6 {
                    ip: addr_v6.ip().octets().to_vec(),
                    port: addr_v6.port() as u32,
                    flowinfo: addr_v6.flowinfo(),
                    scope_id: addr_v6.scope_id(),
                },
            )),
        },
    }
}

pub fn proto_to_socket_addr(proto_addr: &crate::protocol::v2::SocketAddr) -> Result<SocketAddr> {
    match &proto_addr.addr {
        Some(crate::protocol::v2::socket_addr::Addr::V4(v4)) => Ok(SocketAddr::V4(
            SocketAddrV4::new(Ipv4Addr::from(v4.ip), v4.port as u16),
        )),
        Some(crate::protocol::v2::socket_addr::Addr::V6(v6)) => {
            if v6.ip.len() == 16 {
                let mut ip_bytes = [0u8; 16];
                ip_bytes.copy_from_slice(&v6.ip);
                Ok(SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from(ip_bytes),
                    v6.port as u16,
                    v6.flowinfo,
                    v6.scope_id,
                )))
            } else {
                Err(anyhow!(
                    "Invalid IPv6 address length: expected 16 bytes, got {}",
                    v6.ip.len()
                ))
            }
        }
        None => Err(anyhow!("Missing socket address")),
    }
}

pub fn trace_message(label: &str, msg: &ProtocolMessage) {
    match msg {
        ProtocolMessage::CreateDataChannelWithId(data) => {
            trace!(
                "{}: CreateDataChannelWithId {{ channel_id: {}, {:?} }}",
                label,
                data.channel_id,
                data.endpoint
            );
        }
        ProtocolMessage::DataChannelData(data) => {
            //let data_str = String::from_utf8_lossy(&data.data);
            trace!(
                "{}: DataChannelData {{ channel_id: {}, data_size: {} bytes }}",
                label,
                data.channel_id,
                data.data.len(),
            );
        }
        ProtocolMessage::DataChannelDataUdp(data) => {
            trace!(
                "{}: DataChannelDataUdp {{ channel_id: {}, data_size: {} bytes, socket_addr: {:?} }}",
                label,
                data.channel_id,
                data.data.len(),
                data.socket_addr
            );
        }
        ProtocolMessage::DataChannelAck(data) => {
            trace!(
                "{}: DataChannelAck {{ channel_id: {}, consumed: {} bytes }}",
                label,
                data.channel_id,
                data.consumed
            );
        }
        ProtocolMessage::DataChannelEof(data) => {
            trace!(
                "{}: DataChannelEof {{ channel_id: {}, error: {} }}",
                label,
                data.channel_id,
                data.error
            );
        }
        ProtocolMessage::HeartBeat(_) => {
            trace!("{}: HeartBeat", label);
        }
        _ => {
            debug!("{}: {:?}", label, msg);
        }
    }
}
