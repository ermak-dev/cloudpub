use crate::config::{ClientConfig, ClientOpts};
use crate::shell::SubProcess;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use cloudpub_common::fair_channel::FairSender;
use cloudpub_common::protocol::message::Message;
use cloudpub_common::protocol::{Break, ErrorInfo, ErrorKind, ServerEndpoint};
use cloudpub_common::utils::is_tcp_port_available;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{self, Duration, Instant};
use tracing::{debug, error};

#[async_trait]
pub trait Plugin: Send + Sync {
    /// Name of the plugin
    fn name(&self) -> &'static str;

    /// Setup the plugin environment
    async fn setup(
        &self,
        config: &Arc<RwLock<ClientConfig>>,
        opts: &ClientOpts,
        command_rx: &mut mpsc::Receiver<Message>,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<()>;

    /// Publish a service using this plugin
    async fn publish(
        &self,
        endpoint: &ServerEndpoint,
        config: &Arc<RwLock<ClientConfig>>,
        opts: &ClientOpts,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<SubProcess>;
}

pub struct PluginHandle {
    guid: String,
    server: Arc<RwLock<Option<SubProcess>>>,
    cancel_tx: mpsc::Sender<Message>,
    task: JoinHandle<()>,
}

impl PluginHandle {
    pub fn spawn(
        plugin: Arc<dyn Plugin>,
        mut endpoint: ServerEndpoint,
        config: Arc<RwLock<ClientConfig>>,
        opts: ClientOpts,
        to_server_tx: FairSender<Message>,
    ) -> Self {
        let guid = endpoint.guid.clone();
        let guid_for_task = guid.clone();
        let server = Arc::new(RwLock::new(None));
        let server_task = server.clone();

        // Per-process cancel and event channels
        let (cancel_tx, mut cancel_rx) = mpsc::channel::<Message>(1);
        let (proc_event_tx, mut proc_event_rx) = mpsc::channel::<Message>(1024);

        let task = tokio::spawn(async move {
            // Forward per-process events to the main client event channel
            tokio::spawn({
                let mut endpoint = endpoint.clone();
                let to_server_tx = to_server_tx.clone();
                let guid_for_task = guid_for_task.clone();
                async move {
                    use tokio::time::{Duration, Instant};

                    let mut last_progress_time = Instant::now() - Duration::from_secs(1); // Allow first progress immediately

                    while let Some(mut msg) = proc_event_rx.recv().await {
                        let should_send = match &mut msg {
                            Message::Progress(progress_info) => {
                                progress_info.guid = guid_for_task.clone();

                                let now = Instant::now();
                                // Always send 0% and 100% progress messages, or throttle to 1 per second
                                if progress_info.current == 0
                                    || progress_info.current >= progress_info.total
                                    || now.duration_since(last_progress_time)
                                        >= Duration::from_secs(1)
                                {
                                    last_progress_time = now;
                                    true
                                } else {
                                    false
                                }
                            }
                            _ => true, // Always send non-progress messages
                        };

                        if should_send {
                            to_server_tx.send(msg.clone()).await.ok();
                        }
                    }

                    endpoint.status = Some("offline".to_string());
                    to_server_tx
                        .send(Message::EndpointStatus(endpoint.clone()))
                        .await
                        .ok();
                }
            });

            // Initial lifecycle status: waiting
            endpoint.status = Some("waiting".to_string());
            let _ = to_server_tx
                .send(Message::EndpointStatus(endpoint.clone()))
                .await;

            let res: Result<()> = async {
                // Long running setup: cancellable via cancel_rx
                plugin
                    .setup(&config, &opts, &mut cancel_rx, &proc_event_tx)
                    .await
                    .context("Failed to setup plugin")?;

                let server_process = plugin
                    .publish(&endpoint, &config, &opts, &proc_event_tx)
                    .await
                    .context("Failed to publish plugin service")?;

                // Wait until the port is bound
                let now = Instant::now();
                while is_tcp_port_available("127.0.0.1", server_process.port)
                    .await
                    .context("Check port availability")?
                {
                    if server_process.result.read().is_err() {
                        return Ok(()); // Error already reported by subprocess
                    }

                    if now.elapsed() > Duration::from_secs(60) {
                        bail!("{}", crate::t!("error-start-server"));
                    }
                    debug!(
                        "Waiting for server to start on port {}",
                        server_process.port
                    );
                    time::sleep(Duration::from_secs(1)).await;
                }

                *server_task.write() = Some(server_process);
                Ok(())
            }
            .await;

            match res {
                Ok(()) => {
                    endpoint.status = Some("online".into());
                    let _ = to_server_tx.send(Message::EndpointStatus(endpoint)).await;
                }
                Err(e) => {
                    error!("Error handling endpoint {}: {:#}", &guid_for_task, e);
                    let _ = to_server_tx
                        .send(Message::Error(ErrorInfo {
                            kind: ErrorKind::PublishFailed.into(),
                            message: e.to_string(),
                            guid: guid_for_task,
                        }))
                        .await;
                }
            }
        });

        Self {
            guid,
            server,
            cancel_tx,
            task,
        }
    }

    pub fn port(&self) -> Option<u16> {
        self.server.read().as_ref().map(|s| s.port)
    }

    pub fn guid(&self) -> &str {
        &self.guid
    }

    pub fn send_break(&self) {
        let _ = self.cancel_tx.try_send(Message::Break(Break {
            guid: self.guid.clone(),
        }));
    }
}

impl Drop for PluginHandle {
    fn drop(&mut self) {
        // Abort main task and drop cancel_tx to notify setup to stop
        self.task.abort();
        // cancel_tx dropped here; receiver sees end-of-stream and should terminate
        let _ = self.cancel_tx.try_send(Message::Break(Break {
            guid: self.guid.clone(),
        }));
    }
}
