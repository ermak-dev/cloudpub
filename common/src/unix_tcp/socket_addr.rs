/*
 * Copyright (c) 2023, networkException <git@nwex.de>
 *
 * SPDX-License-Identifier: BSD-2-Clause OR MIT
 */

use std::fmt::{self, Debug, Display, Formatter};
use std::io;
use std::net::{self, AddrParseError};
use std::str::FromStr;
// NOTE: PathBuf is used in the signature of functions that also need to
//       be available on non unix systems (at least for a noop).
use std::path::PathBuf;

#[cfg(unix)]
use tokio::net::unix;

#[cfg(feature = "serde")]
use serde::{de::Error, Deserialize, Deserializer, Serialize};

// NOTE: This enum is used in the signature of functions that also need to
//       be available on non unix systems (at least for a noop).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnixSocketAddr {
    #[cfg(unix)]
    AbstractOrUnnamed,
    #[cfg(unix)]
    Pathname(PathBuf),
}

#[cfg(unix)]
impl UnixSocketAddr {
    pub fn is_pathname(input: &str) -> bool {
        input.starts_with('/') || input.starts_with('.')
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SocketAddr {
    Inet(net::SocketAddr),
    #[cfg(unix)]
    Unix(UnixSocketAddr),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NamedSocketAddr {
    Inet(net::SocketAddr),
    #[cfg(unix)]
    Unix(PathBuf),
}

impl SocketAddr {
    pub fn map_inet<F: FnOnce(net::SocketAddr) -> net::SocketAddr>(self, mapper: F) -> SocketAddr {
        match self {
            SocketAddr::Inet(inet_socket_addr) => SocketAddr::Inet(mapper(inet_socket_addr)),
            #[cfg(unix)]
            SocketAddr::Unix(unix_socket_addr) => SocketAddr::Unix(unix_socket_addr),
        }
    }

    // On non unix systems, op is not used.
    #[cfg_attr(not(unix), allow(unused_variables))]
    pub fn map_unix<F: FnOnce(UnixSocketAddr) -> UnixSocketAddr>(self, mapper: F) -> SocketAddr {
        match self {
            SocketAddr::Inet(inet_socket_addr) => SocketAddr::Inet(inet_socket_addr),
            #[cfg(unix)]
            SocketAddr::Unix(unix_socket_addr) => SocketAddr::Unix(mapper(unix_socket_addr)),
        }
    }

    pub fn to_named_socket_addr(self) -> io::Result<NamedSocketAddr> {
        match self {
            SocketAddr::Inet(inet_socket_addr) => Ok(NamedSocketAddr::Inet(inet_socket_addr)),
            #[cfg(unix)]
            SocketAddr::Unix(UnixSocketAddr::Pathname(pathname)) => {
                Ok(NamedSocketAddr::Unix(pathname))
            }
            #[cfg(unix)]
            SocketAddr::Unix(UnixSocketAddr::AbstractOrUnnamed) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Cannot connect to a abstract or unnamed unix socket.",
            )),
        }
    }

    #[cfg_attr(feature = "serde", allow(unused))]
    #[cfg(feature = "serde")]
    pub fn deserialize_from_str<'de, D>(deserializer: D) -> Result<SocketAddr, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        FromStr::from_str(&string).map_err(Error::custom)
    }

    #[cfg_attr(feature = "serde", allow(unused))]
    #[cfg(feature = "serde")]
    pub fn deserialize_from_option_str<'de, D>(
        deserializer: D,
    ) -> Result<Option<SocketAddr>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper(#[serde(deserialize_with = "SocketAddr::deserialize_from_str")] SocketAddr);

        let option = Option::deserialize(deserializer)?;
        Ok(option.map(|Helper(external)| external))
    }
}

impl NamedSocketAddr {
    pub fn map_inet<F: FnOnce(net::SocketAddr) -> net::SocketAddr>(
        self,
        mapper: F,
    ) -> NamedSocketAddr {
        match self {
            NamedSocketAddr::Inet(inet_socket_addr) => {
                NamedSocketAddr::Inet(mapper(inet_socket_addr))
            }
            #[cfg(unix)]
            NamedSocketAddr::Unix(path) => NamedSocketAddr::Unix(path),
        }
    }

    // On non unix systems, op is not used.
    #[cfg_attr(not(unix), allow(unused_variables))]
    pub fn map_unix<F: FnOnce(PathBuf) -> PathBuf>(self, mapper: F) -> NamedSocketAddr {
        match self {
            NamedSocketAddr::Inet(inet_socket_addr) => NamedSocketAddr::Inet(inet_socket_addr),
            #[cfg(unix)]
            NamedSocketAddr::Unix(path) => NamedSocketAddr::Unix(mapper(path)),
        }
    }

