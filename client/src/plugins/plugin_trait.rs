use crate::config::ClientConfig;
use crate::shell::SubProcess;
use anyhow::Result;
use async_trait::async_trait;
use common::protocol::message::Message;
use common::protocol::ServerEndpoint;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Plugin: Send + Sync {
    /// Name of the plugin
    fn name(&self) -> &'static str;

    /// Setup the plugin environment
    async fn setup(
        &self,
        config: &Arc<RwLock<ClientConfig>>,
        command_rx: &mut mpsc::Receiver<Message>,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<()>;

    /// Publish a service using this plugin
    async fn publish(
        &self,
        endpoint: &ServerEndpoint,
        config: &Arc<RwLock<ClientConfig>>,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<SubProcess>;
}
