// Create a module to contain the Protocol Buffers generated code
use anyhow::{anyhow, Context, Result};
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::trace;
pub use v2::*;

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

pub mod v2 {
    use crate::fair_channel::FairGroup;
    pub use crate::protocol::str_enum;

    use super::{DefaultPort, Endpoint};
    use anyhow::{bail, Context, Result};
    use bytes::BytesMut;
    pub use prost::Message as ProstMessage;

    use crate::utils::get_version_number;
    use bytes::Bytes;
    use std::fmt::{self, Display, Formatter};
    use std::str::FromStr;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use tracing::debug;
    use urlencoding::encode;

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
                Protocol::Http
                | Protocol::Https
                | Protocol::Tcp
                | Protocol::Udp
                | Protocol::Rtsp => {
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
}

pub mod v1 {
    use super::v2;
    use crate::config::MaskedString;
    use crate::utils::get_version_number;
    use anyhow::{bail, Result};
    use clap::{self, ValueEnum};
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use std::fmt::{self, Display, Formatter};
    use std::str::FromStr;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use tracing::debug;

    #[derive(ValueEnum, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Protocol {
        #[default]
        #[serde(rename = "http")]
        Http,
        #[serde(rename = "https")]
        Https,
        #[serde(rename = "tcp")]
        Tcp,
        #[serde(rename = "udp")]
        Udp,
        #[serde(rename = "1c")]
        #[clap(name = "1c")]
        OneC,
        #[serde(rename = "minecraft")]
        #[clap(name = "minecraft")]
        Minecraft,
        #[serde(rename = "webdav")]
        #[clap(name = "webdav")]
        WebDav,
        #[serde(rename = "rtsp")]
        #[clap(name = "rtsp")]
        Rtsp,
    }

    impl From<Protocol> for v2::Protocol {
        fn from(p: Protocol) -> Self {
            match p {
                Protocol::Http => v2::Protocol::Http,
                Protocol::Https => v2::Protocol::Https,
                Protocol::Tcp => v2::Protocol::Tcp,
                Protocol::Udp => v2::Protocol::Udp,
                Protocol::OneC => v2::Protocol::OneC,
                Protocol::Minecraft => v2::Protocol::Minecraft,
                Protocol::WebDav => v2::Protocol::Webdav,
                Protocol::Rtsp => v2::Protocol::Rtsp,
            }
        }
    }

