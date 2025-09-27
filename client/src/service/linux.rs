use crate::service::{ServiceConfig, ServiceManager, ServiceStatus};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

// Include the systemd service template at compile time
const SYSTEMD_SERVICE_TEMPLATE: &str = include_str!("templates/systemd.service");

pub struct LinuxServiceManager {
    config: ServiceConfig,
}

impl LinuxServiceManager {
    pub fn new(config: ServiceConfig) -> Self {
        Self { config }
    }

    fn service_file_path(&self) -> String {
        format!("/etc/systemd/system/{}.service", self.config.name)
    }

    fn create_service_file(&self) -> Result<()> {
        let executable = self.config.executable_path.to_string_lossy();
        let args = self.config.args.join(" ");

        // Replace placeholders in the template
        let service_content = SYSTEMD_SERVICE_TEMPLATE
            .replace("{DESCRIPTION}", &self.config.description)
            .replace("{EXECUTABLE}", &executable)
            .replace("{ARGS}", &args);

        fs::write(self.service_file_path(), service_content)
            .context("Failed to write systemd service file")
    }
}

impl ServiceManager for LinuxServiceManager {
    fn install(&self) -> Result<()> {
        // Copy config to system location
        self.config.copy_config_to_system()?;

        // Create the service file
        self.create_service_file()?;

        // Reload systemd to recognize the new service
        Command::new("systemctl")
            .args(["daemon-reload"])
            .status()
            .context("Failed to reload systemd")?;

        // Enable the service to start on boot
        Command::new("systemctl")
            .args(["enable", &self.config.name])
            .status()
            .context("Failed to enable service")?;

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        // Stop the service if it's running
        let _ = self.stop();

        // Disable the service
        Command::new("systemctl")
            .args(["disable", &self.config.name])
            .status()
            .context("Failed to disable service")?;

        // Remove the service file
        if Path::new(&self.service_file_path()).exists() {
            fs::remove_file(self.service_file_path()).context("Failed to remove service file")?;
        }

        // Reload systemd
        Command::new("systemctl")
            .args(["daemon-reload"])
            .status()
            .context("Failed to reload systemd")?;

        Ok(())
    }

    fn start(&self) -> Result<()> {
        Command::new("systemctl")
            .args(["start", &self.config.name])
            .status()
            .context("Failed to start service")?;
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Command::new("systemctl")
            .args(["stop", &self.config.name])
            .status()
            .context("Failed to stop service")?;
        Ok(())
    }

    fn status(&self) -> Result<ServiceStatus> {
        let service_path = self.service_file_path();
        let service_file = Path::new(&service_path);
        if !service_file.exists() {
            return Ok(ServiceStatus::NotInstalled);
        }

        let output = Command::new("systemctl")
            .args(["is-active", &self.config.name])
            .output()
            .context("Failed to check service status")?;

        let status_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

        match status_str.as_str() {
            "active" => Ok(ServiceStatus::Running),
            "inactive" => Ok(ServiceStatus::Stopped),
            _ => Ok(ServiceStatus::Unknown),
        }
    }
}
