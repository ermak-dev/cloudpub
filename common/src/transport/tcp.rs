use crate::config::{TcpConfig, TransportConfig};

use super::{AddrMaybeCached, ProtobufStream, SocketOpts, Transport};
use crate::utils::host_port_pair;
use anyhow::{Context as _, Result};
use async_http_proxy::{http_connect_tokio, http_connect_tokio_with_basic_auth};
use async_trait::async_trait;
use socket2::{SockRef, TcpKeepalive};
#[cfg(unix)]
use std::os::fd::RawFd;
use std::str::FromStr;
use std::time::Duration;
pub use tokio_unix_tcp::{Listener, NamedSocketAddr, SocketAddr, Stream};
type RawTcpStream = Stream;
use crate::protocol::message::Message as ProtocolMessage;
use crate::protocol::{read_message, write_message};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tracing::trace;
use url::Url;

#[derive(Debug)]
pub struct TcpStream {
    inner: RawTcpStream,
}

impl TcpStream {
    pub fn new(stream: RawTcpStream) -> Self {
        Self { inner: stream }
    }

    pub fn into_inner(self) -> RawTcpStream {
        self.inner
    }

    pub fn get_ref(&self) -> &RawTcpStream {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut RawTcpStream {
        &mut self.inner
    }

    pub fn into_stream(self) -> Stream {
        self.inner
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[async_trait]
impl ProtobufStream for TcpStream {
    async fn recv_message(&mut self) -> anyhow::Result<Option<ProtocolMessage>> {
        match read_message(&mut self.inner).await {
            Ok(msg) => Ok(Some(msg)),
            Err(e) => {
                if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
                    if io_err.kind() == std::io::ErrorKind::UnexpectedEof {
                        return Ok(None);
                    }
                }
                Err(e)
            }
        }
    }

    async fn send_message(&mut self, msg: &ProtocolMessage) -> anyhow::Result<()> {
        write_message(&mut self.inner, msg).await
    }

    async fn close(&mut self) -> anyhow::Result<()> {
        self.inner
            .shutdown()
            .await
            .context("Failed to shutdown stream")
    }
}

#[derive(Debug)]
pub struct TcpTransport {
    pub socket_opts: SocketOpts,
    pub cfg: TcpConfig,
}

#[async_trait]
impl Transport for TcpTransport {
    type Acceptor = Listener;
    type Stream = TcpStream;
    type RawStream = Stream;

    fn new(config: &TransportConfig) -> Result<Self> {
        Ok(TcpTransport {
            socket_opts: SocketOpts::for_control_channel(),
            cfg: config.tcp.clone(),
        })
    }

    #[cfg(unix)]
    fn as_raw_fd(conn: &Self::Stream) -> RawFd {
        use std::os::fd::AsRawFd;
        match conn.get_ref() {
            Stream::Tcp(tcp_stream) => tcp_stream.as_raw_fd(),
            #[cfg(unix)]
            Stream::Unix(unix_stream) => unix_stream.as_raw_fd(),
        }
    }

    fn hint(conn: &Self::Stream, opt: SocketOpts) {
        opt.apply(conn.get_ref());
    }

    async fn bind(&self, addr: NamedSocketAddr) -> Result<Self::Acceptor> {
        #[cfg(unix)]
        if let NamedSocketAddr::Unix(path) = &addr {
            // Ensure the socket file is removed if it exists
            if path.exists() {
                tokio::fs::remove_file(path).await?;
            }
        }
        Ok(Listener::bind(&addr).await?)
    }

    async fn accept(&self, a: &Self::Acceptor) -> Result<(Self::RawStream, SocketAddr)> {
        let (s, addr) = a.accept().await?;
        self.socket_opts.apply(&s);
        Ok((s, addr))
    }

    async fn handshake(&self, conn: Self::RawStream) -> Result<Self::Stream> {
        Ok(TcpStream::new(conn))
    }

    async fn connect(&self, addr: &AddrMaybeCached) -> Result<Self::Stream> {
        let s = tcp_connect_with_proxy(addr, self.cfg.proxy.as_ref()).await?;
        self.socket_opts.apply(&s);
        Ok(TcpStream::new(s))
    }
}

// Tokio hesitates to expose this option...So we have to do it on our own :(
// The good news is that using socket2 it can be easily done, without losing portability.
// See https://github.com/tokio-rs/tokio/issues/3082
pub fn try_set_tcp_keepalive(
    conn: &RawTcpStream,
    keepalive_duration: Duration,
    keepalive_interval: Duration,
) -> Result<()> {
    match conn {
        Stream::Tcp(tcp_stream) => {
            let s = SockRef::from(tcp_stream);
            let keepalive = TcpKeepalive::new()
                .with_time(keepalive_duration)
                .with_interval(keepalive_interval);

            trace!(
                "Set TCP keepalive {:?} {:?}",
                keepalive_duration,
                keepalive_interval
            );

            Ok(s.set_tcp_keepalive(&keepalive)?)
        }
        #[cfg(unix)]
        Stream::Unix(_) => {
            // Unix sockets don't support TCP keepalive
            Ok(())
        }
    }
}

/// Create a TcpStream using a proxy
/// e.g. socks5://user:pass@127.0.0.1:1080 http://127.0.0.1:8080
pub async fn tcp_connect_with_proxy(addr: &AddrMaybeCached, proxy: Option<&Url>) -> Result<Stream> {
    if let Some(url) = proxy {
        let addr = &addr.addr;
        let proxy_addr = format!(
            "{}:{}",
            url.host_str().expect("proxy url should have host field"),
            url.port().expect("proxy url should have port field")
        );
        let mut s = Stream::connect(&NamedSocketAddr::from_str(&proxy_addr)?).await?;

        let auth = if !url.username().is_empty() || url.password().is_some() {
            Some(async_socks5::Auth {
                username: url.username().into(),
                password: url.password().unwrap_or("").into(),
            })
        } else {
            None
        };
        match url.scheme() {
            "socks5" => {
                async_socks5::connect(&mut s, host_port_pair(addr)?, auth).await?;
            }
            "http" => {
                let (host, port) = host_port_pair(addr)?;
                match auth {
                    Some(auth) => {
                        http_connect_tokio_with_basic_auth(
                            &mut s,
                            host,
                            port,
                            &auth.username,
                            &auth.password,
                        )
                        .await?
                    }
                    None => http_connect_tokio(&mut s, host, port).await?,
                }
            }
            _ => panic!("unknown proxy scheme"),
        }
        Ok(s)
    } else {
        Ok(match addr.socket_addr.as_ref() {
            Some(s) => Stream::connect(s).await?,
            None => Stream::connect(&NamedSocketAddr::from_str(&addr.addr)?).await?,
        })
    }
}
