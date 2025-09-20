use anyhow::{anyhow, bail, Context, Result};
use backoff::backoff::Backoff;
use cloudpub_common::data::DataChannel;
use cloudpub_common::fair_channel::{fair_channel, FairSender};
use cloudpub_common::protocol::message::Message;
use cloudpub_common::protocol::{
    AgentInfo, ConnectState, Data, DataChannelAck, DataChannelData, DataChannelDataUdp,
    DataChannelEof, ErrorInfo, ErrorKind, HeartBeat, Protocol,
};
use cloudpub_common::transport::{AddrMaybeCached, SocketOpts, Transport, WebsocketTransport};
use cloudpub_common::utils::{
    get_platform, proto_to_socket_addr, socket_addr_to_proto, udp_connect,
};
use cloudpub_common::VERSION;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio::time::{self, Duration, Instant};
use tracing::{debug, error, info, trace, warn};

use cloudpub_common::constants::{
    run_control_chan_backoff, CONTROL_CHANNEL_SIZE, DATA_BUFFER_SIZE, DATA_CHANNEL_SIZE,
    DEFAULT_CLIENT_RETRY_INTERVAL_SECS, UDP_BUFFER_SIZE, UDP_TIMEOUT,
};
use futures::future::FutureExt;

use crate::config::{ClientConfig, ClientOpts};
use crate::upgrade::handle_upgrade_available;
use bytes::Bytes;
use cloudpub_common::transport::ProtobufStream;

#[cfg(feature = "plugins")]
use crate::plugins::plugin_trait::PluginHandle;
#[cfg(feature = "plugins")]
use crate::plugins::registry::PluginRegistry;

type Service = Arc<DataChannel>;

type Services = Arc<DashMap<String, Service>>;

// Holds the state of a client
struct Client<T: Transport> {
    config: Arc<RwLock<ClientConfig>>,
    opts: ClientOpts,
    services: Services,
    transport: Arc<T>,
    connected: bool,
    #[cfg(feature = "plugins")]
    plugin_processes: Arc<DashMap<String, PluginHandle>>,
    data_channels: Arc<DashMap<u32, Arc<DataChannel>>>,
}