    pub fn to_socket_addr(self) -> SocketAddr {
        match self {
            NamedSocketAddr::Inet(inet_socket_addr) => SocketAddr::Inet(inet_socket_addr),
            #[cfg(unix)]
            NamedSocketAddr::Unix(path) => SocketAddr::Unix(UnixSocketAddr::Pathname(path)),
        }
    }

    #[cfg_attr(feature = "serde", allow(unused))]
    #[cfg(feature = "serde")]
    pub fn deserialize_from_str<'de, D>(deserializer: D) -> Result<NamedSocketAddr, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        FromStr::from_str(&string).map_err(Error::custom)
    }

    #[cfg_attr(feature = "serde", allow(unused))]
    #[cfg(feature = "serde")]
    pub fn deserialize_from_option_str<'de, D>(
        deserializer: D,
    ) -> Result<Option<NamedSocketAddr>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper(
            #[serde(deserialize_with = "NamedSocketAddr::deserialize_from_str")] NamedSocketAddr,
        );

        let option = Option::deserialize(deserializer)?;
        Ok(option.map(|Helper(external)| external))
    }
}

impl FromStr for SocketAddr {
    type Err = AddrParseError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        NamedSocketAddr::from_str(string).map(NamedSocketAddr::into)
    }
}

impl FromStr for NamedSocketAddr {
    type Err = AddrParseError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        #[cfg(unix)]
        if UnixSocketAddr::is_pathname(string) {
            return Ok(NamedSocketAddr::Unix(PathBuf::from_str(string).unwrap()));
        }

        Ok(NamedSocketAddr::Inet(net::SocketAddr::from_str(string)?))
    }
}

impl From<NamedSocketAddr> for SocketAddr {
    fn from(named_socket_addr: NamedSocketAddr) -> Self {
        named_socket_addr.to_socket_addr()
    }
}

impl TryFrom<SocketAddr> for NamedSocketAddr {
    type Error = io::Error;

    fn try_from(socket_addr: SocketAddr) -> Result<Self, Self::Error> {
        socket_addr.to_named_socket_addr()
    }
}

impl From<net::SocketAddr> for SocketAddr {
    fn from(inet_socket_addr: net::SocketAddr) -> SocketAddr {
        SocketAddr::Inet(inet_socket_addr)
    }
}

impl From<net::SocketAddr> for NamedSocketAddr {
    fn from(inet_socket_addr: net::SocketAddr) -> NamedSocketAddr {
        NamedSocketAddr::Inet(inet_socket_addr)
    }
}

#[cfg(unix)]
impl From<PathBuf> for SocketAddr {
    fn from(path: PathBuf) -> SocketAddr {
        SocketAddr::Unix(UnixSocketAddr::Pathname(path))
    }
}

#[cfg(unix)]
impl From<PathBuf> for NamedSocketAddr {
    fn from(path: PathBuf) -> NamedSocketAddr {
        NamedSocketAddr::Unix(path)
    }
}

#[cfg(unix)]
impl From<UnixSocketAddr> for SocketAddr {
    fn from(unix_socket_addr: UnixSocketAddr) -> SocketAddr {
        SocketAddr::Unix(unix_socket_addr)
    }
}

#[cfg(unix)]
impl From<unix::SocketAddr> for SocketAddr {
    fn from(unix_socket_addr: unix::SocketAddr) -> Self {
        SocketAddr::Unix(unix_socket_addr.into())
    }
}

#[cfg(unix)]
impl From<unix::SocketAddr> for UnixSocketAddr {
    fn from(unix_socket_addr: unix::SocketAddr) -> Self {
        match unix_socket_addr.as_pathname() {
            Some(path) => UnixSocketAddr::Pathname(path.to_path_buf()),
            None => UnixSocketAddr::AbstractOrUnnamed,
        }
    }
}

#[cfg(unix)]
impl Debug for UnixSocketAddr {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            UnixSocketAddr::AbstractOrUnnamed => write!(formatter, "(abstract or unnamed)"),
            UnixSocketAddr::Pathname(path) => write!(formatter, "{path:?} (pathname)"),
        }
    }
}

impl Display for SocketAddr {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            SocketAddr::Inet(inet_socket_addr) => write!(formatter, "{}", inet_socket_addr),
            #[cfg(unix)]
            SocketAddr::Unix(unix_socket_addr) => write!(formatter, "unix {:?}", unix_socket_addr),
        }
    }
}

impl Display for NamedSocketAddr {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            NamedSocketAddr::Inet(inet_socket_addr) => write!(formatter, "{}", inet_socket_addr),
            #[cfg(unix)]
            NamedSocketAddr::Unix(path) => write!(formatter, "unix {:?}", path),
        }
    }
}
