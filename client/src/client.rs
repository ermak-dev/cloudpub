use anyhow::{anyhow, bail, Context, Result};
use backoff::backoff::Backoff;
use common::protocol::message::Message;
use common::protocol::{
    AgentInfo, ConnectState, Data, DataChannelData, DataChannelDataUdp, DataChannelEof,
    EndpointRemove, EndpointStop, ErrorInfo, ErrorKind, HeartBeat, Protocol, ServerEndpoint,
};
use common::transport::{AddrMaybeCached, SocketOpts, Transport, WebsocketTransport};
use common::utils::{
    get_platform, is_tcp_port_available, proto_to_socket_addr, socket_addr_to_proto, udp_connect,
};
use common::version::VERSION;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio::time::{self, Duration, Instant};
use tracing::{debug, error, info, trace, warn};

use common::constants::{run_control_chan_backoff, DEFAULT_CLIENT_RETRY_INTERVAL_SECS};
use futures::future::FutureExt;

use crate::config::ClientConfig;
use crate::shell::SubProcess;
use crate::upgrade::handle_upgrade_available;
use bytes::Bytes;
use common::transport::ProtobufStream;
use machineid_rs::{Encryption, HWIDComponent, IdBuilder};
use std::fmt::{self, Debug, Formatter};

#[cfg(feature = "plugins")]
use crate::plugins::registry::PluginRegistry;

struct DataChannel {
    data_tx: mpsc::Sender<Data>,
}

impl DataChannel {
    fn new_client(data_tx: mpsc::Sender<Data>) -> Self {
        Self { data_tx }
    }
}

type Service = Arc<DataChannel>;

type Services = Arc<RwLock<HashMap<String, Service>>>;

impl Debug for DataChannel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("DataChannel")
    }
}

// Holds the state of a client
struct Client<T: Transport> {
    config: Arc<RwLock<ClientConfig>>,
    services: Services,
    transport: Arc<T>,
    servers: Arc<RwLock<HashMap<String, SubProcess>>>,
    connected: bool,
    data_channels: Arc<RwLock<HashMap<u32, DataChannel>>>,
    // Used to break long running setup
    break_command_tx: Option<mpsc::Sender<Message>>,
}