    impl From<v2::Protocol> for Protocol {
        fn from(p: v2::Protocol) -> Self {
            match p {
                v2::Protocol::Http => Protocol::Http,
                v2::Protocol::Https => Protocol::Https,
                v2::Protocol::Tcp => Protocol::Tcp,
                v2::Protocol::Udp => Protocol::Udp,
                v2::Protocol::OneC => Protocol::OneC,
                v2::Protocol::Minecraft => Protocol::Minecraft,
                v2::Protocol::Webdav => Protocol::WebDav,
                v2::Protocol::Rtsp => Protocol::Rtsp,
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
    pub enum Auth {
        #[default]
        NONE,
        BASIC,
        FORM,
    }

    impl From<Auth> for v2::Auth {
        fn from(a: Auth) -> Self {
            match a {
                Auth::NONE => v2::Auth::None,
                Auth::BASIC => v2::Auth::Basic,
                Auth::FORM => v2::Auth::Form,
            }
        }
    }

    impl From<v2::Auth> for Auth {
        fn from(a: v2::Auth) -> Self {
            match a {
                v2::Auth::None => Auth::NONE,
                v2::Auth::Basic => Auth::BASIC,
                v2::Auth::Form => Auth::FORM,
            }
        }
    }

    impl FromStr for Auth {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self> {
            match s {
                "none" => Ok(Auth::NONE),
                "basic" => Ok(Auth::BASIC),
                "form" => Ok(Auth::FORM),
                _ => bail!("Invalid auth: {}", s),
            }
        }
    }

    impl Display for Auth {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            match self {
                Auth::NONE => write!(f, "none"),
                Auth::BASIC => write!(f, "basic"),
                Auth::FORM => write!(f, "form"),
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
    pub enum Role {
        #[default]
        NONE,
        ADMIN,
        READER,
        WRITER,
    }

    impl From<Role> for v2::Role {
        fn from(r: Role) -> Self {
            match r {
                Role::NONE => v2::Role::Nobody,
                Role::ADMIN => v2::Role::Admin,
                Role::READER => v2::Role::Reader,
                Role::WRITER => v2::Role::Writer,
            }
        }
    }

    impl From<v2::Role> for Role {
        fn from(r: v2::Role) -> Self {
            match r {
                v2::Role::Nobody => Role::NONE,
                v2::Role::Admin => Role::ADMIN,
                v2::Role::Reader => Role::READER,
                v2::Role::Writer => Role::WRITER,
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
    pub struct ACL {
        pub user: String,
        pub role: Role,
    }

    impl From<ACL> for v2::Acl {
        fn from(acl: ACL) -> Self {
            v2::Acl {
                user: acl.user,
                role: v2::Role::from(acl.role) as i32,
            }
        }
    }

    impl From<v2::Acl> for ACL {
        fn from(acl: v2::Acl) -> Self {
            ACL {
                user: acl.user,
                role: v2::Role::try_from(acl.role)
                    .unwrap_or(v2::Role::Nobody)
                    .into(),
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
    pub struct ClientEndpoint {
        pub local_proto: Protocol,
        pub local_addr: String,
        pub local_port: u16,
        #[serde(default)]
        pub local_path: String,
        pub nodelay: Option<bool>,
        pub description: Option<String>,
        #[serde(default)]
        pub auth: Auth,
        #[serde(default)]
        pub acl: Vec<ACL>,
        #[serde(default)]
        pub username: String,
        #[serde(default)]
        pub password: MaskedString,
    }

    impl From<ClientEndpoint> for v2::ClientEndpoint {
        fn from(ce: ClientEndpoint) -> Self {
            v2::ClientEndpoint {
                local_proto: v2::Protocol::from(ce.local_proto) as i32,
                local_addr: ce.local_addr,
                local_port: ce.local_port as u32,
                local_path: ce.local_path,
                nodelay: ce.nodelay,
                description: ce.description,
                auth: v2::Auth::from(ce.auth) as i32,
                acl: ce.acl.into_iter().map(|a| a.into()).collect(),
                username: ce.username,
                password: ce.password.0,
                headers: Vec::new(),
                filter_rules: Vec::new(),
            }
        }
    }

    impl From<v2::ClientEndpoint> for ClientEndpoint {
        fn from(ce: v2::ClientEndpoint) -> Self {
            ClientEndpoint {
                local_proto: v2::Protocol::try_from(ce.local_proto)
                    .unwrap_or(v2::Protocol::Http)
                    .into(),
                local_addr: ce.local_addr,
                local_port: ce.local_port as u16,
                local_path: ce.local_path,
                nodelay: ce.nodelay,
                description: ce.description,
                auth: v2::Auth::try_from(ce.auth).unwrap_or(v2::Auth::None).into(),
                acl: ce.acl.into_iter().map(|a| a.into()).collect(),
                username: ce.username,
                password: MaskedString(ce.password),
            }
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
    pub struct ServerEndpoint {
        pub status: Option<String>,
        pub guid: String,
        pub remote_proto: Protocol,
        pub remote_addr: String,
        pub remote_port: u16,
        pub client: ClientEndpoint,
        #[serde(skip)]
        pub bind_addr: String,
        #[serde(skip)]
        pub id: Option<i64>,
    }

    impl From<ServerEndpoint> for v2::ServerEndpoint {
        fn from(se: ServerEndpoint) -> Self {
            v2::ServerEndpoint {
                id: se.id.unwrap_or_default(),
                bind_addr: se.bind_addr,
                status: se.status.clone(),
                default_status: se.status,
                guid: se.guid,
                remote_proto: v2::Protocol::from(se.remote_proto) as i32,
                remote_addr: se.remote_addr,
                remote_port: se.remote_port as u32,
                client: Some(se.client.into()),
                error: String::new(),
            }
        }
    }

    impl From<v2::ServerEndpoint> for ServerEndpoint {
        fn from(se: v2::ServerEndpoint) -> Self {
            ServerEndpoint {
                status: se.status,
                guid: se.guid,
                remote_proto: v2::Protocol::try_from(se.remote_proto)
                    .unwrap_or(v2::Protocol::Http)
                    .into(),
                remote_addr: se.remote_addr,
                remote_port: se.remote_port as u16,
                client: se.client.unwrap_or_default().into(),
                bind_addr: se.bind_addr,
                id: Some(se.id),
            }
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
    pub enum ErrorKind {
        AuthFailed,
        Fatal,
        HandshakeFailed,
        PermissionDenied,
        PublishFailed,
        ExecuteFailed,
    }

    impl From<ErrorKind> for v2::ErrorKind {
        fn from(ek: ErrorKind) -> Self {
            match ek {
                ErrorKind::AuthFailed => v2::ErrorKind::AuthFailed,
                ErrorKind::Fatal => v2::ErrorKind::Fatal,
                ErrorKind::HandshakeFailed => v2::ErrorKind::HandshakeFailed,
                ErrorKind::PermissionDenied => v2::ErrorKind::PermissionDenied,
                ErrorKind::PublishFailed => v2::ErrorKind::PublishFailed,
                ErrorKind::ExecuteFailed => v2::ErrorKind::ExecuteFailed,
            }
        }
    }

    impl From<v2::ErrorKind> for ErrorKind {
        fn from(ek: v2::ErrorKind) -> Self {
            match ek {
                v2::ErrorKind::AuthFailed => ErrorKind::AuthFailed,
                v2::ErrorKind::Fatal => ErrorKind::Fatal,
                v2::ErrorKind::HandshakeFailed => ErrorKind::HandshakeFailed,
                v2::ErrorKind::PermissionDenied => ErrorKind::PermissionDenied,
                v2::ErrorKind::PublishFailed => ErrorKind::PublishFailed,
                v2::ErrorKind::ExecuteFailed => ErrorKind::ExecuteFailed,
            }
        }
    }

    fn default_version() -> String {
        "1.0.x".to_string()
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub struct AgentInfo {
        pub agent_id: String,
        pub token: MaskedString,
        pub hostname: String,
        // Next fields should have default values
        #[serde(default = "default_version")]
        pub version: String,
        #[serde(default)]
        pub gui: bool,
        #[serde(default)]
        pub platform: String,
        #[serde(default)]
        pub hwid: String,
        #[serde(default)]
        pub server_host_and_port: String,
    }

    impl From<AgentInfo> for v2::AgentInfo {
        fn from(ai: AgentInfo) -> Self {
            v2::AgentInfo {
                agent_id: ai.agent_id,
                token: ai.token.0,
                hostname: ai.hostname,
                version: ai.version,
                gui: ai.gui,
                platform: ai.platform,
                hwid: ai.hwid,
                server_host_and_port: ai.server_host_and_port,
                email: String::new(),
                password: String::new(),
                transient: false,
                secondary: false,
            }
        }
    }

    impl From<v2::AgentInfo> for AgentInfo {
        fn from(ai: v2::AgentInfo) -> Self {
            AgentInfo {
                agent_id: ai.agent_id,
                token: MaskedString(ai.token),
                hostname: ai.hostname,
                version: if ai.version.is_empty() {
                    default_version()
                } else {
                    ai.version
                },
                gui: ai.gui,
                platform: ai.platform,
                hwid: ai.hwid,
                server_host_and_port: ai.server_host_and_port,
            }
        }
    }

    impl AgentInfo {
        pub fn is_support_upgrade(&self) -> bool {
            get_version_number(&self.version) >= get_version_number("1.1.0")
        }

        pub fn is_support_pong(&self) -> bool {
            get_version_number(&self.version) >= get_version_number("1.2.104")
        }

        pub fn is_support_redirect(&self) -> bool {
            get_version_number(&self.version) >= get_version_number("1.3.2")
        }
    }

    impl Display for AgentInfo {
        fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
            write!(f, "{}", self.hostname)
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub struct DataChannelInfo {
        pub agent_id: String,
        pub guid: String,
    }

    impl From<DataChannelInfo> for v2::DataChannelInfo {
        fn from(dci: DataChannelInfo) -> Self {
            v2::DataChannelInfo {
                agent_id: dci.agent_id,
                guid: dci.guid,
            }
        }
    }

    impl From<v2::DataChannelInfo> for DataChannelInfo {
        fn from(dci: v2::DataChannelInfo) -> Self {
            DataChannelInfo {
                agent_id: dci.agent_id,
                guid: dci.guid,
            }
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub struct UpgradeInfo {
        pub version: String,
        pub url: String,
    }

    impl From<UpgradeInfo> for v2::UpgradeInfo {
        fn from(ui: UpgradeInfo) -> Self {
            v2::UpgradeInfo {
                version: ui.version,
                url: ui.url,
            }
        }
    }

    impl From<v2::UpgradeInfo> for UpgradeInfo {
        fn from(ui: v2::UpgradeInfo) -> Self {
            UpgradeInfo {
                version: ui.version,
                url: ui.url,
            }
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub enum Message {
        AgentHello(AgentInfo),
        AgentAck,
        EndpointStart(ClientEndpoint),
        EndpointAck(ServerEndpoint),
        EndpointStop(String),
        DataChannelHello(DataChannelInfo),
        CreateDataChannel(ServerEndpoint),
        HeartBeat,
        StartForwardTcp,
        StartForwardUdp,
        Error(ErrorKind, String),
        UpgradeAvailable(UpgradeInfo),
        Redirect(String),
    }

    impl From<Message> for v2::Message {
        fn from(msg: Message) -> Self {
            use v2::message::Message as ProtoMessage;
            use v2::{ErrorInfo, ErrorKind};

            let message = match msg {
                Message::AgentHello(info) => ProtoMessage::AgentHello(info.into()),
                Message::AgentAck => ProtoMessage::AgentAck(v2::AgentAck {
                    token: String::new(),
                }),
                Message::EndpointStart(endpoint) => ProtoMessage::EndpointStart(endpoint.into()),
                Message::EndpointAck(endpoint) => ProtoMessage::EndpointAck(endpoint.into()),
                Message::EndpointStop(guid) => {
                    ProtoMessage::EndpointStop(v2::EndpointStop { guid })
                }
                Message::DataChannelHello(info) => ProtoMessage::DataChannelHello(info.into()),
                Message::CreateDataChannel(endpoint) => {
                    ProtoMessage::CreateDataChannel(endpoint.into())
                }
                Message::HeartBeat => ProtoMessage::HeartBeat(v2::HeartBeat {}),
                Message::StartForwardTcp => ProtoMessage::StartForwardTcp(v2::StartForwardTcp {}),
                Message::StartForwardUdp => ProtoMessage::StartForwardUdp(v2::StartForwardUdp {}),
                Message::Error(kind, msg) => ProtoMessage::Error(ErrorInfo {
                    kind: ErrorKind::from(kind) as i32,
                    message: msg,
                }),
                Message::UpgradeAvailable(info) => ProtoMessage::UpgradeAvailable(info.into()),
                Message::Redirect(host_and_port) => {
                    ProtoMessage::Redirect(v2::Redirect { host_and_port })
                }
            };

            v2::Message {
                message: Some(message),
            }
        }
    }

    impl From<v2::Message> for Message {
        fn from(msg: v2::Message) -> Self {
            use v2::message::Message as ProtoMessage;

            match msg.message.unwrap() {
                ProtoMessage::AgentHello(info) => Message::AgentHello(info.into()),
                ProtoMessage::AgentAck(_) => Message::AgentAck,
                ProtoMessage::EndpointStart(endpoint) => Message::EndpointStart(endpoint.into()),
                ProtoMessage::EndpointAck(endpoint) => Message::EndpointAck(endpoint.into()),
                ProtoMessage::EndpointStop(ep) => Message::EndpointStop(ep.guid),
                ProtoMessage::DataChannelHello(info) => Message::DataChannelHello(info.into()),
                ProtoMessage::CreateDataChannel(endpoint) => {
                    Message::CreateDataChannel(endpoint.into())
                }
                ProtoMessage::HeartBeat(_) => Message::HeartBeat,
                ProtoMessage::StartForwardTcp(_) => Message::StartForwardTcp,
                ProtoMessage::StartForwardUdp(_) => Message::StartForwardUdp,
                ProtoMessage::Error(error) => Message::Error(
                    v2::ErrorKind::try_from(error.kind)
                        .unwrap_or(v2::ErrorKind::Fatal)
                        .into(),
                    error.message,
                ),
                ProtoMessage::UpgradeAvailable(info) => Message::UpgradeAvailable(info.into()),
                ProtoMessage::Redirect(r) => Message::Redirect(r.host_and_port),
                // Other not supported V2 protocol messages
                _ => Message::HeartBeat,
            }
        }
    }

    pub async fn read_message<T: AsyncRead + Unpin, M: DeserializeOwned>(
        conn: &mut T,
    ) -> Result<M> {
        let mut buf = [0u8; std::mem::size_of::<u64>()];
        conn.read_exact(&mut buf).await?;
        let len = u64::from_le_bytes(buf) as usize;
        if !(1..=1024 * 1024 * 10).contains(&len) {
            bail!("Invalid message length: {}", len);
        }
        let mut buf = vec![0u8; len];
        conn.read_exact(&mut buf).await?;
        Ok(serde_json::from_slice::<M>(&buf)?)
    }

    pub async fn write_message<T: AsyncWrite + Unpin, M: Serialize>(
        conn: &mut T,
        msg: &M,
    ) -> Result<()> {
        let json_msg = serde_json::to_string(msg)?;
        debug!("Send: {}", json_msg);
        let mut buf = (json_msg.len() as u64).to_le_bytes().to_vec();
        buf.append(&mut json_msg.as_bytes().to_vec());
        conn.write_all(&buf).await?;
        conn.flush().await?;
        Ok(())
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
