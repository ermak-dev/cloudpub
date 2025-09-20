use crate::config::ClientConfig;
use anyhow::{bail, Context, Result};
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use cloudpub_common::protocol::message::Message;
use cloudpub_common::protocol::{Break, ErrorInfo, ErrorKind, ProgressInfo};
use cloudpub_common::transport::rustls::load_roots;
use dirs::cache_dir;
use futures::stream::StreamExt;
use parking_lot::RwLock;
use reqwest::{Certificate, ClientBuilder};
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use walkdir::WalkDir;
use zip::read::ZipArchive;

pub const DOWNLOAD_SUBDIR: &str = "download";

pub struct SubProcess {
    shutdown_tx: mpsc::Sender<Message>,
    command: PathBuf,
    args: Vec<String>,
    pub port: u16,
    canceled: Arc<AtomicBool>,
    pub result: Arc<RwLock<Result<()>>>,
}

impl SubProcess {
    pub fn new(
        command: PathBuf,
        args: Vec<String>,
        chdir: Option<PathBuf>,
        envs: HashMap<String, String>,
        result_tx: mpsc::Sender<Message>,
        port: u16,
    ) -> Self {
        let canceled = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        let command2 = command.clone();
        let args2 = args.clone();
        let canceled2 = canceled.clone();
        let result = Arc::new(RwLock::new(Ok(())));
        let result_clone = result.clone();
        tokio::spawn(async move {
            if let Err(err) = execute(command2, args2, chdir, envs, None, &mut shutdown_rx).await {
                if !canceled2.load(Ordering::Relaxed) {
                    error!("Failed to execute command: {:?}", err);
                    result_tx
                        .send(Message::Error(ErrorInfo {
                            kind: ErrorKind::ExecuteFailed.into(),
                            message: format!("Ошибка запуска {}", err),
                            guid: String::new(),
                        }))
                        .await
                        .ok();
                }
                *result_clone.write() = Err(err);
            }
        });
        Self {
            shutdown_tx,
            port,
            command,
            args,
            canceled,
            result,
        }
    }

    pub fn stop(&mut self) {
        self.canceled.store(true, Ordering::Relaxed);
        self.shutdown_tx
            .try_send(Message::Break(Break {
                ..Default::default()
            }))
            .ok();
    }
}

impl Drop for SubProcess {
    fn drop(&mut self) {
        info!("Drop subprocess: {:?} {:?}", self.command, self.args);
        self.stop();
    }
}

pub async fn send_progress(
    message: &str,
    template: &str,
    total: u64,
    current: u64,
    progress_tx: &mpsc::Sender<Message>,
) {
    let progress = ProgressInfo {
        message: message.to_string(),
        template: template.to_string(),
        total: total as u32,
        current: current as u32,
        ..Default::default()
    };
    progress_tx.send(Message::Progress(progress)).await.ok();
}