impl<T: 'static + Transport> Client<T> {
    // Create a Client from `[client]` config block
    async fn from(config: Arc<RwLock<ClientConfig>>) -> Result<Client<T>> {
        let transport = Arc::new(
            T::new(&config.clone().read().transport)
                .with_context(|| "Failed to create the transport")?,
        );
        Ok(Client {
            config,
            services: Default::default(),
            servers: Arc::new(RwLock::new(Default::default())),
            transport,
            connected: false,
            data_channels: Arc::new(RwLock::new(HashMap::new())),
            break_command_tx: None,
        })
    }

    // The entrypoint of Client
    async fn run(
        &mut self,
        mut command_rx: mpsc::Receiver<Message>,
        result_tx: mpsc::Sender<Message>,
    ) -> Result<()> {
        let transport = self.transport.clone();

        let config = self.config.clone();
        let services = self.services.clone();

        let mut retry_backoff = run_control_chan_backoff(DEFAULT_CLIENT_RETRY_INTERVAL_SECS);

        let mut start = Instant::now();
        result_tx
            .send(Message::ConnectState(ConnectState::Connecting.into()))
            .await
            .context("Can't send Connecting event")?;
        while let Err(err) = self
            .run_control_channel(
                config.clone(),
                transport.clone(),
                &mut command_rx,
                &result_tx,
            )
            .boxed()
            .await
        {
            if result_tx.is_closed() {
                // The client is shutting down
                break;
            }

            if self.connected {
                result_tx
                    .send(Message::Error(ErrorInfo {
                        kind: ErrorKind::HandshakeFailed.into(),
                        message: crate::t!("error-network"),
                    }))
                    .await
                    .context("Can't send Error event")?;
                result_tx
                    .send(Message::ConnectState(ConnectState::Disconnected.into()))
                    .await
                    .context("Can't send Disconnected event")?;
                result_tx
                    .send(Message::ConnectState(ConnectState::Connecting.into()))
                    .await
                    .context("Can't send Connecting event")?;
                self.connected = false;
            }

            services.write().clear();
            self.data_channels.write().clear();

            if start.elapsed() > Duration::from_secs(3) {
                // The client runs for at least 3 secs and then disconnects
                retry_backoff.reset();
            }

            if let Some(duration) = retry_backoff.next_backoff() {
                warn!("{:#}. Retry in {:?}...", err, duration);
                time::sleep(duration).await;
            }

            start = Instant::now();
        }

        services.write().clear();
        self.data_channels.write().clear();

        Ok(())
    }

    async fn run_control_channel(
        &mut self,
        config: Arc<RwLock<ClientConfig>>,
        transport: Arc<T>,
        command_rx: &mut mpsc::Receiver<Message>,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<()> {
        let url = config.read().server.clone();
        let port = url.port().unwrap_or(443);
        let host = url.host_str().context("Failed to get host")?;
        let mut host_and_port = format!("{}:{}", host, port);

        let (mut conn, _remote_addr) = loop {
            let mut remote_addr = AddrMaybeCached::new(&host_and_port);
            remote_addr
                .resolve()
                .await
                .context("Failed to resolve server address")?;

            let mut conn = transport.connect(&remote_addr).await.context(format!(
                "Failed to connect control channel to {}",
                &host_and_port
            ))?;

            self.connected = true;

            T::hint(&conn, SocketOpts::for_control_channel());

            // Send hello
            let hwid = IdBuilder::new(Encryption::SHA256)
                .add_component(HWIDComponent::OSName)
                .add_component(HWIDComponent::SystemID)
                .add_component(HWIDComponent::MachineName)
                .add_component(HWIDComponent::CPUID)
                .build("cloudpub")
                .unwrap_or_default();

            let hwid = config
                .read()
                .hwid
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or(hwid);

            let (email, password) = if let Some(ref cred) = config.read().credentials {
                (cred.0.clone(), cred.1.clone())
            } else {
                (String::new(), String::new())
            };

            let token = config.read().token.clone().unwrap_or_default().to_string();

            let agent_info = AgentInfo {
                agent_id: config.read().agent_id.clone(),
                token,
                email,
                password,
                hostname: hostname::get()?.into_string().unwrap(),
                version: VERSION.to_string(),
                gui: config.read().gui,
                platform: get_platform(),
                hwid,
                server_host_and_port: host_and_port.clone(),
            };

            debug!("Sending hello: {:?}", agent_info);

            let hello_send = Message::AgentHello(agent_info);

            conn.send_message(&hello_send)
                .await
                .context("Failed to send hello message")?;

            debug!("Reading ack");
            match conn
                .recv_message()
                .await
                .context("Failed to read ack message")?
            {
                Some(msg) => match msg {
                    Message::AgentAck(args) => {
                        if !args.token.is_empty() {
                            let mut c = config.write();
                            c.token = Some(args.token.as_str().into());
                            c.save().context("Write config")?;
                        }
                        break (conn, remote_addr);
                    }
                    Message::Redirect(r) => {
                        host_and_port = r.host_and_port.clone();
                        debug!("Redirecting to {}", host_and_port);
                        continue;
                    }
                    Message::Error(err) => {
                        result_tx
                            .send(Message::Error(err.clone()))
                            .await
                            .context("Can't send server error event")?;
                        bail!("Error: {:?}", err.kind);
                    }
                    v => bail!("Unexpected ack message: {:?}", v),
                },
                None => bail!("Connection closed while reading ack message"),
            };
        };

        debug!("Control channel established");

        result_tx
            .send(Message::ConnectState(ConnectState::Connected.into()))
            .await
            .context("Can't send Connected event")?;

        let (command_tx2, mut command_rx2) = mpsc::channel::<Message>(1024);
        // Used to break long running setup

        let heartbeat_timeout = config.read().heartbeat_timeout;

        loop {
            let command_tx2 = command_tx2.clone();
            tokio::select! {
                cmd = command_rx2.recv() => {
                    if let Some(cmd) = cmd {
                        conn.send_message(&cmd).await.context("Failed to send command")?;
                    }
                },
                cmd = command_rx.recv() => {
                    if let Some(cmd) = cmd {
                        debug!("Received message: {:?}", cmd);
                        match cmd {
                            Message::EndpointStart(client) => {
                                info!("Publishing service: {:?}", client);
                                let protocol: Protocol = client.local_proto.try_into().unwrap();
                                let server_endpoint = ServerEndpoint {
                                    guid: String::new(),
                                    client: Some(client.clone()),
                                    status: Some("offline".to_string()),
                                    default_status: Some("online".to_string()),
                                    remote_proto: protocol.into(),
                                    remote_addr: String::new(),
                                    remote_port: 0,
                                    id: 0,
                                    bind_addr: String::new(),
                                    error: String::new(),
                                };

                                result_tx.send(Message::EndpointAck(server_endpoint.clone()))
                                    .await
                                    .context("Can't send Published event")?;
                                command_tx2
                                    .send(Message::EndpointStart(client))
                                    .await
                                    .context("Failed to send EndpointStart message")?;
                            }

                            Message::EndpointStop(ep) => {
                                info!("Unpublishing service: {:?}", ep.guid);
                                // Stop server process if needed
                                self.servers.write().remove(&ep.guid);
                                let msg = Message::EndpointStop(EndpointStop { guid: ep.guid });
                                conn.send_message(&msg).await.context("Failed to send message")?;

                            }

                            Message::EndpointRemove(ep) => {
                                info!("Remove service: {:?}", ep.guid);
                                // Stop server process if needed
                                self.servers.write().remove(&ep.guid);
                                let msg = Message::EndpointRemove(EndpointRemove { guid: ep.guid });
                                conn.send_message(&msg).await.context("Failed to send message")?;
                            }
                            Message::PerformUpgrade(info) => {
                                let config_clone = config.clone();
                                if let Err(e) = handle_upgrade_available(
                                    &info.version,
                                    config_clone,
                                    command_rx,
                                    result_tx,
                                )
                                .await
                                {
                                    result_tx.send(Message::Error(ErrorInfo {
                                        kind: ErrorKind::Fatal.into(),
                                        message: e.to_string(),
                                    }))
                                    .await
                                    .context("Can't send Error event")?;
                                }
                            }
                            Message::Stop(x) => {
                                info!("Stopping the client");
                                if let Some(break_command_tx) = self.break_command_tx.as_ref() {
                                    break_command_tx.send(Message::Stop(x))
                                        .await.ok();
                                }
                                break;
                            }
                            Message::Break(x) => {
                                info!("Breaking operation");
                                if let Some(break_command_tx) = self.break_command_tx.as_ref() {
                                    break_command_tx.send(Message::Break(x))
                                        .await.ok();
                                }
                            }
                            cmd => {
                                conn.send_message(&cmd).await.context("Failed to send message")?;
                            }
                        };
                    } else {
                        debug!("No more commands, shutting down...");
                        break;
                    }
                },
                val = conn.recv_message() => {
                    match val? {
                        Some(val) => {
                            match val {
                                Message::EndpointAck(endpoint) => {
                                    // Setup and start plugin
                                    let result_tx = result_tx.clone();
                                    let command_tx2 = command_tx2.clone();
                                    let servers = self.servers.clone();
                                    let config = config.clone();
                                    let (break_command_tx, mut break_command_rx) = mpsc::channel::<Message>(1);
                                    self.break_command_tx = Some(break_command_tx);
                                    tokio::spawn(async move {
                                        if !servers.read().contains_key(&endpoint.guid) {
                                            if let Err(err) = handle_endpoint_ack(&endpoint, &config, &mut break_command_rx, &result_tx, servers.clone()).await {
                                                error!("Error handling endpoint ack: {:?}", err);
                                                servers.write().remove(&endpoint.guid);
                                                command_tx2.send(Message::EndpointStop(EndpointStop {guid: endpoint.guid.clone()})).await.ok();
                                            }
                                        }
                                        result_tx.send(Message::EndpointAck(endpoint.clone()))
                                            .await
                                            .ok();
                                    });
                                }

                                Message::CreateDataChannelWithId(create_msg) => {
                                    let channel_id = create_msg.channel_id;
                                    let endpoint = create_msg.endpoint.unwrap();

                                    debug!("Creating data channel {} for endpoint {:?}", channel_id, endpoint.guid);

                                    // Create channels for data flow
                                    let (data_tx, data_rx) = mpsc::channel::<Data>(1024);

                                    // Register the data channel
                                    {
                                        let mut channels = self.data_channels.write();
                                        channels.insert(channel_id, DataChannel::new_client(data_tx));
                                    }

                                    // Check if endpoint handled by plugin server
                                    let client = endpoint.client.unwrap();
                                    let local_addr = if let Some(s) = self.servers.read().get(&endpoint.guid) {
                                        format!("127.0.0.1:{}", s.port)
                                    } else {
                                        format!("{}:{}", client.local_addr, client.local_port)
                                    };

                                    // Immediately start handling the data channel
                                    let data_channels = self.data_channels.clone();
                                    let protocol: Protocol = client.local_proto.try_into().unwrap();

                                    tokio::spawn(async move {
                                        let res = if protocol == Protocol::Udp {
                                            handle_udp_data_channel(
                                                channel_id,
                                                local_addr,
                                                command_tx2.clone(),
                                                data_rx
                                            ).await
                                        } else {
                                            handle_tcp_data_channel(
                                                channel_id,
                                                local_addr,
                                                command_tx2.clone(),
                                                data_rx
                                            ).await
                                        };
                                        match res {
                                            Ok(true) => {
                                                debug!("Data channel {} closed by client", channel_id);
                                                command_tx2.send(Message::DataChannelEof(DataChannelEof { channel_id, error: String::new() })).await.ok();
                                            }
                                            Ok(false) => {
                                                error!("Data channel {} closed by server", channel_id);
                                            }
                                            Err(err) => {
                                                error!("Error handling data channel {}: {:?}", channel_id, err);
                                                command_tx2.send(Message::DataChannelEof(DataChannelEof { channel_id, error: err.to_string()})).await.ok();
                                            }
                                        };
                                        data_channels.write().remove(&channel_id);
                                    });
                                },

                                Message::DataChannelData(data) => {
                                    // Forward data to the appropriate data channel
                                    let data_tx = {
                                        let channels = self.data_channels.read();
                                        channels.get(&data.channel_id).map(|ch| ch.data_tx.clone())
                                    };
                                    if let Some(tx) = data_tx {
                                        if let Err(err) = tx.send(Data {
                                            data: data.data.into(),
                                            socket_addr: None
                                        }).await {
                                            self.data_channels.write().remove(&data.channel_id);
                                            error!("Error send to data channel {}: {:?}", data.channel_id, err);
                                        }
                                    } else {
                                        error!("Data channel {} not found, dropping data", data.channel_id);
                                    }
                                },

                                Message::DataChannelDataUdp(data) => {
                                    // Forward UDP data to the appropriate data channel
                                    let data_tx = {
                                        let channels = self.data_channels.read();
                                        channels.get(&data.channel_id).map(|ch| ch.data_tx.clone())
                                    };
                                    if let Some(tx) = data_tx {
                                        let socket_addr = data.socket_addr.as_ref()
                                            .map(proto_to_socket_addr)
                                            .transpose()
                                            .unwrap_or_else(|err| {
                                                error!("Invalid socket address for UDP data channel {}: {:?}", data.channel_id, err);
                                                None
                                            });

                                        if let Err(err) = tx.send(Data {
                                            data: data.data.into(),
                                            socket_addr,
                                        }).await {
                                            self.data_channels.write().remove(&data.channel_id);
                                            error!("Error send to UDP data channel {}: {:?}", data.channel_id, err);
                                        }
                                    } else {
                                        error!("UDP Data channel {} not found, dropping data", data.channel_id);
                                    }
                                },

                                Message::DataChannelEof(eof) => {
                                    // Signal EOF by dropping the data channel
                                    self.data_channels.write().remove(&eof.channel_id);
                                    if eof.error.is_empty() {
                                        // Normal EOF without error
                                        debug!("Data channel {} closed by server", eof.channel_id);
                                    } else {
                                        // EOF with error
                                        error!("Data channel {} closed by server with error: {}", eof.channel_id, eof.error);
                                    }
                                },

                                Message::HeartBeat(_) => {
                                    conn.send_message(&Message::HeartBeat(HeartBeat{})).await.context("Failed to send heartbeat")?;
                                },
                                v => {
                                    result_tx.send(v).await.context("Can't send server message")?;
                                }
                            }
                        },
                        None => {
                            debug!("Connection closed by server");
                            break;
                        }
                    }
                },
                _ = time::sleep(Duration::from_secs(heartbeat_timeout)), if heartbeat_timeout != 0 => {
                    return Err(anyhow!("Heartbeat timed out"))
                }
            }
        }

        info!("Control channel shutdown");
        result_tx
            .send(Message::ConnectState(ConnectState::Disconnected.into()))
            .await
            .context("Can't send Disconnected event")?;
        Ok(())
    }
}

