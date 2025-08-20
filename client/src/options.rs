use crate::config::{ClientConfig, MaskedString, Platform};
use anyhow::{bail, Context, Result};
use std::str::FromStr;

macro_rules! config_options {
    ($(
        $variant:ident => $key:literal,
        get: $getter:expr,
        set: $setter:expr
    ),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ConfigOption {
            $($variant,)*
        }

        impl ConfigOption {
            pub fn all() -> &'static [Self] {
                &[$(Self::$variant,)*]
            }

            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $key,)*
                }
            }


            pub fn get_value(&self, config: &ClientConfig) -> Result<String> {
                match self {
                    $(Self::$variant => $getter(config),)*
                }
            }

            pub fn set_value(&self, config: &mut ClientConfig, value: &str) -> Result<()> {
                match self {
                    $(Self::$variant => $setter(config, value)?,)*
                }
                Ok(())
            }
        }

        impl FromStr for ConfigOption {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($key => Ok(Self::$variant),)*
                    _ => bail!("Unknown config option: {}", s),
                }
            }
        }
    };
}

config_options! {
    Server => "server",
    get: |config: &ClientConfig| Ok(config.server.to_string()),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.server = value.parse().context("Invalid server URL")?;
        Ok(())
    },

    Token => "token",
    get: |config: &ClientConfig| Ok(config.token.as_ref().map_or("".to_string(), |t| t.to_string())),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.token = Some(MaskedString::from(value));
        Ok(())
    },

    Hwid => "hwid",
    get: |config: &ClientConfig| Ok(config.get_hwid()),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.hwid = Some(value.to_string());
        Ok(())
    },

    HeartbeatTimeout => "heartbeat_timeout",
    get: |config: &ClientConfig| Ok(config.heartbeat_timeout.to_string()),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.heartbeat_timeout = value.parse().context("Invalid heartbeat_timeout")?;
        Ok(())
    },

    OneCHome => "1c_home",
    get: |config: &ClientConfig| Ok(config.one_c_home.clone().unwrap_or_default()),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.one_c_home = if value.is_empty() { None } else { Some(value.to_string()) };
        Ok(())
    },

    OneCPlatform => "1c_platform",
    get: |config: &ClientConfig| Ok(config.one_c_platform.as_ref().map(|p| p.to_string()).unwrap_or_default()),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.one_c_platform = if value.is_empty() {
            None
        } else {
            Some(value.parse::<Platform>().context("Invalid platform")?)
        };
        Ok(())
    },

    OneCPublishDir => "1c_publish_dir",
    get: |config: &ClientConfig| Ok(config.one_c_publish_dir.clone().unwrap_or_default()),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.one_c_publish_dir = if value.is_empty() { None } else { Some(value.to_string()) };
        Ok(())
    },

    MinecraftServer => "minecraft_server",
    get: |config: &ClientConfig| Ok(config.minecraft_server.clone().unwrap_or_default()),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.minecraft_server = if value.is_empty() { None } else { Some(value.to_string()) };
        Ok(())
    },

    MinecraftJavaOpts => "minecraft_java_opts",
    get: |config: &ClientConfig| Ok(config.minecraft_java_opts.clone().unwrap_or_default()),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.minecraft_java_opts = if value.is_empty() { None } else { Some(value.to_string()) };
        Ok(())
    },

    MinimizeToTrayOnClose => "minimize_to_tray_on_close",
    get: |config: &ClientConfig| Ok(config.minimize_to_tray_on_close.map_or("false".to_string(), |v| v.to_string())),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.minimize_to_tray_on_close = Some(value.parse().context("Invalid boolean value")?);
        Ok(())
    },

    MinimizeToTrayOnStart => "minimize_to_tray_on_start",
    get: |config: &ClientConfig| Ok(config.minimize_to_tray_on_start.map_or("false".to_string(), |v| v.to_string())),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        config.minimize_to_tray_on_start = Some(value.parse().context("Invalid boolean value")?);
        Ok(())
    },

    UnsafeTls => "unsafe_tls",
    get: |config: &ClientConfig| Ok(config.transport.tls.as_ref().map_or("".to_string(), |tls| {
        tls.danger_ignore_certificate_verification.map_or("".to_string(), |v| v.to_string())
    })),
    set: |config: &mut ClientConfig, value: &str| -> Result<(), anyhow::Error> {
        let allow = value.parse().context("Invalid boolean value")?;
        if config.transport.tls.is_none() {
            config.transport.tls = Some(Default::default());
        }
        if let Some(tls) = config.transport.tls.as_mut() {
            tls.danger_ignore_certificate_verification = Some(allow);
        }
        Ok(())
    },
}

impl ClientConfig {
    pub fn get_option(&self, option: ConfigOption) -> Result<String> {
        option.get_value(self)
    }

    pub fn set_option(&mut self, option: ConfigOption, value: &str) -> Result<()> {
        option.set_value(self, value)?;
        self.save()
    }

    pub fn get_all_config_options(&self) -> Vec<(String, String)> {
        ConfigOption::all()
            .iter()
            .filter_map(|&option| {
                option
                    .get_value(self)
                    .ok()
                    .map(|value| (option.as_str().to_string(), value))
            })
            .collect()
    }
}
