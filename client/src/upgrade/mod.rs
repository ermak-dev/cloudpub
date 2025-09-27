use crate::config::ClientConfig;
use crate::shell::get_cache_dir;
use anyhow::{Context, Result};
use cloudpub_common::protocol::message::Message;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

// Include script templates at compile time
#[cfg(unix)]
const UNIX_SERVICE_UPDATE_SCRIPT: &str = include_str!("scripts/unix_service_update.sh");
#[cfg(unix)]
const UNIX_CLIENT_UPDATE_SCRIPT: &str = include_str!("scripts/unix_client_update.sh");
#[cfg(windows)]
const WINDOWS_SERVICE_UPDATE_SCRIPT: &str = include_str!("scripts/windows_service_update.bat");
#[cfg(windows)]
const WINDOWS_CLIENT_UPDATE_SCRIPT: &str = include_str!("scripts/windows_client_update.bat");
#[cfg(windows)]
const WINDOWS_MSI_INSTALL_SCRIPT: &str = include_str!("scripts/windows_msi_install.bat");
#[cfg(target_os = "macos")]
const MACOS_DMG_INSTALL_SCRIPT: &str = include_str!("scripts/macos_dmg_install.sh");
#[cfg(target_os = "linux")]
const LINUX_DEB_INSTALL_SCRIPT: &str = include_str!("scripts/linux_deb_install.sh");
#[cfg(target_os = "linux")]
const LINUX_RPM_INSTALL_SCRIPT: &str = include_str!("scripts/linux_rpm_install.sh");
#[cfg(target_os = "linux")]
const SYSTEMD_ONESHOT_TEMPLATE: &str = include_str!("scripts/systemd_oneshot.service");
#[cfg(target_os = "macos")]
const LAUNCHD_ONESHOT_TEMPLATE: &str = include_str!("scripts/launchd_oneshot.plist");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformDownloads {
    pub gui: Option<String>,
    pub cli: Option<String>,
    pub rpm: Option<String>,
    pub deb: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsDownloads {
    pub name: String,
    pub platforms: HashMap<String, PlatformDownloads>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub macos: OsDownloads,
    pub windows: OsDownloads,
    pub linux: OsDownloads,
}

impl DownloadConfig {
    pub fn new(base_url: &str, version: &str, branch: &str) -> Self {
        #[cfg(target_os = "macos")]
        let macos_platforms = {
            let mut platforms = HashMap::new();
            platforms.insert(
                "aarch64".to_string(),
                PlatformDownloads {
                    gui: Some(format!(
                        "{base_url}cloudpub-{version}-{branch}-macos-aarch64.dmg"
                    )),
                    cli: Some(format!(
                        "{base_url}clo-{version}-{branch}-macos-aarch64.tar.gz"
                    )),
                    rpm: None,
                    deb: None,
                },
            );
            platforms.insert(
                "x86_64".to_string(),
                PlatformDownloads {
                    gui: Some(format!(
                        "{base_url}cloudpub-{version}-{branch}-macos-x86_64.dmg"
                    )),
                    cli: Some(format!(
                        "{base_url}clo-{version}-{branch}-macos-x86_64.tar.gz"
                    )),
                    rpm: None,
                    deb: None,
                },
            );
            platforms
        };

        #[cfg(target_os = "windows")]
        let windows_platforms = {
            let mut platforms = HashMap::new();
            platforms.insert(
                "x86_64".to_string(),
                PlatformDownloads {
                    gui: Some(format!(
                        "{base_url}cloudpub-{version}-{branch}-windows-x86_64.msi"
                    )),
                    cli: Some(format!(
                        "{base_url}clo-{version}-{branch}-windows-x86_64.zip"
                    )),
                    rpm: None,
                    deb: None,
                },
            );
            platforms
        };

        #[cfg(target_os = "linux")]
        let linux_platforms = {
            let mut platforms = HashMap::new();
            platforms.insert(
                "arm".to_string(),
                PlatformDownloads {
                    gui: None,
                    cli: Some(format!("{base_url}clo-{version}-{branch}-linux-arm.tar.gz")),
                    rpm: None,
                    deb: None,
                },
            );
            platforms.insert(
                "armv5te".to_string(),
                PlatformDownloads {
                    gui: None,
                    cli: Some(format!(
                        "{base_url}clo-{version}-{branch}-linux-armv5te.tar.gz"
                    )),
                    rpm: None,
                    deb: None,
                },
            );
            platforms.insert(
                "aarch64".to_string(),
                PlatformDownloads {
                    gui: None,
                    cli: Some(format!(
                        "{base_url}clo-{version}-{branch}-linux-aarch64.tar.gz"
                    )),
                    rpm: None,
                    deb: None,
                },
            );
            platforms.insert(
                "mips".to_string(),
                PlatformDownloads {
                    gui: None,
                    cli: Some(format!(
                        "{base_url}clo-{version}-{branch}-linux-mips.tar.gz"
                    )),
                    rpm: None,
                    deb: None,
                },
            );
            platforms.insert(
                "mipsel".to_string(),
                PlatformDownloads {
                    gui: None,
                    cli: Some(format!(
                        "{base_url}clo-{version}-{branch}-linux-mipsel.tar.gz"
                    )),
                    rpm: None,
                    deb: None,
                },
            );
            platforms.insert(
                "i686".to_string(),
                PlatformDownloads {
                    gui: None,
                    cli: Some(format!(
                        "{base_url}clo-{version}-{branch}-linux-i686.tar.gz"
                    )),
                    rpm: None,
                    deb: None,
                },
            );
            platforms.insert(
                "x86_64".to_string(),
                PlatformDownloads {
                    gui: None,
                    cli: Some(format!(
                        "{base_url}clo-{version}-{branch}-linux-x86_64.tar.gz"
                    )),
                    rpm: Some(format!(
                        "{base_url}cloudpub-{version}-{branch}-linux-x86_64.rpm"
                    )),
                    deb: Some(format!(
                        "{base_url}cloudpub-{version}-{branch}-linux-x86_64.deb"
                    )),
                },
            );
            platforms
        };

        Self {
            #[cfg(target_os = "macos")]
            macos: OsDownloads {
                name: "macOS".to_string(),
                platforms: macos_platforms,
            },
            #[cfg(not(target_os = "macos"))]
            macos: OsDownloads {
                name: "macOS".to_string(),
                platforms: HashMap::new(),
            },
            #[cfg(target_os = "windows")]
            windows: OsDownloads {
                name: "Windows".to_string(),
                platforms: windows_platforms,
            },
            #[cfg(not(target_os = "windows"))]
            windows: OsDownloads {
                name: "Windows".to_string(),
                platforms: HashMap::new(),
            },
            #[cfg(target_os = "linux")]
            linux: OsDownloads {
                name: "Linux".to_string(),
                platforms: linux_platforms,
            },
            #[cfg(not(target_os = "linux"))]
            linux: OsDownloads {
                name: "Linux".to_string(),
                platforms: HashMap::new(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadType {
    Gui,
    Cli,
    Rpm,
    Deb,
}

pub fn get_current_os() -> String {
    #[cfg(target_os = "macos")]
    return "macos".to_string();
    #[cfg(target_os = "windows")]
    return "windows".to_string();
    #[cfg(target_os = "linux")]
    return "linux".to_string();
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    return std::env::consts::OS.to_string();
}

pub fn get_current_arch() -> String {
    #[cfg(target_arch = "x86_64")]
    return "x86_64".to_string();
    #[cfg(target_arch = "aarch64")]
    return "aarch64".to_string();
    #[cfg(target_arch = "arm")]
    return "arm".to_string();
    #[cfg(target_arch = "x86")]
    return "i686".to_string();
    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "x86"
    )))]
    return std::env::consts::ARCH.to_string();
}

#[cfg(target_os = "linux")]
pub fn get_linux_package_type() -> Result<String> {
    // Check for dpkg (Debian/Ubuntu systems)
    if std::process::Command::new("which")
        .arg("dpkg")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        return Ok("deb".to_string());
    }

    // Check for rpm (Red Hat/SUSE systems)
    if std::process::Command::new("which")
        .arg("rpm")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        return Ok("rpm".to_string());
    }

    Err(anyhow::anyhow!("Could not detect package type"))
}

