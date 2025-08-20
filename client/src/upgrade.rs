use crate::config::ClientConfig;
use crate::shell::get_cache_dir;
use anyhow::{Context, Result};
use common::protocol::message::Message;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

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

#[cfg(windows)]
pub fn install_msi_package(msi_path: &std::path::Path) -> Result<()> {
    use std::os::windows::process::CommandExt;

    let temp_dir = std::env::temp_dir();
    let batch_script = temp_dir.join("cloudpub_install_msi.bat");
    let current_exe = std::env::current_exe()?;

    let script_content = format!(
        r#"@echo off
timeout /t 3 /nobreak >nul
echo Installing MSI package...
echo Requesting administrator privileges...
powershell -Command "Start-Process msiexec -ArgumentList '/i', '{}', '/quiet', '/norestart' -Verb RunAs -Wait"
if errorlevel 1 (
    echo Failed to install MSI package
    exit /b 1
)
echo Installation completed successfully
del "%~f0"
start "" "{}"
"#,
        msi_path.display(),
        current_exe.display()
    );

    std::fs::write(&batch_script, script_content)
        .context("Failed to create MSI install batch script")?;

    // Execute the batch script and exit current process
    std::process::Command::new("cmd")
        .args(&["/C", &batch_script.to_string_lossy()])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .context("Failed to execute MSI install batch script")?;

    std::process::exit(0);
}

#[cfg(windows)]
pub fn replace_and_restart_windows(
    new_exe_path: &std::path::Path,
    current_args: Vec<String>,
) -> Result<()> {
    use std::os::windows::process::CommandExt;

    let current_exe = std::env::current_exe()?;
    let temp_dir = std::env::temp_dir();
    let batch_script = temp_dir.join("cloudpub_update.bat");

    let script_content = format!(
        r#"@echo off
timeout /t 3 /nobreak >nul
move /y "{}" "{}"
if errorlevel 1 (
    echo Failed to replace executable
    exit /b 1
)
start "" "{}" {}
del "%~f0"
"#,
        new_exe_path.display(),
        current_exe.display(),
        current_exe.display(),
        current_args.join(" ")
    );

    std::fs::write(&batch_script, script_content)
        .context("Failed to create update batch script")?;

    // Execute the batch script and exit current process
    std::process::Command::new("cmd")
        .args(&["/C", &batch_script.to_string_lossy()])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .context("Failed to execute update batch script")?;

    std::process::exit(0);
}

#[cfg(not(windows))]
pub fn replace_and_restart_unix(
    new_exe_path: &std::path::Path,
    current_args: Vec<String>,
) -> Result<()> {
    let current_exe = std::env::current_exe()?;
    let backup_exe = current_exe.with_extension("old");

    // Move current executable to backup (this works even if it's running)
    std::fs::rename(&current_exe, &backup_exe).context("Failed to backup current executable")?;

    // Move new executable to current location (atomic operation)
    std::fs::rename(new_exe_path, &current_exe).context("Failed to move new executable")?;

    // Set executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&current_exe)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&current_exe, perms)?;
    }

    // Clean up backup file (optional, ignore errors)
    std::fs::remove_file(&backup_exe).ok();

    // Restart the application
    std::process::Command::new(&current_exe)
        .args(current_args)
        .spawn()
        .context("Failed to restart application")?;

    std::process::exit(0);
}

