/*
 * Copyright (c) 2023, networkException <git@nwex.de>
 *
 * SPDX-License-Identifier: BSD-2-Clause OR MIT
 */

#[cfg(unix)]
use tokio::net::UnixStream;

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

use crate::unix_tcp::{NamedSocketAddr, SocketAddr};

#[derive(Debug)]
pub enum Stream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl From<TcpStream> for Stream {
    fn from(tcp_stream: TcpStream) -> Self {
        Stream::Tcp(tcp_stream)
    }
}

#[cfg(unix)]
impl From<UnixStream> for Stream {
    fn from(unix_stream: UnixStream) -> Self {
        Stream::Unix(unix_stream)
    }
}

impl Stream {
    pub async fn connect(named_socket_addr: &NamedSocketAddr) -> io::Result<Self> {
        match named_socket_addr {
            NamedSocketAddr::Inet(inet_socket_addr) => {
                TcpStream::connect(inet_socket_addr).await.map(Stream::Tcp)
            }
            #[cfg(unix)]
            NamedSocketAddr::Unix(path) => UnixStream::connect(path).await.map(Stream::Unix),
        }
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        match self {
            Stream::Tcp(tcp_stream) => tcp_stream.local_addr().map(SocketAddr::Inet),
            #[cfg(unix)]
            Stream::Unix(unix_stream) => Ok(SocketAddr::Unix(unix_stream.local_addr()?.into())),
        }
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match self {
            Stream::Tcp(tcp_stream) => tcp_stream.peer_addr().map(SocketAddr::Inet),
            #[cfg(unix)]
            Stream::Unix(unix_stream) => Ok(SocketAddr::Unix(unix_stream.local_addr()?.into())),
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match Pin::into_inner(self) {
            Stream::Tcp(tcp_stream) => Pin::new(tcp_stream).poll_read(cx, buf),
            #[cfg(unix)]
            Stream::Unix(unix_stream) => Pin::new(unix_stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match Pin::into_inner(self) {
            Stream::Tcp(tcp_stream) => Pin::new(tcp_stream).poll_write(cx, buf),
            #[cfg(unix)]
            Stream::Unix(unix_stream) => Pin::new(unix_stream).poll_write(cx, buf),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        match Pin::into_inner(self) {
            Stream::Tcp(tcp_stream) => Pin::new(tcp_stream).poll_write_vectored(cx, bufs),
            #[cfg(unix)]
            Stream::Unix(unix_stream) => Pin::new(unix_stream).poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            Stream::Tcp(tcp_stream) => tcp_stream.is_write_vectored(),
            #[cfg(unix)]
            Stream::Unix(unix_stream) => unix_stream.is_write_vectored(),
        }
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match Pin::into_inner(self) {
            Stream::Tcp(tcp_stream) => Pin::new(tcp_stream).poll_flush(context),
            #[cfg(unix)]
            Stream::Unix(unix_stream) => Pin::new(unix_stream).poll_flush(context),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match Pin::into_inner(self) {
            Stream::Tcp(tcp_stream) => Pin::new(tcp_stream).poll_shutdown(context),
            #[cfg(unix)]
            Stream::Unix(unix_stream) => Pin::new(unix_stream).poll_shutdown(context),
        }
    }
}
