use anyhow::Result;
use std::path::PathBuf;
use std::{env, fs};
use tracing::debug;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

// Common service trait
pub trait ServiceManager {
    fn install(&self) -> Result<()>;
    fn uninstall(&self) -> Result<()>;
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn status(&self) -> Result<ServiceStatus>;
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum ServiceStatus {
    Running,
    Stopped,
    NotInstalled,
    Unknown,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ServiceConfig {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub executable_path: PathBuf,
    pub args: Vec<String>,
    pub config_path: PathBuf,
}

impl ServiceConfig {
    /// Get the system config path for the service based on the OS
    pub fn get_system_config_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            // For Windows service running as SYSTEM
            PathBuf::from(r"C:\ProgramData\cloudpub\client.toml")
        }
        #[cfg(target_os = "linux")]
        {
            // For Linux service running as root
            PathBuf::from("/root/.config/cloudpub/client.toml")
        }
        #[cfg(target_os = "macos")]
        {
            // For macOS service running as root
            PathBuf::from("/var/root/.config/cloudpub/client.toml")
        }
        #[cfg(target_os = "android")]
        {
            PathBuf::from("/data/data/com.termux/files/home/.config/cloudpub/client.toml")
        }
    }

    /// Get the original user's config path when running under sudo
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub fn get_sudo_user_config_path() -> Option<PathBuf> {
        // Check if we're running under sudo
        if let Ok(sudo_user) = env::var("SUDO_USER") {
            debug!("Detected SUDO_USER: {}", sudo_user);

            // Get the home directory for the sudo user
            #[cfg(target_os = "linux")]
            let home_base = "/home";
            #[cfg(target_os = "macos")]
            let home_base = "/Users";

            let user_home = PathBuf::from(home_base).join(&sudo_user);
            let config_path = user_home.join(".config/cloudpub/client.toml");

            debug!("Check for sudo user config location: {:?}", config_path);
            if config_path.exists() {
                debug!("Found config at sudo user's location: {:?}", config_path);
                return Some(config_path);
            }
        }
        debug!("No SUDO_USER detected or config file does not exist");
        None
    }

    /// Copy the provided config to the system location
    pub fn copy_config_to_system(&self) -> Result<()> {
        let system_config_path = Self::get_system_config_path();

        // Create parent directory if it doesn't exist
        if let Some(parent) = system_config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Copy the config file
        if self.config_path != system_config_path {
            fs::copy(&self.config_path, &system_config_path)?;
            debug!(
                "Copied config from {:?} to {:?}",
                self.config_path, system_config_path
            );
        }

        Ok(())
    }
}

pub fn create_service_manager(config_path: PathBuf) -> Box<dyn ServiceManager> {
    // Get the current executable path
    let exe_path = env::current_exe().expect("Failed to get current executable path");

    // Prepare config file argument using system config path
    let mut args = Vec::new();

    // Use system config path for service
    let system_config_path = ServiceConfig::get_system_config_path();
    let path_str = system_config_path.to_str().unwrap();
    args.push("--conf".to_string());
    args.push(path_str.to_string());

    // Add the run command for the service
    args.push("run".to_string());
    args.push("--run-as-service".to_string());

    // Create service configuration
    let config = ServiceConfig {
        #[cfg(target_os = "macos")]
        name: "ru.cloudpub.clo".to_string(),
        #[cfg(not(target_os = "macos"))]
        name: "cloudpub".to_string(),
        display_name: "CloudPub Client".to_string(),
        description: "CloudPub Client Service".to_string(),
        executable_path: exe_path,
        args,
        config_path,
    };

    debug!("Service configuration: {:?}", config);

    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsServiceManager::new(config))
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxServiceManager::new(config))
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSServiceManager::new(config))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        panic!("Unsupported platform for service management");
    }
}