pub fn apply_update_and_restart(new_exe_path: &std::path::Path) -> Result<()> {
    // Get current command line arguments (skip the program name)
    let args: Vec<String> = vec!["run".to_string()];

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

    // Get current executable and args for restart
    let current_exe = std::env::current_exe()?;
    let current_args: Vec<String> = std::env::args().skip(1).collect();
    let temp_dir = std::env::temp_dir();
    let install_script = temp_dir.join("cloudpub_install_dmg.sh");

    let script_content = format!(
        r#"#!/bin/bash
set -e

PACKAGE_PATH="{}"
CURRENT_EXE="{}"
CURRENT_ARGS="{}"

echo "Installing DMG package..."

# Mount the DMG
MOUNT_OUTPUT=$(hdiutil attach -nobrowse -quiet "$PACKAGE_PATH")
if [ $? -ne 0 ]; then
    echo "Failed to mount DMG package"
    exit 1
fi

# Extract mount point
MOUNT_POINT=$(echo "$MOUNT_OUTPUT" | grep "/Volumes/" | awk '{{print $NF}}')
if [ -z "$MOUNT_POINT" ]; then
    echo "Could not find mount point"
    exit 1
fi

# Find .app bundle
APP_PATH=$(find "$MOUNT_POINT" -name "*.app" -type d | head -n 1)
if [ -z "$APP_PATH" ]; then
    hdiutil detach "$MOUNT_POINT" -quiet
    echo "No .app bundle found in DMG"
    exit 1
fi

APP_NAME=$(basename "$APP_PATH")
DESTINATION="/Applications/$APP_NAME"

# Remove existing app if it exists
if [ -d "$DESTINATION" ]; then
    rm -rf "$DESTINATION"
fi

# Copy app to Applications
cp -R "$APP_PATH" "/Applications/"
if [ $? -ne 0 ]; then
    hdiutil detach "$MOUNT_POINT" -quiet
    echo "Failed to copy application"
    exit 1
fi

# Unmount DMG
hdiutil detach "$MOUNT_POINT" -quiet

echo "DMG package installed successfully"

# Launch new version
APP_NAME_NO_EXT=$(basename "$APP_NAME" .app)
open -a "$APP_NAME_NO_EXT"
echo "Launched new application version"

# Clean up script
rm -f "$0"
"#,
        package_path.display(),
        current_exe.display(),
        current_args.join(" ")
    );

    std::fs::write(&install_script, script_content)?;

    // Make script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&install_script)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&install_script, perms)?;
    }

    // Execute install script in background
    std::process::Command::new("nohup")
        .arg(&install_script)
        .spawn()?;

    std::process::exit(0);
}

#[cfg(target_os = "linux")]
async fn install_deb_package(package_path: &std::path::Path) -> Result<()> {
    eprintln!("Installing DEB package...");

    // Get current executable and args for restart
    let current_exe = std::env::current_exe()?;
    let current_args: Vec<String> = std::env::args().skip(1).collect();
    let temp_dir = std::env::temp_dir();
    let install_script = temp_dir.join("cloudpub_install_deb.sh");

    let script_content = format!(
        r#"#!/bin/bash
set -e

PACKAGE_PATH="{}"
CURRENT_EXE="{}"
CURRENT_ARGS="{}"

echo "Installing DEB package..."

# Check if running as root, if not use pkexec
if [ "$EUID" -ne 0 ]; then
    echo "Root privileges required for package installation"
    pkexec dpkg -i "$PACKAGE_PATH"
else
    dpkg -i "$PACKAGE_PATH"
fi

if [ $? -ne 0 ]; then
    echo "Failed to install DEB package"
    exit 1
fi

echo "DEB package installed successfully"

# Wait a moment for the process to exit
sleep 3

# Restart the application
exec "$CURRENT_EXE" $CURRENT_ARGS
"#,
        package_path.display(),
        current_exe.display(),
        current_args.join(" ")
    );

    std::fs::write(&install_script, script_content)?;

    // Make script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&install_script)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&install_script, perms)?;
    }

    // Execute install script in background
    std::process::Command::new("nohup")
        .arg(&install_script)
        .spawn()?;

    std::process::exit(0);
}

#[cfg(target_os = "linux")]
async fn install_rpm_package(package_path: &std::path::Path) -> Result<()> {
    eprintln!("Installing RPM package...");

    // Get current executable and args for restart
    let current_exe = std::env::current_exe()?;
    let current_args: Vec<String> = std::env::args().skip(1).collect();
    let temp_dir = std::env::temp_dir();
    let install_script = temp_dir.join("cloudpub_install_rpm.sh");

    let script_content = format!(
        r#"#!/bin/bash
set -e

PACKAGE_PATH="{}"
CURRENT_EXE="{}"
CURRENT_ARGS="{}"

echo "Installing RPM package..."

# Check if running as root, if not use pkexec
if [ "$EUID" -ne 0 ]; then
    echo "Root privileges required for package installation"
    pkexec rpm -Uvh "$PACKAGE_PATH"
else
    rpm -Uvh "$PACKAGE_PATH"
fi

if [ $? -ne 0 ]; then
    echo "Failed to install RPM package"
    exit 1
fi

echo "RPM package installed successfully"

# Wait a moment for the process to exit
sleep 3

# Restart the application
exec "$CURRENT_EXE" $CURRENT_ARGS
"#,
        package_path.display(),
        current_exe.display(),
        current_args.join(" ")
    );

    std::fs::write(&install_script, script_content)?;

    // Make script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&install_script)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&install_script, perms)?;
    }

    // Execute install script in background
    std::process::Command::new("nohup")
        .arg(&install_script)
        .spawn()?;

    std::process::exit(0);
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
    let download_url = get_download_url(&base_url, &download_type, version, "stable")?;

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
    .context("Failed to download update")?;

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
