// Create a module to contain the Protocol Buffers generated code
use anyhow::{anyhow, bail, Context, Result};
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::{debug, trace};
use urlencoding::encode;

use crate::fair_channel::FairGroup;
use crate::utils::get_version_number;
pub use prost::Message as ProstMessage;

pub trait Endpoint {
    fn credentials(&self) -> String;
    fn as_url(&self) -> String;
}

pub trait DefaultPort {
    fn default_port(&self) -> Option<u16>;
}

pub fn parse_enum<E: FromStr + Into<i32>>(name: &str) -> Result<i32> {
    let proto = E::from_str(name).map_err(|_| anyhow!("Invalid enum: {}", name))?;
    Ok(proto.into())
}

pub fn str_enum<E: TryFrom<i32> + ToString>(e: i32) -> String {
    e.try_into()
        .map(|e: E| e.to_string())
        .unwrap_or("unknown".to_string())
}

include!(concat!(env!("OUT_DIR"), "/protocol.rs"));

pub struct Data {
    pub data: Bytes,
    pub socket_addr: Option<std::net::SocketAddr>,
}

impl Display for Protocol {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Protocol::Http => write!(f, "http"),
            Protocol::Https => write!(f, "https"),
            Protocol::Tcp => write!(f, "tcp"),
            Protocol::Udp => write!(f, "udp"),
            Protocol::OneC => write!(f, "1c"),
            Protocol::Minecraft => write!(f, "minecraft"),
            Protocol::Webdav => write!(f, "webdav"),
            Protocol::Rtsp => write!(f, "rtsp"),
        }
    }
}

impl FromStr for Protocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "http" => Ok(Protocol::Http),
            "https" => Ok(Protocol::Https),
            "tcp" => Ok(Protocol::Tcp),
            "udp" => Ok(Protocol::Udp),
            "1c" => Ok(Protocol::OneC),
            "minecraft" => Ok(Protocol::Minecraft),
            "webdav" => Ok(Protocol::Webdav),
            "rtsp" => Ok(Protocol::Rtsp),
            _ => bail!("Invalid protocol: {}", s),
        }
    }
}

impl DefaultPort for Protocol {
    fn default_port(&self) -> Option<u16> {
        match self {
            Protocol::Http => Some(80),
            Protocol::Https => Some(443),
            Protocol::Tcp => None,
            Protocol::Udp => None,
            Protocol::OneC => None,
            Protocol::Minecraft => Some(25565),
            Protocol::Webdav => None,
            Protocol::Rtsp => Some(554),
        }
    }
}

impl FromStr for Role {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "none" => Ok(Role::Nobody),
            "admin" => Ok(Role::Admin),
            "reader" => Ok(Role::Reader),
            "writer" => Ok(Role::Writer),
            _ => bail!("Invalid access: {}", s),
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Role::Nobody => write!(f, "none"),
            Role::Admin => write!(f, "admin"),
            Role::Reader => write!(f, "reader"),
            Role::Writer => write!(f, "writer"),
        }
    }
}

impl FromStr for Auth {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "none" => Ok(Auth::None),
            "basic" => Ok(Auth::Basic),
            "form" => Ok(Auth::Form),
            _ => bail!("Invalid auth: {}", s),
        }
    }
}

impl Display for Auth {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Auth::None => write!(f, "none"),
            Auth::Basic => write!(f, "basic"),
            Auth::Form => write!(f, "form"),
        }
    }
}

impl FromStr for FilterAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "allow" => Ok(FilterAction::FilterAllow),
            "deny" => Ok(FilterAction::FilterDeny),
            "redirect" => Ok(FilterAction::FilterRedirect),
            "log" => Ok(FilterAction::FilterLog),
            _ => bail!("Invalid filter action: {}", s),
        }
    }
}

impl PartialEq for ClientEndpoint {
    fn eq(&self, other: &Self) -> bool {
        self.local_proto == other.local_proto
            && self.local_addr == other.local_addr
            && self.local_port == other.local_port
    }
}

impl Display for ClientEndpoint {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        if let Some(name) = self.description.as_ref() {
            write!(f, "[{}] ", name)?;
        }
        write!(f, "{}", self.as_url())
    }
}

impl Endpoint for ClientEndpoint {
    fn credentials(&self) -> String {
        let mut s = String::new();
        if !self.username.is_empty() {
            s.push_str(&encode(&self.username));
        }
        if !self.password.is_empty() {
            s.push(':');
            s.push_str(&encode(&self.password));
        }
        if !s.is_empty() {
            s.push('@');
        }
        s
    }

    fn as_url(&self) -> String {
        match self.local_proto.try_into().unwrap() {
            Protocol::OneC | Protocol::Minecraft | Protocol::Webdav => {
                let credentials = self.credentials();
                format!(
                    "{}://{}{}",
                    str_enum::<Protocol>(self.local_proto),
                    credentials,
                    &self.local_addr
                )
            }
            Protocol::Http | Protocol::Https | Protocol::Tcp | Protocol::Udp | Protocol::Rtsp => {
                let credentials = self.credentials();
                format!(
                    "{}://{}{}:{}{}",
                    str_enum::<Protocol>(self.local_proto),
                    credentials,
                    self.local_addr,
                    self.local_port,
                    self.local_path
                )
            }
        }
    }
}

impl Endpoint for ServerEndpoint {
    fn credentials(&self) -> String {
        String::new()
    }

    fn as_url(&self) -> String {
        format!(
            "{}://{}:{}",
            str_enum::<Protocol>(self.remote_proto),
            self.remote_addr,
            self.remote_port,
        )
    }
}

