use candid::Principal;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use ic_agent::Agent;
use log::{debug, error, info};
use rustls::crypto::ring;
use std::io::{self, Write};
use strip_ansi_escapes::strip;
use tokio::time::{interval, Duration};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
};
use url::Url;

#[derive(Parser)]
#[command(name = "ic-bn-logs-client")]
#[command(about = "A WebSocket client for Internet Computer API boundary node logs")]
struct Args {
    /// The canister ID to monitor logs for
    #[arg(short, long)]
    canister_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize env_logger. By default, it logs to stderr.
    env_logger::init();

    // Install the default crypto provider for rustls.
    rustls::crypto::CryptoProvider::install_default(ring::default_provider())
        .expect("Failed to install rustls crypto provider");

    // Fetch all API boundary nodes from the Internet Computer.
    let agent = Agent::builder().with_url("https://icp-api.io").build()?;
    let api_bns = agent
        .fetch_api_boundary_nodes_by_subnet_id(
            Principal::from_text("tdb26-jop6k-aogll-7ltgs-eruif-6kk7m-qpktf-gdiqx-mxtrf-vb5e6-eqe")
                .unwrap(),
        )
        .await?;
    let api_bn_domains: Vec<String> = api_bns.iter().map(|node| node.domain.clone()).collect();
    info!("Fetched {} API boundary nodes.", api_bn_domains.len());
    info!("{:?}", api_bn_domains);

    if api_bn_domains.is_empty() {
        error!("No API boundary nodes found. Exiting.");
        return Ok(());
    }

    // Spawn a task for each domain to handle its WebSocket connection independently.
    for domain in api_bn_domains {
        tokio::spawn(handle_websocket_connection(
            domain.to_string(),
            args.canister_id.clone(),
        ));
    }

    info!("WebSocket clients started. Press Ctrl+C to exit.");
    tokio::signal::ctrl_c().await?;
    info!("Shutting down WebSocket clients.");

    Ok(())
}

/// Handles a single WebSocket connection, sending pings and printing messages.
async fn handle_websocket_connection(domain: String, canister_id: String) {
    // Construct the WebSocket URL.
    let url_str = format!("wss://{domain}/logs/canister/{canister_id}");

    let url = match Url::parse(&url_str) {
        Ok(u) => u,
        Err(e) => {
            error!("[{domain}] Failed to parse URL: {url_str} - {e}");
            return;
        }
    };

    info!("[{domain}] Attempting to connect to: {url}");

    // Configure WebSocket with message size limits for security
    let ws_config = WebSocketConfig {
        max_message_size: Some(5 * 1024), // 5KB limit
        max_frame_size: Some(5 * 1024),   // 5KB frame limit
        ..Default::default()
    };

    // Attempt to connect to the WebSocket server with configuration.
    let (ws_stream, _) =
        match connect_async_with_config(url.to_string(), Some(ws_config), false).await {
            Ok((stream, response)) => {
                info!(
                    "[{domain}] WebSocket handshake successful! Response: {:?}",
                    response.status()
                );
                (stream, response)
            }
            Err(e) => {
                error!("[{domain}] Failed to connect: {e}");
                return;
            }
        };

    // Split the WebSocket stream into a sender and a receiver.
    let (mut write, mut read) = ws_stream.split();

    // Create an interval for sending pings every 10 seconds.
    let mut ping_interval = interval(Duration::from_secs(10));
    ping_interval.tick().await; // Consume the first tick

    info!("[{domain}] Starting message and ping loop...");

    // Loop indefinitely to handle incoming messages and send pings.
    loop {
        tokio::select! {
            // Handle incoming WebSocket messages.
            message = read.next() => {
                if !handle_incoming_message(&domain, message) {
                    break;
                }
            },
            // Send a ping message periodically.
            _ = ping_interval.tick() => {
                if !send_ping_message(&domain, &mut write).await {
                    break;
                }
            }
        }
    }

    info!("[{domain}] Disconnected.");
}

/// Handles an incoming WebSocket message and prints it to stdout
fn handle_incoming_message(
    domain: &str,
    message: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
) -> bool {
    match message {
        Some(Ok(Message::Binary(bin))) => {
            // Strip ANSI escape sequences
            let sanitized_bytes = strip(&bin);
            match String::from_utf8(sanitized_bytes) {
                Ok(sanitized_text) => {
                    println!("{sanitized_text}");
                }
                Err(e) => {
                    debug!(
                        "[{domain}] Received BINARY ({} bytes, not valid UTF-8): {e}",
                        e.as_bytes().len()
                    );
                }
            }
            // Ensure stdout is flushed immediately
            io::stdout().flush().unwrap();
            true
        }
        Some(Ok(msg)) => {
            debug!("[{domain}] Received unexpected message: {msg:?}");
            true
        }
        Some(Err(e)) => {
            error!("[{domain}] Error receiving message: {e}");
            false
        }
        None => {
            info!("[{domain}] WebSocket connection closed by remote.");
            false
        }
    }
}

/// Sends a ping message to keep the WebSocket connection alive
async fn send_ping_message(
    domain: &str,
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
) -> bool {
    let ping_message = Message::Ping(vec![1, 2, 3, 4]);
    match write.send(ping_message).await {
        Ok(_) => {
            debug!("[{domain}] Sent PING.");
            io::stdout().flush().unwrap();
            true
        }
        Err(e) => {
            error!("[{domain}] Error sending PING: {e}");
            false
        }
    }
}
