use crate::config::{ClientConfig, EnvConfig};
use crate::shell::{download, get_cache_dir, unzip, SubProcess, DOWNLOAD_SUBDIR};
use crate::t;
use anyhow::{Context, Result};
use common::protocol::message::Message;
use common::protocol::ServerEndpoint;
use common::utils::find_free_tcp_port;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

#[cfg(unix)]
fn kill_by_pid_file(pid_file: &std::path::Path) {
    use std::fs;
    if !pid_file.exists() {
        return;
    }

    let pid_str = match fs::read_to_string(pid_file) {
        Ok(content) => content,
        Err(e) => {
            tracing::warn!("Failed to read PID file {}: {}", pid_file.display(), e);
            return;
        }
    };

    let pid_num: i32 = match pid_str.trim().parse() {
        Ok(pid) => pid,
        Err(e) => {
            tracing::warn!(
                "Failed to parse PID from file {}: {}",
                pid_file.display(),
                e
            );
            return;
        }
    };

    let pid = Pid::from_raw(pid_num);

    // Try to kill the process using nix crate
    match signal::kill(pid, Signal::SIGTERM) {
        Ok(()) => {
            tracing::info!("Successfully sent SIGTERM to process {}", pid_num);
        }
        Err(nix::errno::Errno::ESRCH) => {
            // Process doesn't exist, that's fine
            tracing::debug!("Process {} not found (already dead)", pid_num);
        }
        Err(e) => {
            tracing::warn!("Failed to kill process {}: {}", pid_num, e);
        }
    }

    // Remove the PID file
    if let Err(e) = fs::remove_file(pid_file) {
        tracing::warn!("Failed to remove PID file {}: {}", pid_file.display(), e);
    }
}

#[cfg(target_os = "windows")]
pub const HTTPD_EXE: &str = "httpd.exe";
#[cfg(unix)]
pub const HTTPD_EXE: &str = "httpd";

pub async fn setup_httpd(
    config: &Arc<RwLock<ClientConfig>>,
    command_rx: &mut mpsc::Receiver<Message>,
    result_tx: &mpsc::Sender<Message>,
    env: EnvConfig,
) -> Result<()> {
    let cache_dir = get_cache_dir(DOWNLOAD_SUBDIR)?;
    let httpd_dir = get_cache_dir(&env.httpd_dir)?;

    let mut touch = httpd_dir.clone();
    touch.push("installed.txt");

    if touch.exists() {
        return Ok(());
    }

    let mut httpd = cache_dir.clone();
    httpd.push(env.httpd.clone());

    download(
        &crate::t!("downloading-webserver"),
        config.clone(),
        format!("{}download/{}", config.read().server, env.httpd).as_str(),
        &httpd,
        command_rx,
        result_tx,
    )
    .await
    .context(crate::t!("error-downloading-webserver"))?;

    unzip(
        &crate::t!("unpacking-webserver"),
        &httpd,
        &httpd_dir,
        1,
        result_tx,
    )
    .await
    .context(crate::t!("error-unpacking-webserver"))?;

    #[cfg(target_os = "windows")]
    {
        use crate::shell::execute;
        let mut redist = cache_dir.clone();
        redist.push(env.redist.clone());

        download(
            &crate::t!("downloading-vcpp"),
            config.clone(),
            format!("{}download/{}", config.read().server, env.redist).as_str(),
            &redist,
            command_rx,
            result_tx,
        )
        .await
        .context(crate::t!("error-downloading-vcpp"))?;

        if let Err(err) = execute(
            redist,
            vec![
                "/install".to_string(),
                "/quiet".to_string(),
                "/norestart".to_string(),
            ],
            None,
            Default::default(),
            Some((crate::t!("installing-vcpp"), result_tx.clone(), 2)),
            command_rx,
        )
        .await
        {
            // Non fatal error, probably components already installed
            tracing::warn!("{}: {:?}", crate::t!("error-installing-vcpp"), err);
        }
    }
    // Set exec mode for httpd_exe
    #[cfg(unix)]
    {
        let httpd_exe = httpd_dir.join("bin").join(HTTPD_EXE);
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&httpd_exe, std::fs::Permissions::from_mode(0o755))
            .context(crate::t!("error-setting-permissions"))?;
    }

    // Touch file to mark success
    std::fs::write(touch, "Delete to reinstall").context(crate::t!("error-creating-marker"))?;

    Ok(())
}

pub async fn start_httpd(
    endpoint: &ServerEndpoint,
    config_template: &str,
    config_subdir: &str,
    publish_dir: &str,
    env: EnvConfig,
    result_tx: mpsc::Sender<Message>,
) -> Result<SubProcess> {
    let httpd_dir = get_cache_dir(&env.httpd_dir)?;
    let configs_dir = get_cache_dir(config_subdir)?;

    let mut httpd_cfg = configs_dir.clone();
    httpd_cfg.push(format!("{}.conf", endpoint.guid));

    let mut pid_file = configs_dir.clone();
    pid_file.push(format!("{}.pid", endpoint.guid));

    let mut lock_file = configs_dir.clone();
    lock_file.push(format!("{}.lock", endpoint.guid));

    // Kill any existing httpd process before starting a new one
    #[cfg(unix)]
    kill_by_pid_file(&pid_file);

    let port = find_free_tcp_port()
        .await
        .context(t!("error-finding-free-port"))?;

    let httpd_config = config_template.replace("[[PUBLISH_DIR]]", publish_dir);
    let httpd_config = httpd_config.replace("[[SRVROOT]]", httpd_dir.to_str().unwrap());
    let httpd_config = httpd_config.replace("[[PORT]]", &port.to_string());
    let httpd_config = httpd_config.replace("[[PID_FILE]]", pid_file.to_str().unwrap());
    let httpd_config = httpd_config.replace("[[LOCK_FILE]]", lock_file.to_str().unwrap());

    #[cfg(unix)]
    let httpd_config = httpd_config.replace("[[IS_LINUX]]", "");

    #[cfg(not(unix))]
    let httpd_config = httpd_config.replace("[[IS_LINUX]]", "#");

    std::fs::write(&httpd_cfg, httpd_config).context(crate::t!("error-writing-httpd-conf"))?;

    let httpd_cfg = httpd_cfg.to_str().unwrap().to_string();
    let httpd_exe = httpd_dir.join("bin").join(HTTPD_EXE);

    #[allow(unused_mut)]
    let mut envs = HashMap::<String, String>::new();

    #[cfg(target_os = "macos")]
    envs.insert(
        "DYLD_LIBRARY_PATH".to_string(),
        httpd_dir.join("lib").to_str().unwrap().to_string(),
    );

    #[cfg(target_os = "linux")]
    envs.insert(
        "LD_LIBRARY_PATH".to_string(),
        httpd_dir.join("lib").to_str().unwrap().to_string(),
    );

    let server = SubProcess::new(
        httpd_exe,
        vec![
            #[cfg(not(target_os = "windows"))]
            "-X".to_string(),
            "-f".to_string(),
            httpd_cfg,
        ],
        None,
        envs,
        result_tx,
        port,
    );
    Ok(server)
}
