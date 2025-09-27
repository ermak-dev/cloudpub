use crate::service::{ServiceConfig, ServiceManager as ServiceManagerTrait, ServiceStatus};
use anyhow::{anyhow, Result};
use cloudpub_common::protocol::message::Message;
use cloudpub_common::protocol::Stop;
use std::ffi::OsString;
use std::io::Error;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

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
        debug!("Installing Windows service '{}'...", self.config.name);
        debug!(
            "Service executable: {}",
            self.config.executable_path.display()
        );
        debug!("Service args: {:?}", self.config.args);

        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )
        .map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                e,
                os_error
            )
        })?;
        debug!("Connected to Windows service manager with CREATE_SERVICE access");

        let service_info = self.get_service_info();
        debug!(
            "Service info prepared: display_name={}",
            self.config.display_name
        );

        manager
            .create_service(
                &service_info,
                ServiceAccess::QUERY_STATUS | ServiceAccess::START | ServiceAccess::STOP,
            )
            .map_err(|e| {
                let os_error = Error::last_os_error();
                error!(
                    "Failed to create service: {} (Windows error: {})",
                    e, os_error
                );
                anyhow!(
                    "Failed to create service '{}': {} (Windows error: {})",
                    self.config.name,
                    e,
                    os_error
                )
            })?;
        info!(
            "Windows service '{}' created successfully",
            self.config.name
        );

        // Copy config to system location
        debug!("Copying config to system location...");
        self.config.copy_config_to_system()?;
        debug!("Config copied to system location");

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        debug!("Uninstalling Windows service '{}'...", self.config.name);

        // First, try to stop the service if it's running
        // We need a separate connection for stop operation
        let stop_manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(
                |e| {
                    let os_error = Error::last_os_error();
                    error!(
                "Failed to connect to Windows Service Manager for stop: {} (Windows error: {})",
                e, os_error
            );
                    anyhow!(
                        "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                        e,
                        os_error
                    )
                },
            )?;
        debug!("Connected to Windows service manager for stop operation");

        // Try to stop the service first
        if let Ok(service) = stop_manager.open_service(
            &self.config.name,
            ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
        ) {
            let service_status = service.query_status()?;
            debug!("Current service state: {:?}", service_status.current_state);

            if service_status.current_state != ServiceState::Stopped {
                debug!("Stopping service before deletion...");
                service.stop().map_err(|e| {
                    let os_error = Error::last_os_error();
                    warn!(
                        "Failed to stop service during uninstall: {} (Windows error: {})",
                        e, os_error
                    );
                    e
                })?;

                // Wait for the service to stop
                for i in 0..10 {
                    let status = service.query_status()?;
                    debug!(
                        "Waiting for service to stop, attempt {} - state: {:?}",
                        i + 1,
                        status.current_state
                    );
                    if status.current_state == ServiceState::Stopped {
                        debug!("Service stopped successfully");
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }

        // Now delete the service
        let delete_manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(
                |e| {
                    let os_error = Error::last_os_error();
                    error!(
                "Failed to connect to Windows Service Manager for delete: {} (Windows error: {})",
                e, os_error
            );
                    anyhow!(
                        "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                        e,
                        os_error
                    )
                },
            )?;
        debug!("Connected to Windows service manager for delete operation");

        let service = delete_manager
            .open_service(&self.config.name, ServiceAccess::DELETE)
            .map_err(|e| {
                let os_error = Error::last_os_error();
                error!(
                    "Failed to open service for deletion: {} (Windows error: {})",
                    e, os_error
                );
                anyhow!(
                    "Failed to open service '{}' for deletion: {} (Windows error: {})",
                    self.config.name,
                    e,
                    os_error
                )
            })?;
        debug!("Opened service '{}' for deletion", self.config.name);

        service.delete().map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to delete service: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to delete service '{}': {} (Windows error: {})",
                self.config.name,
                e,
                os_error
            )
        })?;
        info!(
            "Windows service '{}' uninstalled successfully",
            self.config.name
        );
        Ok(())
    }

    fn start(&self) -> Result<()> {
        debug!("Starting Windows service '{}'...", self.config.name);

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                e,
                os_error
            )
        })?;
        debug!("Connected to Windows service manager");

        // First check if service is already running
        let query_service = manager
            .open_service(&self.config.name, ServiceAccess::QUERY_STATUS)
            .map_err(|e| {
                let os_error = Error::last_os_error();
                error!(
                    "Failed to open service for status query: {} (Windows error: {})",
                    e, os_error
                );
                anyhow!(
                    "Failed to open service '{}' for status query: {} (Windows error: {})",
                    self.config.name,
                    e,
                    os_error
                )
            })?;

        let service_status = query_service.query_status()?;
        debug!("Current service state: {:?}", service_status.current_state);

        if service_status.current_state == ServiceState::Running {
            debug!("Service is already running");
            return Ok(());
        }

        // Now open for START access
        let service = manager
            .open_service(&self.config.name, ServiceAccess::START)
            .map_err(|e| {
                let os_error = Error::last_os_error();
                error!(
                    "Failed to open service for start: {} (Windows error: {})",
                    e, os_error
                );
                anyhow!(
                    "Failed to open service '{}' for start: {} (Windows error: {})",
                    self.config.name,
                    e,
                    os_error
                )
            })?;
        debug!("Opened service '{}' for starting", self.config.name);

        debug!("Starting service...");
        service.start::<&str>(&[]).map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to start service: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to start service '{}': {} (Windows error: {})",
                self.config.name,
                e,
                os_error
            )
        })?;
        info!(
            "Windows service '{}' start command sent successfully",
            self.config.name
        );
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        debug!("Stopping Windows service '{}'...", self.config.name);

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                e,
                os_error
            )
        })?;
        debug!("Connected to Windows service manager");

        // First check if service is already stopped
        let query_service = manager
            .open_service(&self.config.name, ServiceAccess::QUERY_STATUS)
            .map_err(|e| {
                let os_error = Error::last_os_error();
                error!(
                    "Failed to open service for status query: {} (Windows error: {})",
                    e, os_error
                );
                anyhow!(
                    "Failed to open service '{}' for status query: {} (Windows error: {})",
                    self.config.name,
                    e,
                    os_error
                )
            })?;

        let service_status = query_service.query_status()?;
        debug!("Current service state: {:?}", service_status.current_state);

        if service_status.current_state == ServiceState::Stopped {
            debug!("Service is already stopped");
            return Ok(());
        }

        // Now open for STOP access
        let service = manager
            .open_service(&self.config.name, ServiceAccess::STOP)
            .map_err(|e| {
                let os_error = Error::last_os_error();
                error!(
                    "Failed to open service for stop: {} (Windows error: {})",
                    e, os_error
                );
                anyhow!(
                    "Failed to open service '{}' for stop: {} (Windows error: {})",
                    self.config.name,
                    e,
                    os_error
                )
            })?;
        debug!("Opened service '{}' for stopping", self.config.name);

        debug!("Stopping service...");
        service.stop().map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to stop service: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to stop service '{}': {} (Windows error: {})",
                self.config.name,
                e,
                os_error
            )
        })?;
        info!(
            "Windows service '{}' stop command sent successfully",
            self.config.name
        );
        Ok(())
    }

    fn status(&self) -> Result<ServiceStatus> {
        debug!(
            "Querying status of Windows service '{}'...",
            self.config.name
        );

        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to connect to Windows Service Manager: {} (Windows error: {})",
                e,
                os_error
            )
        })?;
        debug!("Connected to Windows service manager");

        let service = match manager.open_service(&self.config.name, ServiceAccess::QUERY_STATUS) {
            Ok(service) => service,
            Err(e) => {
                debug!(
                    "Service '{}' not found or inaccessible: {}",
                    self.config.name, e
                );
                return Ok(ServiceStatus::NotInstalled);
            }
        };

        let service_status = service.query_status()?;
        debug!("Service state: {:?}", service_status.current_state);

        let status = match service_status.current_state {
            ServiceState::Running => ServiceStatus::Running,
            ServiceState::Stopped => ServiceStatus::Stopped,
            _ => ServiceStatus::Unknown,
        };

        debug!("Service '{}' status: {:?}", self.config.name, status);
        Ok(status)
    }
}