impl Display for ServerEndpoint {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let client = self.client.as_ref().unwrap();
        if self.error.is_empty() {
            write!(
                f,
                "{} -> {}://{}{}:{}{}",
                client,
                Protocol::try_from(self.remote_proto).unwrap(),
                client.credentials(),
                self.remote_addr,
                self.remote_port,
                client.local_path
            )
        } else {
            write!(f, "{} -> {}", client, self.error)
        }
    }
}

impl AgentInfo {
    pub fn is_support_server_control(&self) -> bool {
        get_version_number(&self.version) >= get_version_number("2.1.1")
    }

    pub fn is_support_backpressure(&self) -> bool {
        get_version_number(&self.version) >= get_version_number("2.2.0")
    }

    pub fn get_unique_id(&self) -> String {
        if self.hwid.is_empty() {
            self.agent_id.clone()
        } else {
            self.hwid.clone()
        }
    }
}

impl PartialEq for ServerEndpoint {
    fn eq(&self, other: &Self) -> bool {
        self.client == other.client
    }
}

// New Protocol Buffers message reading and writing functions
pub async fn read_message<T: AsyncRead + Unpin>(conn: &mut T) -> Result<message::Message> {
    let mut buf = [0u8; std::mem::size_of::<u32>()];
    conn.read_exact(&mut buf).await?;
    let len = u32::from_le_bytes(buf) as usize;
    if !(1..=1024 * 1024 * 10).contains(&len) {
        bail!("Invalid message length: {}", len);
    }
    let mut buf = vec![0u8; len];
    conn.read_exact(&mut buf).await?;

    let proto_msg =
        Message::decode(buf.as_slice()).context("Failed to decode Protocol Buffers message")?;

    Ok(proto_msg.message.unwrap())
}

pub async fn write_message<T: AsyncWrite + Unpin>(
    conn: &mut T,
    msg: &message::Message,
) -> Result<()> {
    let proto_msg = Message {
        message: Some(msg.clone()),
    };

    debug!("Sending proto message: {:?}", msg);

    let mut buf = BytesMut::new();
    proto_msg
        .encode(&mut buf)
        .context("Failed to encode Protocol Buffers message")?;

    let len = buf.len() as u32;
    let len_bytes = len.to_le_bytes();

    conn.write_all(&len_bytes).await?;
    conn.write_all(&buf).await?;
    conn.flush().await?;
    Ok(())
}

impl FairGroup for message::Message {
    fn group_id(&self) -> Option<u32> {
        match self {
            message::Message::DataChannelData(lhs) => Some(lhs.channel_id),
            message::Message::DataChannelDataUdp(lhs) => Some(lhs.channel_id),
            message::Message::DataChannelEof(lhs) => Some(lhs.channel_id),
            message::Message::DataChannelAck(lhs) => Some(lhs.channel_id),
            _ => None,
        }
    }

    fn get_size(&self) -> Option<usize> {
        match self {
            message::Message::DataChannelData(data) => Some(data.data.len()),
            message::Message::DataChannelDataUdp(data) => Some(data.data.len()),
            _ => None, // Control messages have no size
        }
    }
}

pub type UdpPacketLen = u16; // `u16` should be enough for any practical UDP traffic on the Internet
                             //
#[derive(Deserialize, Serialize, Debug)]
pub struct UdpHeader {
    from: std::net::SocketAddr,
    len: UdpPacketLen,
}

#[derive(Debug)]
pub struct UdpTraffic {
    pub from: std::net::SocketAddr,
    pub data: Bytes,
}

impl UdpTraffic {
    pub async fn write<T: AsyncWrite + Unpin>(&self, writer: &mut T) -> Result<()> {
        let hdr = UdpHeader {
            from: self.from,
            len: self.data.len() as UdpPacketLen,
        };

        let v = bincode::serde::encode_to_vec(&hdr, bincode::config::legacy()).unwrap();

        trace!("Write {:?} of length {}", hdr, v.len());
        writer.write_u8(v.len() as u8).await?;
        writer.write_all(&v).await?;

        writer.write_all(&self.data).await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn write_slice<T: AsyncWrite + Unpin>(
        writer: &mut T,
        from: std::net::SocketAddr,
        data: &[u8],
    ) -> Result<()> {
        let hdr = UdpHeader {
            from,
            len: data.len() as UdpPacketLen,
        };

        let v = bincode::serde::encode_to_vec(&hdr, bincode::config::legacy()).unwrap();

        trace!("Write {:?} of length {}", hdr, v.len());
        writer.write_u8(v.len() as u8).await?;
        writer.write_all(&v).await?;

        writer.write_all(data).await?;

        Ok(())
    }

    pub async fn read<T: AsyncRead + Unpin>(reader: &mut T, hdr_len: u8) -> Result<UdpTraffic> {
        let mut buf = vec![0; hdr_len as usize];
        reader
            .read_exact(&mut buf)
            .await
            .with_context(|| "Failed to read udp header")?;

        let hdr: UdpHeader = bincode::serde::decode_from_slice(&buf, bincode::config::legacy())
            .with_context(|| "Failed to deserialize UdpHeader")?
            .0;

        trace!("hdr {:?}", hdr);

        let mut data = BytesMut::new();
        data.resize(hdr.len as usize, 0);
        reader.read_exact(&mut data).await?;

        Ok(UdpTraffic {
            from: hdr.from,
            data: data.freeze(),
        })
    }
}
