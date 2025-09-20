/*
 * Copyright (c) 2023, networkException <git@nwex.de>
 *
 * SPDX-License-Identifier: BSD-2-Clause OR MIT
 */

use std::io;

#[cfg(unix)]
use std::{
    fs::{self, Permissions},
    os::unix::prelude::PermissionsExt,
};

use tokio::net::TcpListener;

use crate::unix_tcp::{NamedSocketAddr, SocketAddr, Stream};

#[cfg(unix)]
use tokio::net::UnixListener;

#[derive(Debug)]
pub enum Listener {
    Tcp(TcpListener),
    #[cfg(unix)]
    Unix(UnixListener),
}

impl From<TcpListener> for Listener {
    fn from(listener: TcpListener) -> Listener {
        Listener::Tcp(listener)
    }
}

#[cfg(unix)]
impl From<UnixListener> for Listener {
    fn from(listener: UnixListener) -> Listener {
        Listener::Unix(listener)
    }
}

impl Listener {
    // On non unix systems, remove and mode are not used.
    #[cfg_attr(not(unix), allow(unused_variables))]
    pub async fn bind_and_prepare_unix(
        named_socket_addr: &NamedSocketAddr,
        remove: bool,
        mode: Option<u32>,
    ) -> io::Result<Listener> {
        match named_socket_addr {
            NamedSocketAddr::Inet(inet_socket_addr) => {
                TcpListener::bind(inet_socket_addr).await.map(Listener::Tcp)
            }
            #[cfg(unix)]
            NamedSocketAddr::Unix(path) => {
                if remove && path.exists() {
                    fs::remove_file(path)?
                }

                let bound = UnixListener::bind(path)?;

                fs::set_permissions(path, Permissions::from_mode(mode.unwrap_or(0o222)))?;

                Ok(Listener::Unix(bound))
            }
        }
    }

    pub async fn bind(named_socket_addr: &NamedSocketAddr) -> io::Result<Listener> {
        match named_socket_addr {
            NamedSocketAddr::Inet(inet_socket_addr) => {
                TcpListener::bind(inet_socket_addr).await.map(Listener::Tcp)
            }
            #[cfg(unix)]
            NamedSocketAddr::Unix(path) => UnixListener::bind(path).map(Listener::Unix),
        }
    }

    pub async fn accept(&self) -> io::Result<(Stream, SocketAddr)> {
        match self {
            Listener::Tcp(listener) => {
                listener
                    .accept()
                    .await
                    .map(|(tcp_stream, inet_socket_addr)| {
                        (Stream::Tcp(tcp_stream), SocketAddr::Inet(inet_socket_addr))
                    })
            }
            #[cfg(unix)]
            Listener::Unix(listener) => {
                listener
                    .accept()
                    .await
                    .map(|(unix_stream, unix_socket_addr)| {
                        (
                            Stream::Unix(unix_stream),
                            SocketAddr::Unix(unix_socket_addr.into()),
                        )
                    })
            }
        }
    }
}
