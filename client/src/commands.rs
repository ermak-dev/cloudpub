use anyhow::{bail, Context, Result};
use clap::builder::TypedValueParser;
use clap::{Args, Subcommand};
use common::config::MaskedString;
use common::protocol::{
    Acl, Auth, ClientEndpoint, DefaultPort, FilterAction, FilterRule, Header, Protocol, Role,
};
use serde::{Deserialize, Serialize};
use std::net::ToSocketAddrs;
use std::str::FromStr;

const ROLE_SEP: &str = ":";

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    #[clap(about = "Set value in the config")]
    Set(SetArgs),
    #[clap(about = "Get value from the config")]
    Get(GetArgs),
    #[clap(about = "Show all config options and their values")]
    Options,
    #[clap(about = "Run all registered services")]
    Run,
    #[clap(about = "Start publication")]
    Start(GuidArgs),
    #[clap(about = "Stop publication")]
    Stop(GuidArgs),
    #[clap(about = "Break current operation (used internally)", hide = true)]
    Break,
    #[clap(about = "Register service on server")]
    Register(PublishArgs),
    #[clap(about = "Register service and run it")]
    Publish(PublishArgs),
    #[clap(about = "Unregister service")]
    Unpublish(GuidArgs),
    #[clap(about = "List all registered services")]
    Ls,
    #[clap(about = "Clean all registered services")]
    Clean,
    #[clap(about = "Purge cache")]
    Purge,
    #[clap(about = "Ping server and measure roundtrip time")]
    Ping(PingArg),
    #[clap(about = "Login to server and save token")]
    Login(LoginArgs),
    #[clap(about = "Logout and clear saved token")]
    Logout,
    #[clap(about = "Upgrade client to the latest version")]
    Upgrade,
    #[clap(about = "Manage system service")]
    Service {
        #[clap(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ServiceAction {
    #[clap(about = "Install as a system service")]
    Install,

    #[clap(about = "Uninstall the system service")]
    Uninstall,

    #[clap(about = "Start the service")]
    Start,

    #[clap(about = "Stop the service")]
    Stop,

    #[clap(about = "Get service status")]
    Status,
}

#[derive(Args, Debug, Serialize, Deserialize, Clone)]
pub struct SetArgs {
    pub key: String,
    pub value: String,
}

#[derive(Args, Debug, Serialize, Deserialize, Clone)]
pub struct PingArg {
    #[clap(short = 'N', long = "num", help = "Number of parallel pings")]
    pub num: Option<i32>,
    #[clap(short = 'B', long = "bare", help = "Output time in µs")]
    pub bare: bool,
}

#[derive(Args, Debug, Serialize, Deserialize, Clone)]
pub struct GetArgs {
    pub key: String,
}

#[derive(Args, Debug, Clone)]
pub struct PublishArgs {
    #[clap(help = "Protocol to use")]
    pub protocol: Protocol,
    #[clap(help = "URL, socket address, port or file path")]
    pub address: String,
    #[clap(short = 'U', long = "username", help = "Username")]
    pub username: Option<String>,
    #[clap(short = 'P', long = "password", help = "Password")]
    pub password: Option<MaskedString>,
    #[clap(short, long, help = "Optional name of the service to publish")]
    pub name: Option<String>,
    #[clap(short, long, help = "Authentification type")]
    pub auth: Option<Auth>,
    #[clap(short='A', long="acl", help = "Access list", value_parser = AclParser)]
    pub acl: Vec<Acl>,
    #[clap(short='H', long="header", help = "HTTP headers", value_parser = HeaderParser)]
    pub headers: Vec<Header>,
    #[clap(short='R', long="rule", help = "Filter rules", value_parser = RuleParser)]
    pub rules: Vec<FilterRule>,
}

#[derive(Args, Debug, Serialize, Deserialize, Clone)]
pub struct GuidArgs {
    pub guid: String,
}

#[derive(Args, Debug, Serialize, Deserialize, Clone)]
pub struct LoginArgs {
    #[clap(
        help = "Email address (if not provided, will prompt)",
        required = false
    )]
    pub email: Option<String>,
    #[clap(
        help = "Password (if not provided, will prompt or use CLO_PASSWORD env var)",
        env = "CLO_PASSWORD",
        hide_env_values = true,
        required = false
    )]
    pub password: Option<String>,
}

