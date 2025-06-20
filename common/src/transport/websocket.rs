use core::result::Result;
use std::io::{Error, ErrorKind};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use super::{
    AddrMaybeCached, Listener, NamedSocketAddr, ProtobufStream, SocketAddr, SocketOpts, Stream,
    TcpTransport, Transport,
};
use crate::config::TransportConfig;
use anyhow::{anyhow, Context as _};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use futures_core::stream::Stream as AsyncStream;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

use crate::utils::trace_message;
use parking_lot::RwLock;
use std::collections::HashMap;
#[cfg(unix)]
use std::os::fd::RawFd;
use std::sync::Arc;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::{Message, WebSocketConfig};
use tokio_tungstenite::{accept_hdr_async_with_config, client_async_with_config, WebSocketStream};
use tokio_util::io::StreamReader;
use tracing::{debug, error, trace};
use url::Url;

use futures_util::sink::{Sink, SinkExt};
use futures_util::stream::StreamExt;

#[cfg(feature = "rustls")]
use super::tls::{get_stream, TlsStream, TlsTransport};

use crate::protocol::v2::message::Message as ProtocolMessage;
use crate::protocol::v2::ProstMessage;

#[derive(Debug)]
enum TransportStream {
    Insecure(Stream),
    #[cfg(feature = "rustls")]
    Secure(TlsStream<Stream>),
}

impl TransportStream {
    fn get_tcpstream(&self) -> &Stream {
        match self {
            TransportStream::Insecure(s) => s,
            #[cfg(feature = "rustls")]
            TransportStream::Secure(s) => get_stream(s),
        }
    }
}