// This function is no longer needed as we handle data channels differently

async fn handle_tcp_data_channel(
    channel_id: u32,
    local_addr: String,
    sender: mpsc::Sender<Message>,
    mut data_rx: mpsc::Receiver<Data>,
) -> Result<bool> {
    debug!(
        "Handling client data channel {} to {}",
        channel_id, local_addr
    );

    // Connect to local service immediately
    let mut local_stream = TcpStream::connect(&local_addr)
        .await
        .with_context(|| format!("Failed to connect to local service at {}", local_addr))?;

    // Set TCP_NODELAY for low latency
    local_stream
        .set_nodelay(true)
        .context("Failed to set TCP_NODELAY")?;

    let mut buf = [0u8; 16384]; // Smaller buffer for low latency

    loop {
        tokio::select! {
            res = local_stream.read(&mut buf) => {
                match res {
                    Ok(0) => {
                        return Ok(true)
                    },
                    Ok(n) => {
                        sender.send(Message::DataChannelData(DataChannelData {
                            channel_id,
                            data: buf[0..n].to_vec()
                        }))
                        .await
                        .context("Failed to send data to server")?;
                    },
                    Err(e) => {
                        return Err(e).context("Local service read error");
                    }
                }
            }

            // Receive data from server via control channel and write to local service
            data_result = data_rx.recv() => {
                match data_result {
                    Some(data) => {
                        trace!("Received {} bytes from server for channel {}", data.data.len(), channel_id);
                        local_stream.write_all(&data.data).await.context("Failed to write data to local service")?;
                    },
                    None => {
                        debug!("EOF received from server for channel {}", channel_id);
                        return Ok(false)
                    }
                }
            }
        }
    }
}