pub fn get_download_url(
    base_url: &str,
    download_type: &DownloadType,
    version: &str,
    branch: &str,
) -> Result<String> {
    let base_url = format!("{}download/{}/", base_url, branch);
    let config = DownloadConfig::new(&base_url, version, branch);
    let os = get_current_os();
    let arch = get_current_arch();

    let os_downloads = match os.as_str() {
        "macos" => &config.macos,
        "windows" => &config.windows,
        "linux" => &config.linux,
        _ => {
            return Err(anyhow::anyhow!("Unsupported OS: {}", os))
                .context("Failed to get OS downloads")
        }
    };

    let platform_downloads = os_downloads
        .platforms
        .get(&arch)
        .with_context(|| format!("Unsupported architecture {} for OS {}", arch, os))?;

    let download_url = match download_type {
        DownloadType::Gui => platform_downloads
            .gui
            .as_ref()
            .context("GUI download not available for this platform")?,
        DownloadType::Cli => platform_downloads
            .cli
            .as_ref()
            .context("CLI download not available for this platform")?,
        DownloadType::Rpm => platform_downloads
            .rpm
            .as_ref()
            .context("RPM download not available for this platform")?,
        DownloadType::Deb => platform_downloads
            .deb
            .as_ref()
            .context("DEB download not available for this platform")?,
    };

    Ok(download_url.clone())
}

