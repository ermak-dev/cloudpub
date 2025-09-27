use crate::client::run_client;
use crate::commands::{Commands, ServiceAction};
pub use crate::config::{ClientConfig, ClientOpts};
use crate::ping;
use crate::service::{create_service_manager, ServiceStatus};
use crate::shell::get_cache_dir;
use anyhow::{bail, Context, Result};
use clap::Parser;
use cloudpub_common::logging::{init_log, WorkerGuard};
use cloudpub_common::protocol::message::Message;
use cloudpub_common::protocol::{
    ConnectState, EndpointClear, EndpointList, EndpointRemove, EndpointStart, EndpointStartAll,
    EndpointStop, ErrorKind, PerformUpgrade, Stop,
};
use cloudpub_common::{LONG_VERSION, VERSION};
use dirs::cache_dir;
use futures::future::FutureExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

const CONFIG_FILE: &str = "client.toml";

#[derive(Parser, Debug)]
#[command(about, version(VERSION), long_version(LONG_VERSION))]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
    #[clap(short, long, default_value = "debug", help = "Log level")]
    pub log_level: String,
    #[clap(short, long, default_value = "false", help = "Ouput log to console")]
    pub verbose: bool,
    #[clap(short, long, help = "Path to the config file")]
    pub conf: Option<String>,
    #[clap(short, long, default_value = "false", help = "Read-only config mode")]
    pub readonly: bool,
}

fn handle_service_command(action: &ServiceAction, config: &ClientConfig) -> Result<()> {
    // Determine config path to use for service
    let config_path = match action {
        ServiceAction::Install { conf: Some(path) } => {
            // Use provided config path
            PathBuf::from(path)
        }
        _ => {
            // Use default config path
            config.get_config_path().to_owned()
        }
    };

    // Check if the provided config path exists
    let actual_config = if config.token.is_some() {
        debug!("Service config has token: {:?}", config_path);
        config_path.clone()
    } else {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            use crate::service::ServiceConfig;
            // On Unix, check if running under sudo and use original user's config if available
            if let Some(sudo_config) = ServiceConfig::get_sudo_user_config_path() {
                sudo_config.clone()
            } else {
                config_path.clone()
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        config_path.clone()
    };

    if actual_config.as_os_str().is_empty() || !actual_config.exists() {
        bail!("Конфигурационный файл не найден ({:?}). Пожалуйста, укажите правильный путь к конфигурационному файлу с помощью параметра --config", config_path);
    }

    let cfg = ClientConfig::from_file(&actual_config, true).context(format!(
        "Не удалось загрузить конфигурацию из файла: {:?}",
        actual_config
    ))?;

    if cfg.token.is_none() {
        bail!("В конфигурации отсутствует токен аутентификации. Пожалуйста, войдите в систему с помощью команды 'clo login' перед установкой службы.");
    }
    // Create the appropriate service manager for the current platform
    let service_manager = create_service_manager(actual_config);

    match action {
        ServiceAction::Install { .. } => {
            service_manager.install()?;
            println!("{}", crate::t!("service-installed"));
        }
        ServiceAction::Uninstall => {
            service_manager.uninstall()?;
            println!("{}", crate::t!("service-uninstalled"));
        }
        ServiceAction::Start => {
            service_manager.start()?;
            println!("{}", crate::t!("service-started"));
        }
        ServiceAction::Stop => {
            service_manager.stop()?;
            println!("{}", crate::t!("service-stopped-service"));
        }
        ServiceAction::Status => {
            let status = service_manager.status()?;
            match status {
                ServiceStatus::Running => println!("{}", crate::t!("service-running")),
                ServiceStatus::Stopped => println!("{}", crate::t!("service-stopped-status")),
                ServiceStatus::NotInstalled => println!("{}", crate::t!("service-not-installed")),
                ServiceStatus::Unknown => println!("{}", crate::t!("service-status-unknown")),
            }
        }
    }

    Ok(())
}

pub fn init(args: &Cli) -> Result<(WorkerGuard, Arc<RwLock<ClientConfig>>)> {
    // Raise `nofile` limit on linux and mac
    if let Err(err) = fdlimit::raise_fd_limit() {
        warn!("Failed to raise file descriptor limit: {}", err);
    }

    // Create log directory
    let log_dir = cache_dir().context("Can't get cache dir")?.join("cloudpub");
    std::fs::create_dir_all(&log_dir).context("Can't create log dir")?;

    let log_file = log_dir.join("client.log");

    let guard = init_log(
        &args.log_level,
        &log_file,
        args.verbose,
        10 * 1024 * 1024,
        2,
    )
    .context("Failed to initialize logging")?;

    let config = if let Some(path) = args.conf.as_ref() {
        ClientConfig::from_file(&path.into(), args.readonly)?
    } else {
        ClientConfig::load(CONFIG_FILE, true, args.readonly)?
    };
    let config = Arc::new(RwLock::new(config));
    Ok((guard, config))
}

#[tokio::main]
pub async fn cli_main(cli: Cli, config: Arc<RwLock<ClientConfig>>) -> Result<()> {
    ctrlc::set_handler(move || {
        std::process::exit(1);
    })
    .context("Error setting Ctrl-C handler")?;

    let (command_tx, command_rx) = mpsc::channel(1024);
    main_loop(cli, config, command_tx, command_rx).await
}

fn make_spinner(msg: String) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    let style = ProgressStyle::default_spinner()
        .template("{spinner} {msg}")
        .unwrap();
    #[cfg(target_os = "windows")]
    let style = style.tick_chars("-\\|/ ");
    spinner.set_style(style);
    spinner.set_message(msg);
    #[cfg(unix)]
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    spinner
}