pub async fn execute(
    command: PathBuf,
    args: Vec<String>,
    chdir: Option<PathBuf>,
    envs: HashMap<String, String>,
    progress: Option<(String, mpsc::Sender<Message>, u64)>,
    shutdown_rx: &mut mpsc::Receiver<Message>,
) -> Result<()> {
    let argv = format!("{} {}", command.to_str().unwrap(), args.join(" "));
    info!("Executing command: {}", argv);

    info!("Environment: {:?}", envs);

    let template = crate::t!("progress-files-eta");
    let chdir = chdir.as_deref().unwrap_or(Path::new("."));

    if let Some((message, tx, total)) = progress.as_ref() {
        send_progress(message, &template, *total, 0, tx).await;
        send_progress(message, &template, *total, 1, tx).await;
    }

    #[cfg(windows)]
    let mut child = Command::new(command.clone())
        .args(args.clone())
        .kill_on_drop(true)
        .current_dir(&chdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .envs(envs)
        .spawn()
        .context(format!(
            "Failed to execute command: {:?} {:?}",
            command, args
        ))?;

    #[cfg(not(windows))]
    let mut child = Command::new(command.clone())
        .args(args.clone())
        .kill_on_drop(true)
        .current_dir(chdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(envs)
        .spawn()
        .context(format!(
            "Failed to execute command: {:?} {:?}",
            command, args
        ))?;

    let stdout = child.stdout.take().context("Failed to get stdout")?;
    let stderr = child.stderr.take().context("Failed to get stderr")?;

    let stdout_reader = BufReader::new(stdout).lines();
    let stderr_reader = BufReader::new(stderr).lines();

    let progress1 = progress.clone();
    let template_clone = template.clone();
    tokio::spawn(async move {
        tokio::pin!(stdout_reader);
        tokio::pin!(stderr_reader);
        let mut current = 0;
        loop {
            let progress = progress1.clone();
            tokio::select! {
                line = stdout_reader.next_line() => match line {
                    Ok(Some(line)) => {
                        info!("STDOUT: {}", line);
                        current += 1;
                        if let Some((message, tx, total)) = progress.as_ref() {
                            send_progress(message, &template_clone, *total, current, tx).await;
                        }
                    }
                    Err(e) => {
                        bail!("Error reading stdout: {}", e);
                    },
                    Ok(None) => {
                        info!("STDOUT EOF");
                        break;
                    }
                },
                line = stderr_reader.next_line() => match line {
                    Ok(Some(line)) => {
                        warn!("STDERR: {}", line);
                        current += 1;
                        if let Some((message, tx, total)) = progress.as_ref() {
                            send_progress(message, &template_clone, *total, current, tx).await;
                        }
                    },
                    Err(e) => {
                        bail!("Error reading stderr: {}", e);
                    },
                    Ok(None) => {
                        info!("STDERR EOF");
                        break;
                    }
                },
            }
        }
        Ok(())
    });

    tokio::select! {
        status = child.wait() => {
            let status = status.context("Failed to wait on child")?;
            if !status.success() {
                if let Some((message, tx, total)) = progress.as_ref() {
                    send_progress(message, &template, *total, *total, tx).await;
                }
                bail!("{}: exit code {}", command.file_name().unwrap().to_string_lossy(), status.code().unwrap_or(-1));
            }
        }

        cmd = shutdown_rx.recv() => match cmd {
            Some(Message::Stop(_)) | Some(Message::Break(_)) => {
                info!("Received break command, killing child process: {}", argv);
                child.kill().await.ok();
            }
            None => {
                info!("Command channel closed, killing child process: {}", argv);
                child.kill().await.ok();
            }
            _ => {}
        }
    }

    if let Some((message, tx, total)) = progress.as_ref() {
        send_progress(message, &template, *total, *total, tx).await;
    }
    info!("Command executed successfully");

    Ok(())
}

pub async fn unzip(
    message: &str,
    zip_file_path: &Path,
    extract_dir: &Path,
    skip: usize,
    result_tx: &mpsc::Sender<Message>,
) -> Result<()> {
    info!("Unzipping {:?} to {:?}", zip_file_path, extract_dir);
    let file = File::open(zip_file_path)?;
    let mut archive = ZipArchive::new(file)?;

    std::fs::create_dir_all(extract_dir)
        .context(format!("Failed to create dir '{:?}'", extract_dir))?;

    let template = crate::t!("progress-files-eta");

    let mut progress = ProgressInfo {
        message: message.to_string(),
        template,
        total: archive.len() as u32,
        current: 0,
        ..Default::default()
    };

    result_tx
        .send(Message::Progress(progress.clone()))
        .await
        .ok();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .context("Failed to get file from archive")?;
        let file_name = Path::new(file.name())
            .components()
            .skip(skip)
            .collect::<PathBuf>();
        let target_path = Path::new(extract_dir).join(file_name);
        if target_path == extract_dir {
            continue;
        }
        info!("Extracting {:?}", target_path);
        if file.is_dir() {
            std::fs::create_dir_all(target_path.clone())
                .context(format!("unzip failed to create dir '{:?}'", target_path))?;
        } else {
            let mut output_file = File::create(target_path.clone())
                .context(format!("unzip failed to create file '{:?}'", target_path))?;
            io::copy(&mut file, &mut output_file).context("unzip failed to copy file")?;
        }

        progress.current = (i + 1) as u32;
        if progress.current % 100 == 0 {
            result_tx
                .send(Message::Progress(progress.clone()))
                .await
                .ok();
        }
    }

    progress.current = progress.total;
    result_tx
        .send(Message::Progress(progress.clone()))
        .await
        .ok();
    Ok(())
}

pub async fn download(
    message: &str,
    config: Arc<RwLock<ClientConfig>>,
    url: &str,
    path: &Path,
    command_rx: &mut mpsc::Receiver<Message>,
    result_tx: &mpsc::Sender<Message>,
) -> Result<()> {
    info!("Downloading {} to {:?}", url, path);

    let mut client = ClientBuilder::default();

    let danger_accept_invalid_certs = config
        .read()
        .transport
        .tls
        .as_ref()
        .and_then(|tls| tls.danger_ignore_certificate_verification)
        .unwrap_or(false);

    if let Some(tls) = &config.read().transport.tls {
        let roots = load_roots(tls).context("Failed to load client config")?;
        for cert_der in roots {
            let cert = Certificate::from_der(&cert_der)?;
            client = client.add_root_certificate(cert);
        }
        if tls.danger_ignore_certificate_verification.unwrap_or(false) {
            client = client.danger_accept_invalid_certs(danger_accept_invalid_certs);
        }
    }

    let client = match client.build() {
        Ok(client) => client,
        Err(e) => {
            error!(
                "Failed to create reqwest client with system certificates:  {:?}",
                e
            );
            warn!("Using default reqwest client");
            reqwest::Client::builder()
                .danger_accept_invalid_certs(danger_accept_invalid_certs)
                .build()
                .context("Failed to create defaut reqwest client")?
        }
    };

    // Reqwest setup
    let res = client
        .get(url)
        .send()
        .await
        .context(format!("Failed to GET from '{}'", &url))?;

    // Check if response status is 200 OK
    if !res.status().is_success() {
        bail!("HTTP request failed with status: {}", res.status());
    }

    // Indicatif setup
    let total_size = res
        .content_length()
        .context(format!("Failed to get content length from '{}'", &url))?;

    if let Ok(file) = File::open(path) {
        if file
            .metadata()
            .context(format!("Failed to get metadata from '{:?}'", path))?
            .len()
            == total_size
        {
            return Ok(());
        }
    }

    let template = crate::t!("progress-bytes");

    let mut progress = ProgressInfo {
        message: message.to_string(),
        template,
        total: total_size as u32,
        current: 0,
        ..Default::default()
    };

    result_tx
        .send(Message::Progress(progress.clone()))
        .await
        .ok();

    // download chunks
    let mut file = File::create(path).context(format!("Failed to create file '{:?}'", path))?;
    let mut stream = res.bytes_stream();

    loop {
        tokio::select! {
            cmd = command_rx.recv() => {
                match cmd {
                    Some(Message::Stop(_)) | Some(Message::Break(_)) => {
                        info!("Download cancelled");
                        progress.total = total_size as u32;
                        result_tx.send(Message::Progress(progress.clone())).await.ok();
                        bail!("Download cancelled");
                    }
                    None => {
                        progress.total = total_size as u32;
                        result_tx.send(Message::Progress(progress.clone())).await.ok();
                        bail!("Command channel closed");
                    }
                    _ => {}
                }
            }

            item = stream.next() => {
                if let Some(item) =  item {
                let chunk = item.context("Failed to get chunk")?;
                    file.write_all(&chunk).context("Error while writing to file")?;
                    let kb_current = progress.current / 1024;
                    progress.current = min(progress.current + (chunk.len() as u32), total_size as u32);
                    let kb_new = progress.current / 1024;
                    // Throttle download progress
                    if kb_new > kb_current {
                        result_tx.send(Message::Progress(progress.clone())).await.ok();
                    }
                } else {
                    break;
                }
            }
        }
    }

    progress.current = total_size as u32;
    result_tx
        .send(Message::Progress(progress.clone()))
        .await
        .ok();
    Ok(())
}

pub fn compare_filenames(path1: &Path, path2: &Path) -> bool {
    if let (Some(file_name1), Some(file_name2)) = (path1.file_name(), path2.file_name()) {
        let filename1 = file_name1.to_string_lossy();
        let filename2 = file_name2.to_string_lossy();
        #[cfg(windows)]
        return filename1.eq_ignore_ascii_case(&filename2);
        #[cfg(not(windows))]
        filename1.eq(&filename2)
    } else {
        false
    }
}

pub fn find(dir: &Path, file: &Path) -> Result<Option<PathBuf>> {
    info!("Searching for {:?} in {:?}", file, dir);
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if compare_filenames(entry.path(), file) {
            return Ok(Some(entry.path().to_path_buf()));
        }
    }
    Ok(None)
}

pub fn get_cache_dir(subdir: &str) -> Result<PathBuf> {
    let mut cache_dir = cache_dir().context("Can't get cache dir")?;
    cache_dir.push("cloudpub");
    if !subdir.is_empty() {
        cache_dir.push(subdir);
    }
    std::fs::create_dir_all(cache_dir.clone()).context("Can't create cache dir")?;
    Ok(cache_dir)
}