/// Generic function to run a script with given template and replacements
fn run_script(
    script_name: &str,
    script_template: &str,
    replacements: Vec<(&str, String)>,
    is_service: bool,
) -> Result<()> {
    use anyhow::Context;
    use tracing::{debug, info};

    // Use cache directory for updates instead of temp
    let cache_dir = get_cache_dir("updates")?;
    std::fs::create_dir_all(&cache_dir).context("Failed to create updates cache directory")?;

    let timestamp = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    #[cfg(windows)]
    let script_path = cache_dir.join(format!("{}_{}.bat", script_name, timestamp));
    #[cfg(unix)]
    let script_path = cache_dir.join(format!("{}_{}.sh", script_name, timestamp));

    debug!("Creating script: {}", script_path.display());

    // Apply all replacements to the template
    let mut script_content = script_template.to_string();
    for (placeholder, value) in replacements {
        script_content = script_content.replace(placeholder, &value);
    }

    std::fs::write(&script_path, script_content).context("Failed to create script")?;

    info!("Script created at: {}", script_path.display());

    // Always make script executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms)?;
    }

    // Execute the script
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(&["/C", &script_path.to_string_lossy()])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .context("Failed to execute script")?;
    }

    #[cfg(target_os = "linux")]
    {
        if is_service {
            // Use systemd one-shot service for service updates
            let service_name = format!("cloudpub-upgrade-{}.service", timestamp);
            let service_path = format!("/etc/systemd/system/{}", service_name);

            debug!("Creating systemd one-shot service: {}", service_name);

            let service_content = SYSTEMD_ONESHOT_TEMPLATE
                .replace("{TIMESTAMP}", &timestamp)
                .replace("{SCRIPT_PATH}", &script_path.display().to_string());

            std::fs::write(&service_path, service_content)
                .context("Failed to create systemd one-shot service")?;

            std::process::Command::new("systemctl")
                .arg("daemon-reload")
                .output()
                .context("Failed to reload systemd")?;

            std::process::Command::new("systemctl")
                .arg("start")
                .arg(&service_name)
                .spawn()
                .context("Failed to start upgrade service")?;

            info!(
                "Upgrade scheduled via systemd one-shot service: {}",
                service_name
            );
        } else {
            // Use nohup for regular executable updates
            debug!("Executing script with nohup: {}", script_path.display());

            std::process::Command::new("nohup")
                .arg("/bin/bash")
                .arg(&script_path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("Failed to spawn script with nohup")?;

            info!("Script spawned with nohup");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if is_service {
            // Use launchd one-shot job for service updates
            let plist_name = format!("com.cloudpub.upgrade.{}.plist", timestamp);
            let plist_path = format!("/Library/LaunchDaemons/{}", plist_name);

            debug!("Creating launchd one-shot job: {}", plist_name);

            let plist_content = LAUNCHD_ONESHOT_TEMPLATE
                .replace("{TIMESTAMP}", &timestamp)
                .replace("{SCRIPT_PATH}", &script_path.display().to_string());

            std::fs::write(&plist_path, plist_content)
                .context("Failed to create launchd one-shot plist")?;

            std::process::Command::new("chmod")
                .args(["644", &plist_path])
                .output()
                .context("Failed to set plist permissions")?;

            std::process::Command::new("launchctl")
                .args(["load", &plist_path])
                .spawn()
                .context("Failed to load upgrade job")?;

            info!("Upgrade scheduled via launchd one-shot job: {}", plist_name);
        } else {
            // Use nohup for regular executable updates
            debug!("Executing script with nohup: {}", script_path.display());

            std::process::Command::new("nohup")
                .arg("/bin/bash")
                .arg(&script_path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("Failed to spawn script with nohup")?;

            info!("Script spawned with nohup");
        }
    }

    info!("Script execution initiated, exiting current process");
    std::process::exit(0);
}

/// Run an upgrade script with logging support
fn run_upgrade_script(
    script_template: &str,
    new_exe_path: &std::path::Path,
    current_args: Vec<String>,
    is_service: bool,
) -> Result<()> {
    use tracing::debug;

    let current_exe = std::env::current_exe()?;
    let cache_dir = get_cache_dir("updates")?;
    std::fs::create_dir_all(&cache_dir).context("Failed to create updates cache directory")?;

    let timestamp = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    let log_file = cache_dir.join(format!("cloudpub_update_{}.log", timestamp));

    debug!("Starting update process");
    debug!("Current executable: {}", current_exe.display());
    debug!("New executable: {}", new_exe_path.display());
    debug!("Current args: {:?}", current_args);
    debug!("Log file location: {}", log_file.display());
    debug!("Is service: {}", is_service);

    // Extract config path for service mode
    // Note: The actual flag used is --conf, not --config
    let config_path = if is_service {
        current_args
            .windows(2)
            .find(|w| w[0] == "--conf" || w[0] == "--config")
            .and_then(|w| w.get(1))
            .map(|s| s.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    // Always provide all replacements - unused ones will be ignored
    let replacements = vec![
        ("{LOG_FILE}", log_file.display().to_string()),
        ("{NEW_EXE}", new_exe_path.display().to_string()),
        ("{CURRENT_EXE}", current_exe.display().to_string()),
        ("{ARGS}", current_args.join(" ")),
        ("{CONFIG_PATH}", config_path),
        ("{SERVICE_ARGS}", current_args.join(" ")),
    ];

    run_script("cloudpub_update", script_template, replacements, is_service)
}

#[cfg(windows)]
pub fn install_msi_package(msi_path: &std::path::Path) -> Result<()> {
    let current_exe = std::env::current_exe()?;

    run_script(
        "cloudpub_install_msi",
        WINDOWS_MSI_INSTALL_SCRIPT,
        vec![
            ("{MSI_PATH}", msi_path.display().to_string()),
            ("{CURRENT_EXE}", current_exe.display().to_string()),
        ],
        false,
    )
}

#[cfg(windows)]
pub fn replace_and_restart_windows(
    new_exe_path: &std::path::Path,
    current_args: Vec<String>,
) -> Result<()> {
    let is_service = current_args.contains(&"--run-as-service".to_string());
    let script_template = if is_service {
        WINDOWS_SERVICE_UPDATE_SCRIPT
    } else {
        WINDOWS_CLIENT_UPDATE_SCRIPT
    };
    run_upgrade_script(script_template, new_exe_path, current_args, is_service)
}

#[cfg(not(windows))]
pub fn replace_and_restart_unix(
    new_exe_path: &std::path::Path,
    current_args: Vec<String>,
) -> Result<()> {
    let is_service = current_args.contains(&"--run-as-service".to_string());
    let script_template = if is_service {
        UNIX_SERVICE_UPDATE_SCRIPT
    } else {
        UNIX_CLIENT_UPDATE_SCRIPT
    };
    run_upgrade_script(script_template, new_exe_path, current_args, is_service)
}

pub fn apply_update_and_restart(new_exe_path: &std::path::Path) -> Result<()> {
    // Get current command line arguments (skip the program name)
    let args: Vec<String> = std::env::args().skip(1).collect();

    eprintln!("{}", crate::t!("applying-update"));

    #[cfg(windows)]
    replace_and_restart_windows(new_exe_path, args)?;

    #[cfg(not(windows))]
    replace_and_restart_unix(new_exe_path, args)?;

    Ok(())
}

#[cfg(target_os = "macos")]
async fn install_dmg_package(package_path: &std::path::Path) -> Result<()> {
    eprintln!("Installing DMG package...");

    let current_exe = std::env::current_exe()?;
    let current_args: Vec<String> = std::env::args().skip(1).collect();

    run_script(
        "cloudpub_install_dmg",
        MACOS_DMG_INSTALL_SCRIPT,
        vec![
            ("{PACKAGE_PATH}", package_path.display().to_string()),
            ("{CURRENT_EXE}", current_exe.display().to_string()),
            ("{CURRENT_ARGS}", current_args.join(" ")),
        ],
        false,
    )
}

#[cfg(target_os = "linux")]
async fn install_deb_package(package_path: &std::path::Path) -> Result<()> {
    eprintln!("Installing DEB package...");

    let current_exe = std::env::current_exe()?;
    let current_args: Vec<String> = std::env::args().skip(1).collect();

    run_script(
        "cloudpub_install_deb",
        LINUX_DEB_INSTALL_SCRIPT,
        vec![
            ("{PACKAGE_PATH}", package_path.display().to_string()),
            ("{CURRENT_EXE}", current_exe.display().to_string()),
            ("{CURRENT_ARGS}", current_args.join(" ")),
        ],
        false,
    )
}

#[cfg(target_os = "linux")]
async fn install_rpm_package(package_path: &std::path::Path) -> Result<()> {
    eprintln!("Installing RPM package...");

    let current_exe = std::env::current_exe()?;
    let current_args: Vec<String> = std::env::args().skip(1).collect();

    run_script(
        "cloudpub_install_rpm",
        LINUX_RPM_INSTALL_SCRIPT,
        vec![
            ("{PACKAGE_PATH}", package_path.display().to_string()),
            ("{CURRENT_EXE}", current_exe.display().to_string()),
            ("{CURRENT_ARGS}", current_args.join(" ")),
        ],
        false,
    )
}

async fn handle_cli_update(
    filename: &str,
    output_path: &std::path::Path,
    cache_dir: &std::path::Path,
    command_rx: &mut mpsc::Receiver<Message>,
    result_tx: &mpsc::Sender<Message>,
) -> Result<()> {
    let unpack_message = "Unpacking update".to_string();

    if filename.ends_with(".zip") {
        crate::shell::unzip(&unpack_message, output_path, cache_dir, 0, result_tx)
            .await
            .context("Failed to unpack zip update")?;
    } else if filename.ends_with(".tar.gz") {
        let tar_args = vec![
            "-xzf".to_string(),
            output_path.to_string_lossy().to_string(),
        ];

        let total_files = 1;
        let progress = Some((unpack_message, result_tx.clone(), total_files));

        crate::shell::execute(
            std::path::PathBuf::from("tar"),
            tar_args,
            Some(cache_dir.to_path_buf()),
            std::collections::HashMap::new(),
            progress,
            command_rx,
        )
        .await
        .context("Failed to unpack tar.gz update")?;
    } else {
        return Err(anyhow::anyhow!("Unknown file format: {}", filename));
    }

    eprintln!(
        "{}",
        crate::t!("update-unpacked", "path" => cache_dir.display().to_string())
    );

    #[cfg(not(windows))]
    let new_exe_path = cache_dir.join("clo");
    #[cfg(windows)]
    let new_exe_path = cache_dir.join("clo.exe");

    apply_update_and_restart(&new_exe_path)?;
    Ok(())
}

pub async fn handle_upgrade_download(
    version: &str,
    gui: bool,
    config: Arc<RwLock<ClientConfig>>,
    command_rx: &mut mpsc::Receiver<Message>,
    result_tx: &mpsc::Sender<Message>,
) -> Result<(std::path::PathBuf, DownloadType, String)> {
    // Delete old update cache directory
    let cache_dir = get_cache_dir("updates").unwrap();
    std::fs::remove_dir_all(&cache_dir).ok();

    // Download the upgrade
    let cache_dir = get_cache_dir("updates").unwrap();

    let download_type = if gui {
        #[cfg(target_os = "linux")]
        {
            // Try to detect package type and use appropriate installer
            match get_linux_package_type() {
                Ok(pkg_type) if pkg_type == "deb" => DownloadType::Deb,
                Ok(pkg_type) if pkg_type == "rpm" => DownloadType::Rpm,
                _ => DownloadType::Cli, // Fallback to CLI
            }
        }
        #[cfg(not(target_os = "linux"))]
        DownloadType::Gui
    } else {
        DownloadType::Cli
    };

    // Use base URL from config server field
    let base_url = config.read().server.to_string();
    let download_url =
        get_download_url(&base_url, &download_type, version, cloudpub_common::BRANCH)?;

    // Extract file extension from download URL
    let mut file_extension = std::path::Path::new(&download_url)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext))
        .unwrap_or_else(|| ".tar.gz".to_string());

    if file_extension == ".gz" {
        // If the file extension is .gz, we assume it's a tar.gz file
        file_extension = ".tar.gz".to_string();
    }

    let filename = format!("cloudpub-{}{}", version, file_extension);
    let output_path = cache_dir.join(&filename);

    let message = crate::t!("downloading-update", "path" => output_path.display().to_string());

    crate::shell::download(
        &message,
        config.clone(),
        &download_url,
        &output_path,
        command_rx,
        result_tx,
    )
    .await
    .with_context(|| format!("Failed to download update ({})", download_url))?;

    eprintln!(
        "{}",
        crate::t!("update-downloaded", "path" => output_path.display().to_string())
    );

    Ok((output_path, download_type, filename))
}

pub async fn handle_upgrade_install(
    output_path: std::path::PathBuf,
    download_type: DownloadType,
    filename: String,
    command_rx: &mut mpsc::Receiver<Message>,
    result_tx: &mpsc::Sender<Message>,
) -> Result<()> {
    let cache_dir = output_path.parent().unwrap();
    // change the current directory to the cache directory
    std::env::set_current_dir(cache_dir)
        .context("Failed to change current directory to cache directory")?;

    // Handle different package types
    match download_type {
        DownloadType::Gui => {
            #[cfg(target_os = "macos")]
            {
                if filename.ends_with(".dmg") {
                    install_dmg_package(&output_path).await?;
                } else {
                    return Err(anyhow::anyhow!("Unsupported GUI package format"));
                }
            }
            #[cfg(target_os = "windows")]
            {
                if filename.ends_with(".msi") {
                    install_msi_package(&output_path)?;
                } else {
                    return Err(anyhow::anyhow!("Unsupported GUI package format"));
                }
            }
            #[cfg(target_os = "linux")]
            {
                return Err(anyhow::anyhow!("GUI packages not supported on Linux"));
            }
        }
        DownloadType::Deb => {
            #[cfg(target_os = "linux")]
            {
                install_deb_package(&output_path).await?;
            }
            #[cfg(not(target_os = "linux"))]
            {
                return Err(anyhow::anyhow!("DEB packages only supported on Linux"));
            }
        }
        DownloadType::Rpm => {
            #[cfg(target_os = "linux")]
            {
                install_rpm_package(&output_path).await?;
            }
            #[cfg(not(target_os = "linux"))]
            {
                return Err(anyhow::anyhow!("RPM packages only supported on Linux"));
            }
        }
        _ => {
            // Handle CLI updates (tar.gz/zip extraction and replacement)
            handle_cli_update(&filename, &output_path, cache_dir, command_rx, result_tx).await?;
        }
    }

    Ok(())
}

pub async fn handle_upgrade_available(
    version: &str,
    config: Arc<RwLock<ClientConfig>>,
    gui: bool,
    command_rx: &mut mpsc::Receiver<Message>,
    result_tx: &mpsc::Sender<Message>,
) -> Result<()> {
    let (output_path, download_type, filename) =
        handle_upgrade_download(version, gui, config, command_rx, result_tx).await?;

    handle_upgrade_install(output_path, download_type, filename, command_rx, result_tx).await?;

    Ok(())
}