// Service main function that will be called by the Windows service manager
fn service_main(arguments: Vec<OsString>) {
    debug!(
        "Windows service main function called with arguments: {:?}",
        arguments
    );
    if let Err(e) = run_service() {
        error!("Service failed to run: {}", e);
    }
}

fn run_service() -> Result<()> {
    debug!("Starting Windows service run loop");

    // Create a channel for sending stop commands
    let (stop_tx, _) = broadcast::channel::<()>(1);
    let stop_tx_clone = stop_tx.clone();

    // Set up the service control handler
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                debug!("Received STOP control event from Windows Service Manager");
                // Send stop signal through the channel
                let _ = stop_tx_clone.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => {
                debug!("Received INTERROGATE control event from Windows Service Manager");
                ServiceControlHandlerResult::NoError
            }
            _ => {
                debug!("Received unhandled control event: {:?}", control_event);
                ServiceControlHandlerResult::NotImplemented
            }
        }
    };

    debug!("Registering service control handler");
    let status_handle =
        service_control_handler::register("cloudpub", event_handler).map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to register service control handler: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to register service control handler: {} (Windows error: {})",
                e,
                os_error
            )
        })?;
    debug!("Service control handler registered successfully");

    // Tell the service manager that the service is running
    debug!("Setting service status to RUNNING");
    status_handle
        .set_service_status(WinServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to set service status to RUNNING: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to set service status to RUNNING: {} (Windows error: {})",
                e,
                os_error
            )
        })?;
    info!("Windows service status set to RUNNING");

    // Run the application with stop signal
    debug!("Starting main application loop");
    run_app(stop_tx);

    // When done, update the service status to stopped
    debug!("Setting service status to STOPPED");
    status_handle
        .set_service_status(WinServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .map_err(|e| {
            let os_error = Error::last_os_error();
            error!(
                "Failed to set service status to STOPPED: {} (Windows error: {})",
                e, os_error
            );
            anyhow!(
                "Failed to set service status to STOPPED: {} (Windows error: {})",
                e,
                os_error
            )
        })?;
    info!("Windows service status set to STOPPED");

    Ok(())
}

