use anyhow::{bail, Context, Result};
use common::constants::DEFAULT_HEARTBEAT_TIMEOUT_SECS;
use common::DOMAIN;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

use crate::options::ConfigOption;
pub use common::config::{MaskedString, TransportConfig};
use lazy_static::lazy_static;
use machineid_rs::{Encryption, HWIDComponent, IdBuilder};
use std::collections::HashMap;
use std::fs::{self, create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;
use tracing::debug;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Platform {
    #[default]
    X64,
    X32,
}

#[derive(Clone)]
pub struct EnvConfig {
    pub home_1c: PathBuf,
    #[cfg(target_os = "windows")]
    pub redist: String,
    pub httpd: String,
    pub httpd_dir: String,
}

impl Default for EnvConfig {
    fn default() -> Self {
        #[cfg(target_pointer_width = "64")]
        return ENV_CONFIG.get(&Platform::X64).unwrap().clone();
        #[cfg(target_pointer_width = "32")]
        return ENV_CONFIG.get(&Platform::X32).unwrap().clone();
    }
}

lazy_static! {
    pub static ref ENV_CONFIG: HashMap<Platform, EnvConfig> = {
        let mut m = HashMap::new();
        #[cfg(target_os = "windows")]
        m.insert(
            Platform::X64,
            EnvConfig {
                home_1c: PathBuf::from("C:\\Program Files\\1cv8\\"),
                redist: "vc_redist.x64.exe".to_string(),
                httpd: "httpd-2.4.61-240703-win64-VS17.zip".to_string(),
                httpd_dir: "httpd-x64".to_string(),
            },
        );
        #[cfg(target_os = "windows")]
        m.insert(
            Platform::X32,
            EnvConfig {
                home_1c: PathBuf::from("C:\\Program Files (x86)\\1cv8"),
                redist: "vc_redist.x86.exe".to_string(),
                httpd: "httpd-2.4.61-240703-win32-vs17.zip".to_string(),
                httpd_dir: "httpd-x32".to_string(),
            },
        );
        #[cfg(target_os = "linux")]
        m.insert(
            Platform::X64,
            EnvConfig {
                home_1c: PathBuf::from("/opt/1C"),
                httpd: "httpd-2.4.62-linux.zip".to_string(),
                httpd_dir: "httpd-x64".to_string(),
            },
        );
        #[cfg(target_os = "macos")]
        m.insert(
            Platform::X64,
            EnvConfig {
                home_1c: PathBuf::from("/Applications/1C"),
                httpd: "httpd-2.4.62-macos.zip".to_string(),
                httpd_dir: "httpd-x64".to_string(),
            },
        );
        m
    };
}

impl Display for Platform {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Platform::X64 => write!(f, "x64"),
            Platform::X32 => write!(f, "x32"),
        }
    }
}

impl std::str::FromStr for Platform {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "x64" => Ok(Platform::X64),
            "x32" => Ok(Platform::X32),
            _ => bail!("Invalid platform: {}", s),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct ClientOpts {
    pub gui: bool,
    pub credentials: Option<(String, String)>,
    pub transient: bool,
    pub secondary: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct ClientConfig {
    // This fields are not persistent
    #[serde(skip)]
    config_path: PathBuf,
    #[serde(skip)]
    pub readonly: bool,
    pub agent_id: String,
    pub hwid: Option<String>,
    pub server: Url,
    pub token: Option<MaskedString>,
    pub heartbeat_timeout: u64,
    pub one_c_home: Option<String>,
    pub one_c_platform: Option<Platform>,
    pub one_c_publish_dir: Option<String>,
    pub minecraft_server: Option<String>,
    pub minecraft_java_opts: Option<String>,
    pub minimize_to_tray_on_close: Option<bool>,
    pub minimize_to_tray_on_start: Option<bool>,
    pub transport: TransportConfig,
}

impl ClientConfig {
    pub fn get_config_path(&self) -> &PathBuf {
        &self.config_path
    }

    pub fn from_file(path: &PathBuf, readonly: bool) -> Result<Self> {
        if !path.exists() {
            let default_config = Self::default();
            debug!("Creating default config at {:?}", default_config);
            let s = toml::to_string_pretty(&default_config)
                .context("Failed to serialize the default config")?;
            fs::write(path, s).context("Failed to write the default config")?;
        }

        let s: String = fs::read_to_string(path)
            .with_context(|| format!("Failed to read the config {:?}", path))?;
        let mut cfg: Self = toml::from_str(&s).with_context(|| {
            "Configuration is invalid. Please refer to the configuration specification."
        })?;

        cfg.config_path = path.clone();
        cfg.readonly = readonly;
        Ok(cfg)
    }

    pub fn get_config_dir(user_dir: bool) -> Result<PathBuf> {
        let dir = if user_dir {
            let mut dir = dirs::config_dir().context("Can't get config_dir")?;
            dir.push("cloudpub");
            dir
        } else if cfg!(target_family = "unix") {
            PathBuf::from("./etc/cloudpub")
        } else {
            PathBuf::from("C:\\Windows\\system32\\config\\systemprofile\\AppData\\Local\\cloudpub")
        };
        if !dir.exists() {
            create_dir_all(&dir).context("Can't create config dir")?;
        }
        debug!("Config dir: {:?}", dir);
        Ok(dir)
    }

    pub fn save(&self) -> Result<()> {
        if self.readonly {
            debug!("Skipping saving the config in readonly mode");
        } else {
            let s = toml::to_string_pretty(self).context("Failed to serialize the config")?;
            let mut f = File::create(&self.config_path)?;
            f.write_all(s.as_bytes())?;
            f.sync_all()?;
        }
        Ok(())
    }

    pub fn load(cfg_name: &str, user_dir: bool, readonly: bool) -> Result<Self> {
        let mut config_path = Self::get_config_dir(user_dir)?;
        config_path.push(cfg_name);

        Self::from_file(&config_path, readonly)
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let option = ConfigOption::from_str(key)?;
        self.set_option(option, value)
    }

    pub fn get(&self, key: &str) -> Result<String> {
        let option = ConfigOption::from_str(key)?;
        self.get_option(option)
    }

    pub fn get_all_options(&self) -> Vec<(String, String)> {
        self.get_all_config_options()
    }

    pub fn validate(&self) -> Result<()> {
        if self.token.is_none() {
            bail!("{}", crate::t!("error-auth-missing"));
        }
        TransportConfig::validate(&self.transport, false)?;
        Ok(())
    }

    pub fn get_hwid(&self) -> String {
        self.hwid
            .as_ref()
            .unwrap_or(
                &IdBuilder::new(Encryption::SHA256)
                    .add_component(HWIDComponent::OSName)
                    .add_component(HWIDComponent::SystemID)
                    .add_component(HWIDComponent::MachineName)
                    .add_component(HWIDComponent::CPUID)
                    .build("cloudpub")
                    .unwrap_or(self.agent_id.clone()),
            )
            .to_string()
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            agent_id: Uuid::new_v4().to_string(),
            hwid: None,
            config_path: PathBuf::new(),
            server: format!("https://{}", DOMAIN).parse().unwrap(),
            token: None,
            heartbeat_timeout: DEFAULT_HEARTBEAT_TIMEOUT_SECS,
            one_c_home: None,
            one_c_platform: None,
            one_c_publish_dir: None,
            minecraft_server: None,
            minecraft_java_opts: None,
            minimize_to_tray_on_close: Some(true),
            minimize_to_tray_on_start: Some(false),
            transport: TransportConfig::default(),
            readonly: false,
        }
    }
}