// UDP port map for managing forwarders per remote address
type UdpPortMap = Arc<tokio::sync::RwLock<HashMap<SocketAddr, mpsc::Sender<Bytes>>>>;

const UDP_TIMEOUT: u64 = 300; // 5 minutes timeout for UDP forwarders
const UDP_BUFFER_SIZE: usize = 65536;

async fn handle_udp_data_channel(
    channel_id: u32,
    local_addr: String,
    sender: mpsc::Sender<Message>,
    mut data_rx: mpsc::Receiver<Data>,
) -> Result<bool> {
    debug!(
        "Handling client UDP channel {} to {}",
        channel_id, local_addr
    );

    let port_map: UdpPortMap = Arc::new(tokio::sync::RwLock::new(HashMap::new()));

    loop {
        // Receive data from server via control channel
        match data_rx.recv().await {
            Some(data) => {
                let external_addr = data.socket_addr.unwrap();
                let m = port_map.read().await;

                if m.get(&external_addr).is_none() {
                    // This packet is from an address we haven't seen for a while,
                    // which is not in the UdpPortMap.
                    // So set up a mapping (and a forwarder) for it

                    // Drop the reader lock
                    drop(m);

                    // Grab the writer lock
                    // This is the only thread that will try to grab the writer lock
                    // So no need to worry about some other thread has already set up
                    // the mapping between the gap of dropping the reader lock and
                    // grabbing the writer lock
                    let mut m = port_map.write().await;

                    match udp_connect(&local_addr).await {
                        Ok(s) => {
                            let (inbound_tx, inbound_rx) = mpsc::channel(1024);
                            m.insert(external_addr, inbound_tx);
                            tokio::spawn(run_udp_forwarder(
                                s,
                                inbound_rx,
                                sender.clone(),
                                external_addr,
                                channel_id,
                                port_map.clone(),
                            ));
                        }
                        Err(e) => {
                            error!(
                                "Failed to create UDP forwarder for {}: {:#}",
                                external_addr, e
                            );
                        }
                    }
                }

                // Now there should be a udp forwarder that can receive the packet
                let m = port_map.read().await;
                if let Some(tx) = m.get(&external_addr) {
                    let _ = tx.send(data.data).await;
                }
            }
            None => {
                debug!("EOF received from server for UDP channel {}", channel_id);
                return Ok(false);
            }
        }
    }
}