// Function to be called when running as a Windows service
pub fn run_as_service() -> Result<()> {
    debug!("Starting Windows service dispatcher for 'cloudpub'");
    service_dispatcher::start("cloudpub", ffi_service_main).map_err(|e| {
        error!("Failed to start service dispatcher: {:?}", e);
        anyhow!("Failed to start service dispatcher: {:?}", e)
    })
}

#[tokio::main]
pub async fn run_app(stop_tx: broadcast::Sender<()>) {
    use crate::base::{init, main_loop, Cli};
    use crate::commands::Commands;
    use crate::service::ServiceConfig;
    use anyhow::Context;
    use tokio::sync::mpsc;

    debug!("Windows service run_app started");

    // Use system config path for service
    let config_path = ServiceConfig::get_system_config_path()
        .to_str()
        .unwrap()
        .to_string();
    debug!("Using config path: {}", config_path);

    let cli: Cli = Cli {
        command: Commands::Run {
            run_as_service: true,
        },
        conf: Some(config_path.clone()),
        verbose: false,
        readonly: false,
        log_level: "debug".to_string(),
    };

    debug!("Initializing service with config: {}", config_path);
    let (_guard, config) = match init(&cli).context("Failed to initialize config") {
        Ok(r) => {
            debug!("Service initialization successful");
            r
        }
        Err(err) => {
            error!("Failed to initialize service: {:?}", err);
            return;
        }
    };

    let (command_tx, command_rx) = mpsc::channel(1024);
    debug!("Command channels created");

    // Create a task to handle the stop signal
    let command_tx_clone = command_tx.clone();
    let mut stop_rx = stop_tx.subscribe();

    debug!("Spawning stop signal handler task");
    let stop_handler = tokio::spawn(async move {
        if stop_rx.recv().await.is_ok() {
            debug!("Stop signal received, sending Stop message");
            // Send stop command when service stop signal is received
            let _ = command_tx_clone.send(Message::Stop(Stop {})).await;
            debug!("Stop message sent");
        }
    });

    // Run the main loop
    info!("Starting main service loop");
    if let Err(err) = main_loop(cli, config, command_tx, command_rx).await {
        error!("Error running main loop: {}", err);
    }

    // Make sure the stop handler is terminated
    debug!("Aborting stop handler task");
    stop_handler.abort();
    debug!("Windows service run_app completed");
}
