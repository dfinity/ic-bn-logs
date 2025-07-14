use candid::Principal;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use ic_agent::Agent;
use log::{debug, error, info};
use rustls::crypto::ring;
use std::io::{self, Write};
use tokio::time::{interval, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
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
    let url_str = format!("wss://{}/logs/canister/{}", domain, canister_id);

    let url = match Url::parse(&url_str) {
        Ok(u) => u,
        Err(e) => {
            error!("[{}] Failed to parse URL: {} - {}", domain, url_str, e);
            return;
        }
    };

    info!("[{}] Attempting to connect to: {}", domain, url);

    // Attempt to connect to the WebSocket server.
    let (ws_stream, _) = match connect_async(url.to_string()).await {
        Ok((stream, response)) => {
            info!(
                "[{}] WebSocket handshake successful! Response: {:?}",
                domain,
                response.status()
            );
            (stream, response)
        }
        Err(e) => {
            error!("[{}] Failed to connect: {}", domain, e);
            return;
        }
    };

    // Split the WebSocket stream into a sender and a receiver.
    let (mut write, mut read) = ws_stream.split();

    // Create an interval for sending pings every 10 seconds.
    let mut ping_interval = interval(Duration::from_secs(10));
    ping_interval.tick().await; // Consume the first tick

    info!("[{}] Starting message and ping loop...", domain);

    // Loop indefinitely to handle incoming messages and send pings.
    loop {
        tokio::select! {
            // Handle incoming WebSocket messages.
            message = read.next() => {
                match message {
                    Some(Ok(msg)) => {
                        match msg {
                            Message::Binary(bin) => {
                                // Attempt to convert binary data to UTF-8 string, print to stdout.
                                match String::from_utf8(bin) {
                                    Ok(text) => {
                                        println!("{}", text);
                                    },
                                    Err(e) => {
                                        // If not valid UTF-8, log to stderr.
                                        debug!("[{}] Received BINARY ({} bytes, not valid UTF-8): {}", domain, e.as_bytes().len(), e);
                                    }
                                }
                            },
                            _ => {
                                debug!("[{}] Received unexpected message: {:?}", domain, msg);
                            }
                        }
                        // Ensure stdout is flushed immediately
                        io::stdout().flush().unwrap();
                    },
                    Some(Err(e)) => {
                        error!("[{}] Error receiving message: {}", domain, e);
                        break;
                    },
                    None => {
                        info!("[{}] WebSocket connection closed by remote.", domain);
                        break;
                    }
                }
            },
            // Send a ping message periodically.
            _ = ping_interval.tick() => {
                let ping_message = Message::Ping(vec![1, 2, 3, 4]);
                match write.send(ping_message).await {
                    Ok(_) => {
                        debug!("[{}] Sent PING.", domain);
                        io::stdout().flush().unwrap();
                    },
                    Err(e) => {
                        error!("[{}] Error sending PING: {}", domain, e);
                        break;
                    }
                }
            }
        }
    }

    info!("[{}] Disconnected.", domain);
}
