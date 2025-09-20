use anyhow::{Context, Result};
use cloudpub_common::protocol::message::Message;
use cloudpub_common::protocol::{ClientEndpoint, ServerEndpoint};
use cloudpub_common::utils::find_free_tcp_port;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, error};

#[derive(Debug, Clone)]
pub struct Settings {
    pub warm_up_count: u64,
    pub msg_count: u64,
    pub msg_size: u64,
    pub sleep_time: u64,
}

pub async fn start(port: u16) -> Result<mpsc::Sender<()>> {
    // Create a channel for stop signal
    let (stop_tx, mut stop_rx) = mpsc::channel(1);

    let tcp_addr = format!("localhost:{}", port);

    // Spawn TCP ponger in a non-blocking task
    tokio::spawn(async move {
        debug!("Starting TCP ponger on {}", tcp_addr);
        match TcpListener::bind(&tcp_addr).await {
            Ok(acceptor) => {
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = stop_rx.recv() => {
                                debug!("TCP ponger received stop signal");
                                break;
                            }
                            accept_result = acceptor.accept() => {
                                match accept_result {
                                    Ok((client, addr)) => {
                                        debug!("TCP client connected from {}", addr);
                                        tokio::spawn(pong_tcp(client));
                                    }
                                    Err(e) => {
                                        error!("Failed to accept TCP connection: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });
            }
            Err(e) => {
                error!("Failed to bind TCP ponger to {}: {}", tcp_addr, e);
            }
        }
    });

    Ok(stop_tx)
}

pub async fn publish(command_tx: mpsc::Sender<Message>) -> Result<()> {
    let port = find_free_tcp_port()
        .await
        .context("Failed to find free TCP port")?;
    // Create TCP publish args
    let client = ClientEndpoint {
        local_proto: cloudpub_common::protocol::Protocol::Tcp.into(),
        local_addr: "localhost".to_string(),
        local_port: port as u32,
        description: Some("TCP Ponger".to_string()),
        ..Default::default()
    };

    debug!("Publishing TCP service on port {}", port);
    command_tx.send(Message::EndpointStart(client)).await?;
    Ok(())
}

pub async fn ping_test(endpoint: ServerEndpoint, bare: bool) -> Result<String> {
    debug!("Running ping test on {}", endpoint);
    let addr = format!("{}:{}", endpoint.remote_addr, endpoint.remote_port);

    let _stop_tx = start(endpoint.client.as_ref().unwrap().local_port as u16)
        .await
        .context("Failed to start ping service")?;

    // Wait for the server to start
    sleep(Duration::from_millis(100)).await;

    let settings = Settings {
        warm_up_count: 1,
        msg_count: 10,
        msg_size: 48,
        sleep_time: 0,
    };

    let client = TcpStream::connect(&addr)
        .await
        .context(format!("Failed to connect to {}", addr))?;
    let mut times = ping_tcp(client, &settings).await;

    if times.is_empty() {
        return Ok(crate::t!("error-measurement"));
    }

    times.sort();

    if bare {
        let p50 = times.len() as f64 * 0.5;
        Ok(format!("{}", times[p50 as usize] / 1_000))
    } else {
        Ok(format_stats(times))
    }
}

// TCP implementation
async fn ping_tcp(mut client: TcpStream, settings: &Settings) -> Vec<u32> {
    let msg_string = "x".to_string().repeat(settings.msg_size as usize);
    let msg: &[u8] = msg_string.as_bytes();
    let mut recv_buf: [u8; 65000] = [0; 65000];

    let mut times = Vec::with_capacity(settings.msg_count as usize);

    // Warm-up phase
    for _ in 0..settings.warm_up_count {
        send_single_ping_tcp(&mut client, msg, &mut recv_buf).await;
    }

    // Measurement phase
    for _ in 0..settings.msg_count {
        let start = Instant::now();
        let bytes_read = send_single_ping_tcp(&mut client, msg, &mut recv_buf).await;
        let end = Instant::now();

        if bytes_read == 0 {
            return times;
        }

        if bytes_read != msg.len() {
            return times;
        }

        let duration = end.duration_since(start).subsec_nanos();
        times.push(duration);

        sleep(Duration::from_millis(settings.sleep_time)).await;
    }

    times
}

async fn send_single_ping_tcp(client: &mut TcpStream, msg: &[u8], recv_buf: &mut [u8]) -> usize {
    debug!("Sending ping");
    if let Err(e) = client.write_all(msg).await {
        error!("Sending ping failed: {}", e);
        return 0;
    }

    let mut bytes_read = 0;
    while bytes_read < msg.len() {
        match client.read(&mut recv_buf[bytes_read..]).await {
            Ok(0) => return 0, // Connection closed
            Ok(n) => bytes_read += n,
            Err(e) => {
                error!("Error reading from socket: {}", e);
                return 0;
            }
        }
    }

    bytes_read
}

async fn pong_tcp(mut sock: TcpStream) {
    let mut buf: [u8; 65000] = [0; 65000];

    loop {
        let total_read = match sock.read(&mut buf).await {
            Ok(0) => return, // Connection closed
            Ok(n) => n,
            Err(e) => {
                error!("Error reading from TCP socket: {}", e);
                return;
            }
        };

        // Send the response
        if let Err(e) = sock.write_all(&buf[0..total_read]).await {
            error!("Error writing to TCP socket: {}", e);
            return;
        }
    }
}

fn format_stats(times: Vec<u32>) -> String {
    let p50 = times.len() as f64 * 0.5;
    let p95 = times.len() as f64 * 0.95;
    let p99 = times.len() as f64 * 0.99;

    // Convert nanoseconds to appropriate time units for better readability
    let format_duration = |ns: u32| -> String {
        if ns < 1_000 {
            format!("{} ns", ns)
        } else if ns < 1_000_000 {
            format!("{:.2} µs", ns as f64 / 1_000.0)
        } else if ns < 1_000_000_000 {
            format!("{:.2} ms", ns as f64 / 1_000_000.0)
        } else {
            format!("{:.2} s", ns as f64 / 1_000_000_000.0)
        }
    };

    format!(
        "{}:\n   p50: {}\n   p95: {}\n   p99: {}\n   max: {}",
        crate::t!("ping-time-percentiles"),
        format_duration(times[p50 as usize]),
        format_duration(times[p95 as usize]),
        format_duration(times[p99 as usize]),
        format_duration(*times.last().unwrap()),
    )
}
