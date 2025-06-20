use crate::service::{ServiceConfig, ServiceManager as ServiceManagerTrait, ServiceStatus};
use anyhow::{anyhow, Context, Result};
use common::protocol::message::Message;
use common::protocol::Stop;
use std::ffi::OsString;
use std::time::Duration;
use tokio::sync::broadcast;
use winreg::enums::*;
use winreg::RegKey;

use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
    ServiceInfo, ServiceStartType, ServiceState, ServiceStatus as WinServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_service::{define_windows_service, service_dispatcher};

define_windows_service!(ffi_service_main, service_main);

pub struct WindowsServiceManager {
    config: ServiceConfig,
}

impl WindowsServiceManager {
    pub fn new(config: ServiceConfig) -> Self {
        Self { config }
    }

    // Registry key for storing the config file path
    fn registry_key() -> Result<RegKey> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.create_subkey("SOFTWARE\\CloudPub")?;
        Ok(key.0)
    }

    // Store config file path in registry
    fn store_config_path(&self, config_path: &str) -> Result<()> {
        let key = Self::registry_key()?;
        key.set_value("ConfigPath", &config_path)?;
        Ok(())
    }

    // Retrieve config file path from registry
    fn get_config_path() -> Result<String> {
        let key = Self::registry_key()?;
        let config_path: String = key
            .get_value("ConfigPath")
            .context("Failed to get config path from registry")?;
        Ok(config_path)
    }

    #[cfg(windows)]
    fn get_service_info(&self) -> ServiceInfo {
        ServiceInfo {
            name: OsString::from(&self.config.name),
            display_name: OsString::from(&self.config.display_name),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: self.config.executable_path.clone(),
            launch_arguments: self.config.args.iter().map(|s| s.into()).collect(),
            dependencies: vec![],
            account_name: None,
            account_password: None,
        }
    }
}

impl ServiceManagerTrait for WindowsServiceManager {
    fn install(&self) -> Result<()> {
        let manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CREATE_SERVICE)?;

        let service_info = self.get_service_info();

        manager.create_service(
            &service_info,
            ServiceAccess::QUERY_STATUS | ServiceAccess::START | ServiceAccess::STOP,
        )?;

        // Store config file path in registry if available
        if let Some(config_path) = &self.config.config_path {
            if let Some(path_str) = config_path.to_str() {
                self.store_config_path(path_str)?;
            }
        }

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

        let service = manager.open_service(
            &self.config.name,
            ServiceAccess::DELETE | ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
        )?;

        // Try to stop the service if it's running
        let service_status = service.query_status()?;
        if service_status.current_state != ServiceState::Stopped {
            service.stop()?;

            // Wait for the service to stop
            for _ in 0..10 {
                let status = service.query_status()?;
                if status.current_state == ServiceState::Stopped {
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }

        service.delete()?;
        Ok(())
    }

    fn start(&self) -> Result<()> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

        let service = manager.open_service(
            &self.config.name,
            ServiceAccess::START | ServiceAccess::QUERY_STATUS,
        )?;

        let service_status = service.query_status()?;
        if service_status.current_state == ServiceState::Running {
            return Ok(());
        }

        service.start::<&str>(&[])?;
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

        let service = manager.open_service(
            &self.config.name,
            ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
        )?;

        let service_status = service.query_status()?;
        if service_status.current_state == ServiceState::Stopped {
            return Ok(());
        }

        service.stop()?;
        Ok(())
    }

    fn status(&self) -> Result<ServiceStatus> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

        let service = match manager.open_service(&self.config.name, ServiceAccess::QUERY_STATUS) {
            Ok(service) => service,
            Err(_) => return Ok(ServiceStatus::NotInstalled),
        };

        let service_status = service.query_status()?;

        match service_status.current_state {
            ServiceState::Running => Ok(ServiceStatus::Running),
            ServiceState::Stopped => Ok(ServiceStatus::Stopped),
            _ => Ok(ServiceStatus::Unknown),
        }
    }
}

// Service main function that will be called by the Windows service manager
fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        // Log error using tracing
        use tracing::error;
        error!("Service failed to run: {}", e);
    }
}

fn run_service() -> Result<()> {
    // Create a channel for sending stop commands
    let (stop_tx, _) = broadcast::channel::<()>(1);
    let stop_tx_clone = stop_tx.clone();

    // Set up the service control handler
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                // Send stop signal through the channel
                let _ = stop_tx_clone.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register("cloudpub", event_handler)?;

    // Tell the service manager that the service is running
    status_handle.set_service_status(WinServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    // Run the application with stop signal
    run_app(stop_tx);

    // When done, update the service status to stopped
    status_handle.set_service_status(WinServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

// Function to be called when running as a Windows service
pub fn run_as_service() -> Result<()> {
    service_dispatcher::start("cloudpub", ffi_service_main)
        .map_err(|e| anyhow!("Failed to start service dispatcher: {:?}", e))
}

#[tokio::main]
pub async fn run_app(stop_tx: broadcast::Sender<()>) {
    use crate::base::{init, main_loop, Cli};
    use crate::commands::Commands;
    use anyhow::Context;
    use tokio::sync::mpsc;

    // Get config path from registry
    let config_path = WindowsServiceManager::get_config_path().unwrap_or_else(|_| {
        // Fallback to default path if registry key not found
        "C:\\ProgramData\\CloudPub.toml".to_string()
    });

    let cli: Cli = Cli {
        command: Commands::Run,
        conf: Some(config_path),
        verbose: false,
        readonly: false,
        log_level: "debug".to_string(),
    };
    let (_guard, config) = match init(&cli, false).context("Failed to initialize config") {
        Ok(r) => r,
        Err(err) => {
            tracing::error!("Failed to initialize: {:?}", err);
            return;
        }
    };

    let (command_tx, command_rx) = mpsc::channel(1024);

    // Create a task to handle the stop signal
    let command_tx_clone = command_tx.clone();
    let mut stop_rx = stop_tx.subscribe();

    let stop_handler = tokio::spawn(async move {
        if stop_rx.recv().await.is_ok() {
            // Send stop command when service stop signal is received
            let _ = command_tx_clone.send(Message::Stop(Stop {}));
        }
    });

    // Run the main loop
    if let Err(err) = main_loop(cli, config, command_tx, command_rx, None, None).await {
        tracing::error!("Error running main loop: {}", err);
    }

    // Make sure the stop handler is terminated
    stop_handler.abort();
}