impl AsyncRead for TransportStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            TransportStream::Insecure(s) => Pin::new(s).poll_read(cx, buf),
            #[cfg(feature = "rustls")]
            TransportStream::Secure(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for TransportStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            TransportStream::Insecure(s) => Pin::new(s).poll_write(cx, buf),
            #[cfg(feature = "rustls")]
            TransportStream::Secure(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            TransportStream::Insecure(s) => Pin::new(s).poll_flush(cx),
            #[cfg(feature = "rustls")]
            TransportStream::Secure(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            TransportStream::Insecure(s) => Pin::new(s).poll_shutdown(cx),
            #[cfg(feature = "rustls")]
            TransportStream::Secure(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

#[derive(Debug)]
struct StreamWrapper {
    inner: WebSocketStream<TransportStream>,
}

#[async_trait]
impl ProtobufStream for StreamWrapper {
    async fn recv_message(&mut self) -> anyhow::Result<Option<ProtocolMessage>> {
        match self.inner.next().await {
            Some(Ok(Message::Binary(b))) => {
                let msg = crate::protocol::v2::Message::decode(b.as_ref())
                    .context("Failed to decode protobuf message")?;
                let msg = msg
                    .message
                    .context("Message field is missing in the protobuf message")?;
                trace_message("Recv", &msg);
                Ok(Some(msg))
            }
            Some(Ok(Message::Close(_))) => {
                debug!("WebSocket connection closed");
                Ok(None)
            }
            Some(Ok(Message::Ping(data))) => {
                debug!("Received ping, sending pong");
                self.inner
                    .send(Message::Pong(data))
                    .await
                    .context("Failed to send pong")?;
                Ok(None)
            }
            Some(Ok(Message::Pong(_))) => {
                debug!("Received pong");
                Ok(None)
            }
            Some(Ok(Message::Text(_))) => {
                error!("Received unexpected text message");
                Err(anyhow!("Unexpected text message received"))
            }
            Some(Ok(m)) => {
                error!("Received unexpected  message: {:?}", m);
                Err(anyhow!("Unexpected  message received"))
            }
            None => Ok(None),
            Some(Err(e)) => Err(anyhow!("WebSocket error: {}", e)),
        }
    }

    async fn send_message(
        &mut self,
        msg: &crate::protocol::v2::message::Message,
    ) -> anyhow::Result<()> {
        trace_message("Send", msg);
        let mut buf = BytesMut::new();
        let msg = crate::protocol::v2::Message {
            message: Some(msg.clone()),
        };
        msg.encode(&mut buf)
            .context("Failed to encode protobuf message")?;
        self.inner
            .send(Message::Binary(buf.into()))
            .await
            .context("Failed to send WebSocket message")?;
        Ok(())
    }
}

impl AsyncStream for StreamWrapper {
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.get_mut().inner).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(Err(err))) => {
                Poll::Ready(Some(Err(Error::new(ErrorKind::Other, err))))
            }
            Poll::Ready(Some(Ok(res))) => {
                if let Message::Binary(b) = res {
                    Poll::Ready(Some(Ok(b.into())))
                } else {
                    Poll::Ready(Some(Err(Error::new(
                        ErrorKind::InvalidData,
                        "unexpected frame",
                    ))))
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[derive(Debug)]
pub struct WebsocketStream {
    inner: StreamReader<StreamWrapper, Bytes>,
}

impl AsyncRead for WebsocketStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncBufRead for WebsocketStream {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<&[u8]>> {
        Pin::new(&mut self.get_mut().inner).poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut self.get_mut().inner).consume(amt)
    }
}

impl AsyncWrite for WebsocketStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let sw = self.get_mut().inner.get_mut();
        ready!(Pin::new(&mut sw.inner)
            .poll_ready(cx)
            .map_err(|err| Error::new(ErrorKind::Other, err)))?;

        let bbuf = BytesMut::from(buf);

        match Pin::new(&mut sw.inner).start_send(Message::Binary(bbuf.into())) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(e) => Poll::Ready(Err(Error::new(ErrorKind::Other, e))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.get_mut().inner.get_mut().inner)
            .poll_flush(cx)
            .map_err(|err| Error::new(ErrorKind::Other, err))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.get_mut().inner.get_mut().inner)
            .poll_close(cx)
            .map_err(|err| Error::new(ErrorKind::Other, err))
    }
}

#[async_trait]
impl ProtobufStream for WebsocketStream {
    async fn recv_message(&mut self) -> anyhow::Result<Option<ProtocolMessage>> {
        self.inner.get_mut().recv_message().await
    }

    async fn send_message(&mut self, msg: &ProtocolMessage) -> anyhow::Result<()> {
        self.inner.get_mut().send_message(msg).await
    }
}

#[derive(Debug)]
enum SubTransport {
    #[cfg(feature = "rustls")]
    Secure(TlsTransport),
    Insecure(TcpTransport),
}

#[derive(Debug)]
pub struct WebsocketTransport {
    sub: SubTransport,
    conf: WebSocketConfig,
    headers: Arc<RwLock<HashMap<String, String>>>,
}

#[async_trait]
impl Transport for WebsocketTransport {
    type Acceptor = Listener;
    type RawStream = Stream;
    type Stream = WebsocketStream;

    fn new(config: &TransportConfig) -> anyhow::Result<Self> {
        let wsconfig = config
            .websocket
            .as_ref()
            .ok_or_else(|| anyhow!("Missing websocket config"))?;

        let conf = WebSocketConfig {
            write_buffer_size: 0,
            ..Default::default()
        };

        let sub = match wsconfig.tls {
            #[cfg(feature = "rustls")]
            true => SubTransport::Secure(TlsTransport::new(config)?),
            #[cfg(not(feature = "rustls"))]
            true => unreachable!("TLS support not enabled"),
            false => SubTransport::Insecure(TcpTransport::new(config)?),
        };
        let headers = Default::default();
        Ok(WebsocketTransport { sub, conf, headers })
    }

    fn hint(conn: &Self::Stream, opt: SocketOpts) {
        opt.apply(conn.inner.get_ref().inner.get_ref().get_tcpstream())
    }

    #[cfg(unix)]
    fn as_raw_fd(conn: &Self::Stream) -> RawFd {
        match conn.inner.get_ref().inner.get_ref().get_tcpstream() {
            Stream::Tcp(tcp_stream) => tcp_stream.as_raw_fd(),
            Stream::Unix(unix_stream) => unix_stream.as_raw_fd(),
        }
    }

    async fn bind(&self, addr: NamedSocketAddr) -> anyhow::Result<Self::Acceptor> {
        Listener::bind(&addr).await.map_err(Into::into)
    }

    async fn accept(&self, a: &Self::Acceptor) -> anyhow::Result<(Self::RawStream, SocketAddr)> {
        let (s, addr) = match &self.sub {
            SubTransport::Insecure(t) => t.accept(a).await?,
            #[cfg(feature = "rustls")]
            SubTransport::Secure(t) => t.accept(a).await?,
        };
        Ok((s, addr))
    }

    async fn handshake(&self, conn: Self::RawStream) -> anyhow::Result<Self::Stream> {
        let tsream = match &self.sub {
            SubTransport::Insecure(t) => {
                TransportStream::Insecure(t.handshake(conn).await?.into_stream())
            }
            #[cfg(feature = "rustls")]
            SubTransport::Secure(t) => TransportStream::Secure(t.handshake(conn).await?),
        };

        let headers = self.headers.clone();

        let callback = move |req: &Request, res: Response| {
            let mut headers = headers.write();
            for ref header in req.headers() {
                trace!("WS headers: {:?}", header);
                headers.insert(
                    header.0.to_string(),
                    header.1.to_str().unwrap_or_default().to_string(),
                );
            }
            Ok(res)
        };

        let wsstream = accept_hdr_async_with_config(tsream, callback, Some(self.conf)).await?;

        let tun = WebsocketStream {
            inner: StreamReader::new(StreamWrapper { inner: wsstream }),
        };
        Ok(tun)
    }

    async fn connect(&self, addr: &AddrMaybeCached) -> anyhow::Result<Self::Stream> {
        let u = format!("wss://{}/endpoint/v3", &addr.addr.as_str());
        let url = match Url::parse(&u) {
            Ok(parsed_url) => parsed_url,
            Err(e) => {
                error!("Failed to parse URL: {:?}", e);
                return Err(e.into());
            }
        };
        let tstream = match &self.sub {
            SubTransport::Insecure(t) => {
                TransportStream::Insecure(t.connect(addr).await?.into_stream())
            }
            #[cfg(feature = "rustls")]
            SubTransport::Secure(t) => TransportStream::Secure(t.connect(addr).await?),
        };
        debug!("Connecting to {}", &url);
        let (wsstream, _) = client_async_with_config(url, tstream, Some(self.conf))
            .await
            .with_context(|| format!("Failed to connect to {}", u))?;

        debug!("Connected");

        let tun = WebsocketStream {
            inner: StreamReader::new(StreamWrapper { inner: wsstream }),
        };
        Ok(tun)
    }

    fn get_header(&self, name: &str) -> Option<String> {
        self.headers.read().get(&name.to_lowercase()).cloned()
    }
}