impl<T: 'static + Transport> Client<T> {
    // Create a Client from `[client]` config block
    async fn from(config: Arc<RwLock<ClientConfig>>, opts: ClientOpts) -> Result<Client<T>> {
        let transport = Arc::new(
            T::new(&config.clone().read().transport)
                .with_context(|| "Failed to create the transport")?,
        );
        Ok(Client {
            config,
            opts,
            services: Default::default(),
            transport,
            connected: false,
            #[cfg(feature = "plugins")]
            plugin_processes: Arc::new(DashMap::new()),
            data_channels: Arc::new(DashMap::new()),
        })
    }

    // The entrypoint of Client
    async fn run(
        &mut self,
        mut command_rx: mpsc::Receiver<Message>,
        result_tx: mpsc::Sender<Message>,
    ) -> Result<()> {
        let transport = self.transport.clone();

        let mut retry_backoff = run_control_chan_backoff(DEFAULT_CLIENT_RETRY_INTERVAL_SECS);

        let mut start = Instant::now();
        result_tx
            .send(Message::ConnectState(ConnectState::Connecting.into()))
            .await
            .context("Can't send Connecting event")?;
        while let Err(err) = self
            .run_control_channel(transport.clone(), &mut command_rx, &result_tx)
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
                        guid: String::new(),
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

            self.services.clear();
            #[cfg(feature = "plugins")]
            self.plugin_processes.clear();
            self.data_channels.clear();

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

        self.services.clear();
        #[cfg(feature = "plugins")]
        self.plugin_processes.clear();
        self.data_channels.clear();

        Ok(())
    }

    async fn run_control_channel(
        &mut self,
        transport: Arc<T>,
        command_rx: &mut mpsc::Receiver<Message>,
        result_tx: &mpsc::Sender<Message>,
    ) -> Result<()> {
        let url = self.config.read().server.clone();
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

            let (email, password) = if let Some(ref cred) = self.opts.credentials {
                (cred.0.clone(), cred.1.clone())
            } else {
                (String::new(), String::new())
            };

            let token = self
                .config
                .read()
                .token
                .clone()
                .unwrap_or_default()
                .to_string();

            let hwid = self.config.read().get_hwid();

            let agent_info = AgentInfo {
                agent_id: self.config.read().agent_id.clone(),
                token,
                email,
                password,
                hostname: hostname::get()?.to_string_lossy().into_owned(),
                version: VERSION.to_string(),
                gui: self.opts.gui,
                platform: get_platform(),
                hwid,
                server_host_and_port: host_and_port.clone(),
                transient: self.opts.transient,
                secondary: self.opts.secondary,
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
                            let mut c = self.config.write();
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

        let (to_server_tx, mut to_server_rx) = fair_channel::<Message>(CONTROL_CHANNEL_SIZE);
        // Used to break long running setup

        let heartbeat_timeout = self.config.read().heartbeat_timeout;

        loop {
            tokio::select! {
                cmd = to_server_rx.recv() => {
                    if let Some(cmd) = cmd {
                        conn.send_message(&cmd).await.context("Failed to send command")?;
                    }
                },
                cmd = command_rx.recv() => {
                    if let Some(cmd) = cmd {
                        debug!("Received message: {:?}", cmd);
                        match cmd {
                            Message::PerformUpgrade(info) => {
                                let config_clone = self.config.clone();
                                if let Err(e) = handle_upgrade_available(
                                    &info.version,
                                    config_clone,
                                    self.opts.gui,
                                    command_rx,
                                    result_tx,
                                )
                                .await
                                {
                                    result_tx.send(Message::Error(ErrorInfo {
                                        kind: ErrorKind::Fatal.into(),
                                        message: e.to_string(),
                                        guid: String::new(),
                                    }))
                                    .await
                                    .context("Can't send Error event")?;
                                }
                            }
                            Message::Stop(_x) => {
                                info!("Stopping the client");
                                break;
                            }
                            Message::Break(break_msg) => {
                                info!("Breaking operation for guid: {}", break_msg.guid);
                                #[cfg(feature = "plugins")]
                                if let Some((_, handle)) = self.plugin_processes.remove(&break_msg.guid) {
                                    info!("Dropped plugin handle for guid: {}", break_msg.guid);
                                    drop(handle);
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
                                Message::EndpointAck(mut endpoint) => {
                                    #[cfg(feature = "plugins")]
                                    {
                                        let to_server_tx = to_server_tx.clone();
                                        let config = self.config.clone();
                                        let opts = self.opts.clone();
                                        if endpoint.error.is_empty() {
                                            let protocol: Protocol = endpoint
                                                .client
                                                .as_ref()
                                                .unwrap()
                                                .local_proto
                                                .try_into()
                                                .unwrap_or(Protocol::Tcp);
                                            if let Some(plugin) = PluginRegistry::new().get(protocol) {
                                                let handle = PluginHandle::spawn(
                                                    plugin,
                                                    endpoint.clone(),
                                                    config,
                                                    opts,
                                                    to_server_tx,
                                                );
                                                self.plugin_processes.insert(endpoint.guid.clone(), handle);
                                            } else {
                                                endpoint.status = Some("online".into());
                                                let _ = to_server_tx.send(Message::EndpointStatus(endpoint.clone())).await;
                                            }
                                        }
                                    }
                                    #[cfg(not(feature = "plugins"))]
                                    {
                                        endpoint.status = Some("online".into());
                                        let _ = to_server_tx.send(Message::EndpointStatus(endpoint.clone())).await;
                                    }
                                    result_tx
                                        .send(Message::EndpointAck(endpoint))
                                        .await
                                        .context("Can't send EndpointAck event")?;
                                }

                                Message::CreateDataChannelWithId(create_msg) => {
                                    let channel_id = create_msg.channel_id;
                                    let endpoint = create_msg.endpoint.unwrap();

                                    trace!("Creating data channel {} for endpoint {:?}", channel_id, endpoint.guid);

                                    // Create channels for data flow
                                    let (to_service_tx, to_service_rx) = mpsc::channel::<Data>(DATA_CHANNEL_SIZE);

                                    // Register the data channel
                                    let data_channel = Arc::new(DataChannel::new_client(channel_id, to_service_tx.clone()));
                                    self.data_channels.insert(channel_id, data_channel.clone());

                                    // Check if endpoint handled by plugin server
                                    let client = endpoint.client.unwrap();
                                    let mut local_addr = format!("{}:{}", client.local_addr, client.local_port);
                                    #[cfg(feature = "plugins")]
                                    if let Some(handle) = self.plugin_processes.get(&endpoint.guid) {
                                        if let Some(port) = handle.value().port() {
                                            local_addr = format!("127.0.0.1:{}", port);
                                        }
                                    }

                                    // Immediately start handling the data channel
                                    let data_channels = self.data_channels.clone();
                                    let protocol: Protocol = client.local_proto.try_into().unwrap();

                                    let to_server_tx_cloned = to_server_tx.clone();
                                    tokio::spawn(async move {
                                        if let Err(err) = if protocol == Protocol::Udp {
                                            handle_udp_data_channel(
                                                data_channel,
                                                local_addr,
                                                to_server_tx_cloned.clone(),
                                                to_service_rx
                                            ).await
                                        } else {
                                            handle_tcp_data_channel(
                                                data_channel,
                                                local_addr,
                                                to_server_tx_cloned.clone(),
                                                to_service_rx
                                            ).await
                                        } {
                                            error!("DataChannel {{ channel_id: {} }}: {:?}", channel_id, err);
                                            to_server_tx_cloned
                                                .send(Message::DataChannelEof(
                                                        DataChannelEof {
                                                            channel_id,
                                                            error: err.to_string()
                                                    })
                                                ).await.ok();
                                        }
                                        if let Some((_, dc)) = data_channels.remove(&channel_id) { dc.close() }
                                    });
                                },

                                Message::DataChannelData(data) => {
                                    // Forward data to the appropriate data channel
                                    let to_service_tx = self.data_channels.get(&data.channel_id).map(|ch| ch.data_tx.clone());
                                    if let Some(tx) = to_service_tx {
                                        if let Err(err) = tx.send(Data {
                                            data: data.data.into(),
                                            socket_addr: None
                                        }).await {
                                            self.data_channels.remove(&data.channel_id);
                                            error!("Error send to data channel {}: {:?}", data.channel_id, err);
                                        }
                                    } else {
                                        trace!("Data channel {} not found, dropping data", data.channel_id);
                                    }
                                },

                                Message::DataChannelDataUdp(data) => {
                                    // Forward UDP data to the appropriate data channel
                                    let to_service_tx = self.data_channels.get(&data.channel_id).map(|ch| ch.data_tx.clone());
                                    if let Some(tx) = to_service_tx {
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
                                            self.data_channels.remove(&data.channel_id);
                                            error!("Error send to UDP data channel {}: {:?}", data.channel_id, err);
                                        }
                                    } else {
                                        trace!("UDP Data channel {} not found, dropping data", data.channel_id);
                                    }
                                },

                                Message::DataChannelEof(eof) => {
                                    // Signal EOF by dropping the data channel
                                    if let Some((_, dc)) = self.data_channels.remove(&eof.channel_id) { dc.close() }
                                    if eof.error.is_empty() {
                                        // Normal EOF without error
                                        trace!("Data channel {} closed by server", eof.channel_id);
                                    } else {
                                        // EOF with error
                                        trace!("Data channel {} closed by server with error: {}", eof.channel_id, eof.error);
                                    }
                                },

                                Message::DataChannelAck(DataChannelAck { channel_id, consumed }) => {
                                    if let Some(ch) = self.data_channels.get(&channel_id) {
                                        ch.add_capacity(consumed);
                                    }
                                }

                                Message::EndpointStopAck(ref ep) => {
                                    self.services.remove(&ep.guid);
                                    #[cfg(feature = "plugins")]
                                    self.plugin_processes.remove(&ep.guid);
                                    result_tx.send(val).await.context("Can't send result message")?;
                                }

                                Message::EndpointRemoveAck(ref ep) => {
                                    self.services.remove(&ep.guid);
                                    #[cfg(feature = "plugins")]
                                    self.plugin_processes.remove(&ep.guid);
                                    result_tx.send(val).await.context("Can't send result message")?;
                                }

                                Message::HeartBeat(_) => {
                                    conn.send_message(&Message::HeartBeat(HeartBeat{})).await.context("Failed to send heartbeat")?;
                                },

                                Message::Error(ref err) => {
                                    let kind: ErrorKind = err.kind.try_into().unwrap_or(ErrorKind::Fatal);
                                    result_tx.send(val.clone()).await.context("Can't send result message")?;
                                    if kind == ErrorKind::Fatal || kind == ErrorKind::AuthFailed {
                                        error!("Fatal error received, stop client: {:?}", err);
                                        break;
                                    }
                                }

                                Message::Break(break_msg) => {
                                    info!("Breaking operation for guid: {}", break_msg.guid);
                                    #[cfg(feature = "plugins")]
                                    self.plugin_processes.remove(&break_msg.guid);
                                }

                                v => {
                                    result_tx.send(v).await.context("Can't send result message")?;
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
        conn.close().await.ok();
        time::sleep(Duration::from_millis(100)).await; // Give some time for the connection to close gracefully
        Ok(())
    }
}

pub async fn run_client(
    config: Arc<RwLock<ClientConfig>>,
    opts: ClientOpts,
    command_rx: mpsc::Receiver<Message>,
    result_tx: mpsc::Sender<Message>,
) -> Result<()> {
    let mut client = Client::<WebsocketTransport>::from(config, opts)
        .await
        .context("Failed to create Websocket client")?;
    client.run(command_rx, result_tx).await
}
async fn handle_tcp_data_channel(
    data_channel: Arc<DataChannel>,
    local_addr: String,
    to_server_tx: FairSender<Message>,
    mut data_rx: mpsc::Receiver<Data>,
) -> Result<()> {
    trace!("Handling client {:?} to {}", data_channel, local_addr);

    // Connect to local service immediately
    let mut local_stream = TcpStream::connect(&local_addr)
        .await
        .with_context(|| format!("Failed to connect to local service at {}", local_addr))?;

    // Set TCP_NODELAY for low latency
    local_stream
        .set_nodelay(true)
        .context("Failed to set TCP_NODELAY")?;

    let mut buf = [0u8; DATA_BUFFER_SIZE]; // Smaller buffer for low latency

    loop {
        tokio::select! {
            res = local_stream.read(&mut buf) => {
                match res {
                    Ok(0) => {
                        trace!("EOF received from local service for {:?}", data_channel);
                        if let Err(err) = to_server_tx.send(Message::DataChannelEof(DataChannelEof {
                            channel_id: data_channel.id,
                            error: String::new()
                        }))
                        .await {
                            trace!("Failed to send EOF to server for {:?}: {:#}", data_channel, err);
                        }
                        break;
                    },
                    Ok(n) => {
                        //debug!("Read {} bytes from local service for {:?}", n, data_channel);
                        if data_channel.wait_for_capacity(n as u32).await.is_err() {
                            trace!("Data channel {} closed when waiting for capacity", data_channel.id);
                            break;
                        }
                        if let Err(err) = to_server_tx.send(Message::DataChannelData(DataChannelData {
                            channel_id: data_channel.id,
                            data: buf[0..n].to_vec()
                        }))
                        .await {
                            trace!("Failed to send data to server for {:?}: {:#}", data_channel, err);
                            break;
                        }
                    },
                    Err(e) => {
                        return Err(e).context("Failed to read from local service");
                    }
                }
            }

            // Receive data from server via control channel and write to local service
            data_result = data_rx.recv() => {
                match data_result {
                    Some(data) => {
                        trace!("Received {} bytes from server for {:?}", data.data.len(), data_channel);
                        local_stream.write_all(&data.data).await.context("Failed to write data to local service")?;
                        to_server_tx.send(Message::DataChannelAck(
                            DataChannelAck {
                                channel_id: data_channel.id,
                                consumed: data.data.len() as u32
                            }
                        )).await.with_context(|| "Failed to send TCP traffic ack to the server")?;
                    },
                    None => {
                        trace!("EOF received from server for {:?}", data_channel);
                        break;
                    }
                }
            }

            _ = data_channel.closed() => {
                trace!("Data channel {} closed", data_channel.id);
                break;
            }
        }
    }
    Ok(())
}

// UDP port map for managing forwarders per remote address
type UdpPortMap = Arc<DashMap<SocketAddr, mpsc::Sender<Bytes>>>;

async fn handle_udp_data_channel(
    data_channel: Arc<DataChannel>,
    local_addr: String,
    to_server_tx: FairSender<Message>,
    mut data_rx: mpsc::Receiver<Data>,
) -> Result<()> {
    trace!(
        "Handling client UDP channel {:?} to {}",
        data_channel,
        local_addr
    );

    let port_map: UdpPortMap = Arc::new(DashMap::new());

    loop {
        let data_channel = data_channel.clone();
        // Receive data from server via control channel
        tokio::select! {
            data = data_rx.recv() => {
                match data {
                    Some(data) => {
                        let external_addr = data.socket_addr.unwrap();

                        if !port_map.contains_key(&external_addr) {
                            // This packet is from an address we haven't seen for a while,
                            // which is not in the UdpPortMap.
                            // So set up a mapping (and a forwarder) for it

                            match udp_connect(&local_addr).await {
                                Ok(s) => {
                                    let (to_service_tx, to_service_rx) = mpsc::channel(DATA_CHANNEL_SIZE);
                                    port_map.insert(external_addr, to_service_tx);
                                    tokio::spawn(run_udp_forwarder(
                                        s,
                                        to_service_rx,
                                        to_server_tx.clone(),
                                        external_addr,
                                        data_channel,
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
                        if let Some(tx) = port_map.get(&external_addr) {
                            let _ = tx.send(data.data).await;
                        }
                    }
                    None => {
                        trace!("EOF received from server for UDP {:?}", data_channel);
                        break;
                    }
                }
            }
            _ = data_channel.closed() => {
                trace!("Data channel {} closed", data_channel.id);
                break;
            }
        }
    }
    Ok(())
}

// Run a UdpSocket for the visitor `from`
async fn run_udp_forwarder(
    s: UdpSocket,
    mut to_service_rx: mpsc::Receiver<Bytes>,
    to_server_tx: FairSender<Message>,
    from: SocketAddr,
    data_channel: Arc<DataChannel>,
    port_map: UdpPortMap,
) -> Result<()> {
    trace!("UDP forwarder created for {} on {:?}", from, data_channel);
    let mut buf = vec![0u8; UDP_BUFFER_SIZE];

    loop {
        tokio::select! {
            // Receive from the server
            data = to_service_rx.recv() => {
                if let Some(data) = data {
                    s.send(&data).await.with_context(|| "Failed to send UDP traffic to the service")?;
                    to_server_tx.send(Message::DataChannelAck(
                        DataChannelAck {
                            channel_id: data_channel.id,
                            consumed: data.len() as u32
                        }
                    )).await.with_context(|| "Failed to send UDP traffic ack to the server")?;
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

                if data_channel.wait_for_capacity(len as u32).await.is_err() {
                    break;
                }

                to_server_tx.send(Message::DataChannelDataUdp(
                    DataChannelDataUdp {
                    channel_id: data_channel.id,
                    data: buf[..len].to_vec(),
                    socket_addr: Some(socket_addr_to_proto(&from)),
                })).await.with_context(|| "Failed to send UDP traffic to the server")?;
            },

            // No traffic for the duration of UDP_TIMEOUT, clean up the state
            _ = time::sleep(Duration::from_secs(UDP_TIMEOUT)) => {
                break;
            }

            _ = data_channel.closed() => {
                trace!("Data channel {} closed", data_channel.id);
                break;
            }
        }
    }

    port_map.remove(&from);

    debug!("UDP forwarder dropped for {} on {:?}", from, data_channel);
    Ok(())
}