pub async fn main_loop(
    mut cli: Cli,
    config: Arc<RwLock<ClientConfig>>,
    command_tx: mpsc::Sender<Message>,
    command_rx: mpsc::Receiver<Message>,
) -> Result<()> {
    let mut opts = ClientOpts::default();
    let write_stdout = |res: String| {
        println!("{}", res);
    };

    let write_stderr = |res: String| {
        eprintln!("{}", res);
    };

    let (result_tx, mut result_rx) = mpsc::channel(1024);

    let mut pings = 1;

    opts.secondary = match &mut cli.command {
        Commands::Set(set_args) => {
            config.write().set(&set_args.key, &set_args.value)?;
            return Ok(());
        }
        Commands::Get(get_args) => {
            let value = config.read().get(&get_args.key)?;
            write_stdout(value);
            return Ok(());
        }
        Commands::Options => {
            let options = config.read().get_all_options();
            let mut output = String::new();
            for (key, value) in options {
                output.push_str(&format!("{}: {}\n", key, value));
            }
            write_stdout(output.trim_end().to_string());
            return Ok(());
        }
        Commands::Purge => {
            let cache_dir = get_cache_dir("")?;
            debug!(
                "{}",
                crate::t!("purge-cache-dir", "path" => cache_dir.to_str().unwrap())
            );
            std::fs::remove_dir_all(&cache_dir).ok();
            return Ok(());
        }
        Commands::Login(args) => {
            let email = match &args.email {
                Some(email) => email.clone(),
                None => {
                    // Prompt the user for email
                    print!("{}", crate::t!("enter-email"));
                    std::io::stdout().flush().ok();
                    let mut email = String::new();
                    std::io::stdin().read_line(&mut email)?;
                    email.trim().to_string()
                }
            };

            let password = match &args.password {
                Some(pwd) => pwd.clone(),
                None => {
                    // Try to read from environment first
                    if let Ok(pwd) = std::env::var("PASSWORD") {
                        pwd
                    } else {
                        // If not in environment, prompt the user
                        print!("{}", crate::t!("enter-password"));
                        std::io::stdout().flush().ok();
                        rpassword::read_password().unwrap_or_default()
                    }
                }
            };
            opts.credentials = Some((email, password));
            true
        }
        Commands::Logout => {
            config.write().token = None;
            config
                .write()
                .save()
                .context("Failed to save config after logout")?;
            write_stderr(crate::t!("session-terminated"));
            return Ok(());
        }
        Commands::Register(publish_args) => {
            config.read().validate()?;
            publish_args.parse()?;
            true
        }
        Commands::Publish(publish_args) => {
            config.read().validate()?;
            publish_args.parse()?;
            false
        }

        Commands::Ping(_) => {
            opts.transient = true;
            true
        }

        Commands::Run { run_as_service } => {
            opts.is_service = *run_as_service;
            false
        }

        Commands::Unpublish(_)
        | Commands::Start(_)
        | Commands::Stop(_)
        | Commands::Break
        | Commands::Ls
        | Commands::Clean
        | Commands::Upgrade => {
            config.read().validate()?;
            true
        }
        Commands::Service { action } => {
            return handle_service_command(action, &config.read());
        }
    };

    debug!("Config: {:?}", config);

    tokio::spawn(async move {
        if let Err(err) = run_client(config.clone(), opts, command_rx, result_tx)
            .boxed()
            .await
        {
            error!("Error running client: {:?}", err);
        }
    });

    let mut current_spinner = None;
    let multi_progress = MultiProgress::new();
    let mut progress_bars: HashMap<String, ProgressBar> = HashMap::new();

    loop {
        match result_rx
            .recv()
            .await
            .context("Failed to receive message")?
        {
            Message::Error(err) => {
                let kind: ErrorKind = err.kind.try_into().unwrap_or(ErrorKind::Fatal);
                if kind == ErrorKind::Fatal || kind == ErrorKind::AuthFailed {
                    command_tx.send(Message::Stop(Stop {})).await.ok();
                    bail!("{}", err.message);
                } else {
                    write_stderr(err.message.to_string());
                }
            }

            Message::UpgradeAvailable(info) => match cli.command {
                Commands::Upgrade => {
                    command_tx
                        .send(Message::PerformUpgrade(PerformUpgrade {
                            version: info.version.clone(),
                        }))
                        .await
                        .context("Failed to send upgrade message")?;
                }
                Commands::Run { .. } | Commands::Publish(_) => {
                    write_stderr(crate::t!("upgrade-available", "version" => info.version.clone()));
                }
                _ => {}
            },

            Message::EndpointAck(endpoint) => {
                if endpoint.status == Some("online".to_string()) {
                    match cli.command {
                        Commands::Ping(ref args) => {
                            current_spinner = Some(make_spinner(crate::t!("measuring-speed")));
                            let stats = ping::ping_test(endpoint, args.bare).await?;
                            current_spinner.take();
                            if args.bare {
                                write_stdout(stats.to_string());
                            } else {
                                write_stdout(stats);
                            }
                            pings -= 1;
                            if pings == 0 {
                                break;
                            }
                        }
                        Commands::Register(_) => {
                            write_stdout(
                                crate::t!("service-registered", "endpoint" => endpoint.to_string()),
                            );
                            break;
                        }
                        Commands::Publish(_) | Commands::Run { .. } => {
                            if endpoint.error.is_empty() {
                                write_stdout(
                                    crate::t!("service-published", "endpoint" => endpoint.to_string()),
                                )
                            } else {
                                write_stdout(
                                    crate::t!("service-error", "endpoint" => endpoint.to_string()),
                                )
                            }
                        }
                        _ => {}
                    }
                }
            }

            Message::EndpointStopAck(ep) => {
                write_stdout(crate::t!("service-stopped", "guid" => ep.guid));
                if matches!(cli.command, Commands::Unpublish(_)) {
                    break;
                }
            }

            Message::EndpointRemoveAck(ep) => {
                write_stdout(crate::t!("service-removed", "guid" => ep.guid));
                if matches!(cli.command, Commands::Unpublish(_)) {
                    break;
                }
            }

            Message::ConnectState(st) => match st.try_into().unwrap_or(ConnectState::Connecting) {
                ConnectState::Connecting => {
                    current_spinner = Some(make_spinner(crate::t!("connecting")));
                }

                ConnectState::Connected => {
                    if let Some(spinner) = current_spinner.take() {
                        spinner.finish_and_clear();
                    }

                    match cli.command {
                        Commands::Ls => {
                            command_tx
                                .send(Message::EndpointList(EndpointList {}))
                                .await?;
                        }
                        Commands::Clean => {
                            command_tx
                                .send(Message::EndpointClear(EndpointClear {}))
                                .await?;
                        }
                        Commands::Run { .. } => {
                            command_tx
                                .send(Message::EndpointStartAll(EndpointStartAll {}))
                                .await?;
                        }
                        Commands::Publish(ref endpoint) => {
                            command_tx
                                .send(Message::EndpointStart(endpoint.parse()?))
                                .await?;
                        }
                        Commands::Register(ref endpoint) => {
                            command_tx
                                .send(Message::EndpointStart(endpoint.parse()?))
                                .await?;
                        }
                        Commands::Unpublish(ref args) => {
                            command_tx
                                .send(Message::EndpointRemove(EndpointRemove {
                                    guid: args.guid.clone(),
                                }))
                                .await?;
                        }
                        Commands::Start(ref args) => {
                            command_tx
                                .send(Message::EndpointGuidStart(EndpointStart {
                                    guid: args.guid.clone(),
                                }))
                                .await?;
                        }
                        Commands::Stop(ref args) => {
                            command_tx
                                .send(Message::EndpointStop(EndpointStop {
                                    guid: args.guid.clone(),
                                }))
                                .await?;
                        }
                        Commands::Ping(ref args) => {
                            pings = args.num.unwrap_or(1);
                            for _i in 0..pings {
                                ping::publish(command_tx.clone()).await?;
                            }
                        }
                        Commands::Login(_) => {
                            write_stdout(crate::t!("client-authorized"));
                            break;
                        }
                        _ => {}
                    }
                }
                ConnectState::Disconnected => {
                    if let Some(spinner) = current_spinner.take() {
                        spinner.finish_and_clear();
                    }
                }
            },

            Message::Progress(info) => {
                let progress_guid = if info.guid.is_empty() {
                    "default"
                } else {
                    &info.guid
                };

                if info.current == 0 {
                    // Create a new progress bar for this GUID
                    let bar = multi_progress.add(ProgressBar::new(info.total as u64));
                    bar.set_message(info.message.clone());
                    bar.set_style(ProgressStyle::default_bar().template(&info.template)?);
                    progress_bars.insert(progress_guid.to_string(), bar);
                } else if info.current >= info.total {
                    // Progress completed, remove and finish the progress bar
                    if let Some(progress_bar) = progress_bars.remove(progress_guid) {
                        progress_bar.finish_and_clear();
                    }
                } else if let Some(bar) = progress_bars.get(progress_guid) {
                    // Update existing progress bar
                    bar.set_position(info.current as u64);
                    if !info.message.is_empty() {
                        bar.set_message(info.message.clone());
                    }
                }
            }

            Message::EndpointListAck(list) => {
                if list.endpoints.is_empty() {
                    write_stdout(crate::t!("no-registered-services"));
                } else {
                    let mut output = String::new();
                    let use_colors = io::stderr().is_terminal();

                    for ep in &list.endpoints {
                        let status = ep.status.as_deref().unwrap_or("unknown");
                        let colored_status = if use_colors {
                            match status {
                                "online" => format!("\x1b[32m{}\x1b[0m ", status), // Green
                                "offline" => format!("\x1b[31m{}\x1b[0m", status), // Red
                                "starting" => format!("\x1b[33m{}\x1b[0m", status), // Yellow
                                "stopping" => format!("\x1b[33m{}\x1b[0m", status), // Yellow
                                "error" => format!("\x1b[31m{}\x1b[0m   ", status), // Red
                                _ => status.to_string(),
                            }
                        } else {
                            status.to_string()
                        };

                        output.push_str(&format!("{} {} {}\n", colored_status, ep.guid, ep));
                    }
                    write_stdout(output);
                }
                if !matches!(cli.command, Commands::Run { .. }) {
                    break;
                }
            }

            Message::EndpointClearAck(_) => {
                write_stdout(crate::t!("all-services-removed"));
                if !matches!(cli.command, Commands::Run { .. }) {
                    break;
                }
            }

            other => {
                debug!("Unhandled message: {:?}", other);
            }
        }
    }

    command_tx.send(Message::Stop(Stop {})).await.ok();

    Ok(())
}
