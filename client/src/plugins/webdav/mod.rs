use crate::config::{ClientConfig, ClientOpts};
use crate::plugins::httpd::{setup_httpd, start_httpd};
use crate::plugins::Plugin;
use crate::shell::SubProcess;
use anyhow::Result;
use async_trait::async_trait;
use cloudpub_common::protocol::message::Message;
use cloudpub_common::protocol::ServerEndpoint;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;

const WEBDAV_CONFIG: &str = include_str!("httpd.conf");
const WEBDAV_SUBDIR: &str = "webdav";

pub struct WebdavPlugin;

#[async_trait]
impl Plugin for WebdavPlugin {
    fn name(&self) -> &'static str {
        "webdav"
    }

    async fn setup(
        &self,
        config: &Arc<RwLock<ClientConfig>>,
        _opts: &ClientOpts,
        command_rx: &mut mpsc::Receiver<Message>,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<()> {
        setup_httpd(config, command_rx, result_tx, Default::default()).await
    }

    async fn publish(
        &self,
        endpoint: &ServerEndpoint,
        _config: &Arc<RwLock<ClientConfig>>,
        _opts: &ClientOpts,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<SubProcess> {
        // Well, there is actually local_addr for that, but to backward compatibility
        // we must use local_addr for the publish directory.
        let publish_dir = endpoint.client.as_ref().unwrap().local_addr.clone();
        let env = Default::default();

        start_httpd(
            endpoint,
            WEBDAV_CONFIG,
            WEBDAV_SUBDIR,
            &publish_dir,
            env,
            result_tx.clone(),
        )
        .await
    }
}