// Run a UdpSocket for the visitor `from`
async fn run_udp_forwarder(
    s: UdpSocket,
    mut inbound_rx: mpsc::Receiver<Bytes>,
    sender: mpsc::Sender<Message>,
    from: SocketAddr,
    channel_id: u32,
    port_map: UdpPortMap,
) -> Result<()> {
    debug!(
        "UDP forwarder created for {} on channel {}",
        from, channel_id
    );
    let mut buf = vec![0u8; UDP_BUFFER_SIZE];

    loop {
        tokio::select! {
            // Receive from the server
            data = inbound_rx.recv() => {
                if let Some(data) = data {
                    s.send(&data).await.with_context(|| "Failed to send UDP traffic to the service")?;
                } else {
                    break;
                }
            },

            // Receive from the service
            val = s.recv(&mut buf) => {
                let len = match val {
                    Ok(v) => v,
                    Err(_) => break
                };

                sender.send(Message::DataChannelDataUdp(
                    DataChannelDataUdp {
                    channel_id,
                    data: buf[..len].to_vec(),
                    socket_addr: Some(socket_addr_to_proto(&from)),
                })).await.with_context(|| "Failed to send UDP traffic to the server")?;
            },

            // No traffic for the duration of UDP_TIMEOUT, clean up the state
            _ = time::sleep(Duration::from_secs(UDP_TIMEOUT)) => {
                break;
            }
        }
    }

    let mut port_map = port_map.write().await;
    port_map.remove(&from);

    debug!(
        "UDP forwarder dropped for {} on channel {}",
        from, channel_id
    );
    Ok(())
}

