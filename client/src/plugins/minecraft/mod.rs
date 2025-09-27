use crate::config::{ClientConfig, ClientOpts};
use crate::plugins::Plugin;
use crate::shell::{download, get_cache_dir, SubProcess};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use cloudpub_common::protocol::message::Message;
use cloudpub_common::protocol::ServerEndpoint;
use cloudpub_common::utils::find_free_tcp_port;
use parking_lot::RwLock;
use regex::Regex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

#[cfg(unix)]
use crate::shell::execute;

#[cfg(target_os = "windows")]
use crate::shell::unzip;

#[cfg(target_os = "windows")]
const JDK_URL: &str =
    "https://download.java.net/openjdk/jdk23/ri/openjdk-23+37_windows-x64_bin.zip";

#[cfg(target_os = "linux")]
const JDK_URL: &str =
    "https://download.java.net/openjdk/jdk23/ri/openjdk-23+37_linux-x64_bin.tar.gz";

#[cfg(target_os = "macos")]
const JDK_URL: &str = "https://download.java.net/java/GA/jdk23/3c5b90190c68498b986a97f276efd28a/37/GPL/openjdk-23_macos-x64_bin.tar.gz";

#[cfg(target_os = "android")] // TODO
const JDK_URL: &str =
    "";

// Minecraft server 1.21.8
const MINECRAFT_SERVER_URL: &str =
    "https://piston-data.mojang.com/v1/objects/6bce4ef400e4efaa63a13d5e6f6b500be969ef81/server.jar";

const MINECRAFT_SERVER_CFG: &str = include_str!("server.properties");

pub const JDK_SUBDIR: &str = "jdk";
pub const DOWNLOAD_SUBDIR: &str = "download";

#[cfg(not(target_os = "macos"))]
fn get_java() -> Result<PathBuf> {
    Ok(get_cache_dir(JDK_SUBDIR)?.join("bin").join("java"))
}

#[cfg(target_os = "macos")]
fn get_java() -> Result<PathBuf> {
    Ok(get_cache_dir(JDK_SUBDIR)?
        .join("Home")
        .join("bin")
        .join("java"))
}

pub struct MinecraftPlugin;

#[async_trait]
impl Plugin for MinecraftPlugin {
    fn name(&self) -> &'static str {
        "minecraft"
    }

    async fn setup(
        &self,
        config: &Arc<RwLock<ClientConfig>>,
        _opts: &ClientOpts,
        command_rx: &mut mpsc::Receiver<Message>,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<()> {
        info!("Setup minecraft server");

        let download_dir = get_cache_dir(DOWNLOAD_SUBDIR)?;
        let jdk_dir = get_cache_dir(JDK_SUBDIR)?;
        let jdk_filename = JDK_URL.split('/').next_back().unwrap();
        let jdk_file = download_dir.join(jdk_filename);

        let minecraft_file = download_dir.join("server.jar");

        let mut touch = jdk_dir.clone();
        touch.push("installed.txt");

        if touch.exists() {
            return Ok(());
        }

        download(
            &crate::t!("downloading-jdk"),
            config.clone(),
            JDK_URL,
            &jdk_file,
            command_rx,
            result_tx,
        )
        .await
        .context(crate::t!("error-downloading-jdk"))?;

        #[cfg(unix)]
        execute(
            "tar".into(),
            vec![
                "xvf".to_string(),
                jdk_file.to_str().unwrap().to_string(),
                "-C".to_string(),
                jdk_dir.to_str().unwrap().to_string(),
                #[cfg(target_os = "macos")]
                "--strip-components=3".to_string(),
                #[cfg(not(target_os = "macos"))]
                "--strip-components=1".to_string(),
            ],
            None,
            Default::default(),
            Some((crate::t!("installing-jdk"), result_tx.clone(), 450)),
            command_rx,
        )
        .await?;

        #[cfg(target_os = "windows")]
        unzip(
            &crate::t!("installing-jdk"),
            &jdk_file,
            &jdk_dir,
            1,
            result_tx,
        )
        .await
        .context(crate::t!("error-unpacking-jdk"))?;

        let minecraft_jar = config
            .read()
            .minecraft_server
            .clone()
            .unwrap_or(MINECRAFT_SERVER_URL.to_string());

        let maybe_path = PathBuf::from(&minecraft_jar);

        if maybe_path.is_file() {
            std::fs::copy(&minecraft_jar, &minecraft_file).with_context(
                || crate::t!("error-copying-minecraft-server", "path" => minecraft_jar),
            )?;
        } else if maybe_path.is_dir() {
            bail!(crate::t!("error-invalid-minecraft-jar-directory", "path" => minecraft_jar));
        } else if minecraft_jar.starts_with("http") {
            download(
                &crate::t!("downloading-minecraft-server"),
                config.clone(),
                &minecraft_jar,
                &minecraft_file,
                command_rx,
                result_tx,
            )
            .await
            .with_context(
                || crate::t!("error-downloading-minecraft-server", "url" => minecraft_jar),
            )?;
        } else {
            bail!(crate::t!("error-invalid-minecraft-path", "path" => minecraft_jar));
        }
        std::fs::write(touch, "Delete to reinstall").context(crate::t!("error-creating-marker"))?;

        Ok(())
    }

    async fn publish(
        &self,
        endpoint: &ServerEndpoint,
        config: &Arc<RwLock<ClientConfig>>,
        opts: &ClientOpts,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<SubProcess> {
        let minecraft_dir: PathBuf = endpoint.client.as_ref().unwrap().local_addr.clone().into();
        std::fs::create_dir_all(&minecraft_dir)
            .context(crate::t!("error-creating-server-directory"))?;

        let download_dir = get_cache_dir(DOWNLOAD_SUBDIR)?;
        let minecraft_file = download_dir.join("server.jar");

        let mut server_cfg = minecraft_dir.clone();
        server_cfg.push("server.properties");

        let mut eula = minecraft_dir.clone();
        eula.push("eula.txt");

        if !server_cfg.exists() {
            std::fs::write(&server_cfg, MINECRAFT_SERVER_CFG)
                .context(crate::t!("error-creating-server-properties"))?;
            std::fs::write(eula, "eula=true").context(crate::t!("error-creating-eula-file"))?;
        }

        let port = find_free_tcp_port()
            .await
            .context(crate::t!("error-finding-free-port"))?;

        let re = Regex::new(r"server\-port\s*=\s*\d+").unwrap();

        let server_config = std::fs::read_to_string(&server_cfg)
            .context(crate::t!("error-reading-server-properties"))?;

        // Read the server config file and replace 'server-port=XXXX' with the new port
        let server_config = re.replace_all(&server_config, |_caps: &regex::Captures| {
            format!("server-port={}", port)
        });

        std::fs::write(&server_cfg, server_config.to_string())
            .context(crate::t!("error-writing-server-properties"))?;

        // Use custom Java options if provided, otherwise use defaults
        let java_opts = config
            .read()
            .minecraft_java_opts
            .clone()
            .unwrap_or("-Xmx2048M -Xms2048M".to_string());

        // Split the Java options string into individual arguments
        let mut args: Vec<String> = java_opts
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        // Add the jar file argument
        args.push("-jar".to_string());
        args.push(minecraft_file.to_str().unwrap().to_string());
        if !opts.gui {
            args.push("nogui".to_string());
        }

        let server = SubProcess::new(
            get_java().context(crate::t!("error-getting-java-path"))?,
            args,
            Some(minecraft_dir),
            Default::default(),
            result_tx.clone(),
            port,
        );
        Ok(server)
    }
}