impl PublishArgs {
    pub fn parse(&self) -> Result<ClientEndpoint> {
        let auth = self.auth.unwrap_or(if self.protocol == Protocol::Webdav {
            Auth::Basic
        } else {
            Auth::None
        });
        if self.address.contains("://") {
            let url = url::Url::parse(&self.address).context(crate::t!("invalid-url"))?;
            let local_proto =
                Protocol::from_str(url.scheme()).context(crate::t!("invalid-protocol"))?;
            let local_addr = url.host_str().unwrap().to_string();
            let local_port = url
                .port()
                .or_else(|| self.protocol.default_port())
                .context(crate::t!("port-required"))?;
            let mut local_path = url.path().to_string();
            if url.query().is_some() {
                local_path.push('?');
                local_path.push_str(url.query().unwrap());
            }
            let mut username = String::new();
            if !url.username().is_empty() {
                username = url.username().to_string();
            }
            let mut password = MaskedString(String::new());
            if let Some(pass) = url.password() {
                password = MaskedString(pass.to_string());
            }
            let mut filter_rules = self.rules.clone();
            for (index, rule) in filter_rules.iter_mut().enumerate() {
                rule.order = index as i32;
            }

            Ok(ClientEndpoint {
                description: self.name.clone(),
                local_proto: local_proto.into(),
                local_addr,
                local_port: local_port as u32,
                local_path,
                nodelay: Some(true),
                auth: auth.into(),
                acl: self.acl.clone(),
                headers: self.headers.clone(),
                filter_rules,
                username,
                password: password.0,
            })
        } else {
            let (local_addr, local_port, local_path) = match self.protocol {
                Protocol::OneC | Protocol::Minecraft | Protocol::Webdav => {
                    (self.address.clone(), 0, String::new())
                }

                Protocol::Http
                | Protocol::Https
                | Protocol::Tcp
                | Protocol::Udp
                | Protocol::Rtsp => {
                    if let Ok(port) = self.address.parse::<u16>() {
                        ("localhost".to_string(), port, String::new())
                    } else {
                        let mut address = self.address.split('/').next().unwrap().to_string();
                        let path = self.address[address.len()..].to_string();

                        if let Some(port) = self.protocol.default_port() {
                            if !address.contains(":") {
                                address.push(':');
                                address.push_str(port.to_string().as_str());
                            }
                        }

                        match address.to_socket_addrs() {
                            Ok(mut addrs) => {
                                if addrs.next().is_some() {
                                    // Split original address to addr and port
                                    let parts = address.split(':').collect::<Vec<&str>>();
                                    (parts[0].to_string(), parts[1].parse::<u16>().unwrap(), path)
                                } else {
                                    bail!(crate::t!("invalid-address", "address" => address));
                                }
                            }
                            Err(err) => bail!(
                                crate::t!("invalid-address-error", "error" => err.to_string(), "address" => address)
                            ),
                        }
                    }
                }
            };

            let mut filter_rules = self.rules.clone();
            for (index, rule) in filter_rules.iter_mut().enumerate() {
                rule.order = index as i32;
            }

            Ok(ClientEndpoint {
                description: self.name.clone(),
                local_proto: self.protocol.into(),
                local_addr,
                local_port: local_port as u32,
                local_path,
                nodelay: Some(true),
                auth: auth.into(),
                acl: self.acl.clone(),
                headers: self.headers.clone(),
                filter_rules,
                username: self.username.clone().unwrap_or("".to_string()),
                password: self
                    .password
                    .clone()
                    .unwrap_or(MaskedString("".to_string()))
                    .to_string(),
            })
        }
    }
}

impl PartialEq for PublishArgs {
    fn eq(&self, other: &Self) -> bool {
        self.protocol == other.protocol && self.address == other.address
    }
}

const HEADER_SEP: &str = ":";

#[derive(Debug, Clone)]
struct HeaderParser;

impl TypedValueParser for HeaderParser {
    type Value = Header;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let value = value.to_string_lossy();
        let parts: Vec<&str> = value.splitn(2, HEADER_SEP).collect();
        if parts.len() != 2 {
            return Err(clap::Error::raw(
                clap::error::ErrorKind::ValueValidation,
                format!("Invalid Header format (should be 'name:value'): {}", value),
            ));
        }
        Ok(Header {
            name: parts[0].trim().to_string(),
            value: parts[1].trim().to_string(),
        })
    }
}

#[derive(Debug, Clone)]
struct AclParser;

impl TypedValueParser for AclParser {
    type Value = Acl;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let value = value.to_string_lossy();
        let parts: Vec<&str> = value.split(ROLE_SEP).collect();
        if parts.len() != 2 {
            return Err(clap::Error::raw(
                clap::error::ErrorKind::ValueValidation,
                format!("Invalid Acl: {}", value),
            ));
        }
        let role = Role::from_str(parts[1]).map_err(|_err| {
            clap::Error::raw(
                clap::error::ErrorKind::ValueValidation,
                format!("Invalid role: {}", parts[1]),
            )
        })?;
        Ok(Acl {
            user: parts[0].to_string(),
            role: role.into(),
        })
    }
}

#[derive(Debug, Clone)]
struct RuleParser;

impl TypedValueParser for RuleParser {
    type Value = FilterRule;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let value = value.to_string_lossy();
        let parts: Vec<&str> = value.splitn(3, ":").collect();
        if parts.is_empty() {
            return Err(clap::Error::raw(
                clap::error::ErrorKind::ValueValidation,
                format!(
                    "Invalid rule format (should be 'action_type[:action_value][:data]'): {}",
                    value
                ),
            ));
        }

        let action_type = FilterAction::from_str(parts[0]).map_err(|_| {
            clap::Error::raw(
                clap::error::ErrorKind::ValueValidation,
                format!("Invalid action type: {}", parts[0]),
            )
        })?;

        let (action_value, data) = if action_type == FilterAction::FilterRedirect {
            // For FILTER_REDIRECT, expect action_value and optional data
            if parts.len() < 2 {
                return Err(clap::Error::raw(
                    clap::error::ErrorKind::ValueValidation,
                    format!("FILTER_REDIRECT requires action_value: {}", value),
                ));
            }
            let action_value = if parts[1].is_empty() {
                None
            } else {
                Some(parts[1].to_string())
            };
            let data = parts.get(2).unwrap_or(&"").to_string();
            (action_value, data)
        } else {
            // For other actions, no action_value, but data can be in parts[1]
            let data = parts.get(1).unwrap_or(&"").to_string();
            (None, data)
        };

        Ok(FilterRule {
            order: 0, // Will be set based on position in the arguments list
            action_value,
            action_type: action_type.into(),
            data,
        })
    }
}