// The entrypoint of running a client
async fn setup_plugin(
    protocol: Protocol,
    config: &Arc<RwLock<ClientConfig>>,
    command_rx: &mut mpsc::Receiver<Message>,
    result_tx: &mpsc::Sender<Message>,
) -> anyhow::Result<()> {
    match protocol {
        #[cfg(feature = "plugins")]
        Protocol::Webdav | Protocol::OneC | Protocol::Minecraft => {
            if let Some(plugin) = PluginRegistry::new().get(protocol) {
                plugin.setup(config, command_rx, result_tx).await
            } else {
                Err(anyhow!(
                    "Unsupported protocol: no plugin found for {:?}",
                    protocol
                ))
            }
        }
        #[cfg(not(feature = "plugins"))]
        Protocol::Webdav | Protocol::OneC | Protocol::Minecraft => Err(anyhow!(
            "Unsupported protocol: plugins support is not enabled"
        )),
        Protocol::Tcp | Protocol::Udp | Protocol::Http | Protocol::Https | Protocol::Rtsp => Ok(()),
    }
}

async fn handle_endpoint_ack(
    endpoint: &ServerEndpoint,
    config: &Arc<RwLock<ClientConfig>>,
    command_rx: &mut mpsc::Receiver<Message>,
    result_tx: &mpsc::Sender<Message>,
    servers: Arc<RwLock<HashMap<String, SubProcess>>>,
) -> Result<()> {
    let protocol: Protocol = endpoint
        .client
        .as_ref()
        .unwrap()
        .local_proto
        .try_into()
        .context("Unsupported protocol")?;
    debug!("Publish service: {:?}", endpoint);

    if !endpoint.error.is_empty() {
        error!("Endpoint error: {}", endpoint.error);
        return Ok(());
    }

    let err = setup_plugin(protocol, config, command_rx, result_tx).await;

    if let Err(err) = err {
        error!("{:?}", err);
        result_tx
            .send(Message::Error(ErrorInfo {
                kind: ErrorKind::Fatal.into(),
                message: err.to_string(),
            }))
            .await
            .context("Can't send Error event")?;
        return Err(err);
    }

    let res: Option<anyhow::Result<SubProcess>> = match protocol {
        #[cfg(feature = "plugins")]
        Protocol::OneC | Protocol::Minecraft | Protocol::Webdav => {
            if let Some(plugin) = PluginRegistry::new().get(protocol) {
                Some(plugin.publish(endpoint, config, result_tx).await)
            } else {
                None
            }
        }
        _ => None,
    };
    match res {
        Some(Ok(p)) => {
            let port = p.port;
            servers.write().insert(endpoint.guid.clone(), p);

            let now = Instant::now();
            while is_tcp_port_available("0.0.0.0", port)
                .await
                .context("Check port availability")?
            {
                if now.elapsed() > Duration::from_secs(60) {
                    result_tx
                        .send(Message::Error(ErrorInfo {
                            kind: ErrorKind::Fatal.into(),
                            message:
                                "Не удалось запустить сервер за 60 секунд. Проверьте логи сервера."
                                    .to_string(),
                        }))
                        .await
                        .context("Can't send Error event")?;
                    break;
                }
                debug!("Waiting for server to start on port {}", port);
                time::sleep(Duration::from_secs(1)).await;
            }
        }
        Some(Err(err)) => {
            error!("{:?}", err);
            result_tx
                .send(Message::Error(ErrorInfo {
                    kind: ErrorKind::Fatal.into(),
                    message: err.to_string(),
                }))
                .await
                .context("Can't send Error event")?;
            return Err(err);
        }
        None => {}
    }

    Ok(())
}

pub async fn run_client(
    config: Arc<RwLock<ClientConfig>>,
    command_rx: mpsc::Receiver<Message>,
    result_tx: mpsc::Sender<Message>,
) -> Result<()> {
    let mut client = Client::<WebsocketTransport>::from(config)
        .await
        .context("Failed to create Websocket client")?;
    client.run(command_rx, result_tx).await
}
