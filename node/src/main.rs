mod api;
mod file_transfer;
mod messaging_behaviour;
mod protocol_handler;
mod websocket;

use api::{start_api_server, ApiState, NodeStats, PeerInfo, FileInfo, FileStatus};
use futures::StreamExt;
use libp2p::{
    identify, identity, mdns, noise, ping, swarm::SwarmEvent, tcp, yamux, Multiaddr, SwarmBuilder,
};
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time;
use tracing::{info, Level};
use websocket::{start_websocket_server, WsEvent, WsEventSender};

use messaging_behaviour::{MessagingBehaviour, MessagingBehaviourEvent};

#[derive(libp2p::swarm::NetworkBehaviour)]
struct CoreLinkBehaviour {
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    mdns: mdns::tokio::Behaviour,
    messaging: MessagingBehaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Tracing setup
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args
        .iter()
        .position(|arg| arg == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(4001);

    info!("🚀 Starting CoreLink node on port {}", port);

    // Create identity
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = local_key.public().to_peer_id();

    info!("🔑 Peer ID: {}", local_peer_id);

    // Create swarm
    let mut swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(
            |key| -> Result<CoreLinkBehaviour, Box<dyn Error + Send + Sync>> {
                let peer_id = key.public().to_peer_id();
                Ok(CoreLinkBehaviour {
                    ping: ping::Behaviour::new(ping::Config::new()),
                    identify: identify::Behaviour::new(identify::Config::new(
                        "/corelink/1.0.0".to_string(),
                        key.public(),
                    )),
                    mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?,
                    messaging: MessagingBehaviour::new()?,
                })
            },
        )?
        .with_swarm_config(|c| {
            c.with_idle_connection_timeout(Duration::from_secs(60))
                .with_per_connection_event_buffer_size(64)
        })
        .build();

    // Listen on all interfaces
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", port).parse()?;
    swarm.listen_on(listen_addr.clone())?;

    info!("👂 Listening on {}", listen_addr);

    // Start WebSocket server (derive port from node port: 4001 -> 8001, 4002 -> 8002, etc.)
    let ws_port = port + 4000;
    let ws_addr = format!("127.0.0.1:{}", ws_port);
    let ws_tx = start_websocket_server(&ws_addr)
        .await
        .expect("Failed to start WebSocket server");
    info!("🌐 WebSocket server ready at ws://{}", ws_addr);

    // Create API state and start REST API server (derive port from node port: 4001 -> 7001, 4002 -> 7002, etc.)
    let api_state = ApiState::new();
    let api_state_clone = api_state.clone();
    let api_port = port + 3000;
    let api_addr = format!("127.0.0.1:{}", api_port);
    let api_addr_clone = api_addr.clone();

    tokio::spawn(async move {
        if let Err(e) = start_api_server(&api_addr_clone, api_state_clone).await {
            tracing::error!("API server error: {}", e);
        }
    });
    info!("🌐 REST API server ready at http://{}", api_addr);

    // Track start time for uptime calculation
    let start_time = std::time::Instant::now();

    // Setup stdin for interactive commands
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    info!("💡 Commands: 'offer' to share test.txt, 'help' for more");

    // Discovery broadcast interval
    let mut discovery_interval = time::interval(Duration::from_secs(10));

    // Status broadcast interval (every 5 seconds)
    let mut status_interval = time::interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("📍 Listening on {}", address);
                    }
                    SwarmEvent::Behaviour(CoreLinkBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, addr) in list {
                            info!("🔍 Discovered peer: {} at {}", peer_id, addr);
                            if let Err(e) = swarm.dial(addr.clone()) {
                                info!("❌ Failed to dial {}: {:?}", peer_id, e);
                            }
                        }
                    }
                    SwarmEvent::Behaviour(CoreLinkBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, _) in list {
                            info!("🕳️ Peer expired: {}", peer_id);
                        }
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        info!("✅ Connection established with {} via {}", peer_id, endpoint.get_remote_address());

                        // Broadcast to WebSocket clients
                        broadcast_ws_event(&ws_tx, WsEvent::PeerConnected {
                            peer_id: peer_id.to_string(),
                            address: endpoint.get_remote_address().to_string(),
                            timestamp: current_timestamp(),
                        });
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        info!("❌ Connection closed with {}: {:?}", peer_id, cause);

                        // Broadcast to WebSocket clients
                        broadcast_ws_event(&ws_tx, WsEvent::PeerDisconnected {
                            peer_id: peer_id.to_string(),
                            timestamp: current_timestamp(),
                        });
                    }
                    SwarmEvent::Behaviour(CoreLinkBehaviourEvent::Ping(ping::Event { peer, result, .. })) => {
                        match result {
                            Ok(rtt) => info!("🏓 Ping to {}: {:?}", peer, rtt),
                            Err(e) => info!("❌ Ping failed to {}: {:?}", peer, e),
                        }
                    }
                    SwarmEvent::Behaviour(CoreLinkBehaviourEvent::Identify(identify::Event::Received { peer_id, info })) => {
                        info!("🆔 Identified {}: {:?}", peer_id, info.protocol_version);
                    }
                    SwarmEvent::Behaviour(CoreLinkBehaviourEvent::Messaging(event)) => {
                        match event {
                            MessagingBehaviourEvent::MessageReceived { from, message } => {
                                info!("📬 Messaging event: MessageReceived from {}: {:?}", from, message.msg_type);
                            }
                            MessagingBehaviourEvent::MessageSent { to } => {
                                info!("✅ Message sent to {}", to);
                            }
                            MessagingBehaviourEvent::SendError { to, error } => {
                                info!("❌ Failed to send message to {}: {}", to, error);
                            }
                            MessagingBehaviourEvent::FileOffered { peer, metadata } => {
                                info!(
                                    "📁 File offered by {}: {} ({} bytes, {} chunks)",
                                    peer, metadata.name, metadata.size, metadata.total_chunks
                                );

                                // Broadcast to WebSocket clients
                                broadcast_ws_event(&ws_tx, WsEvent::FileOffered {
                                    peer_id: peer.to_string(),
                                    file_id: metadata.file_id.clone(),
                                    name: metadata.name.clone(),
                                    size: metadata.size,
                                    chunks: metadata.total_chunks,
                                    timestamp: current_timestamp(),
                                });

                                // Update API state
                                api_state.add_file(FileInfo {
                                    file_id: metadata.file_id.clone(),
                                    name: metadata.name.clone(),
                                    size: metadata.size,
                                    chunks: metadata.total_chunks,
                                    status: FileStatus::Downloading,
                                    progress: 0.0,
                                    peer_id: Some(peer.to_string()),
                                }).await;
                            }
                            MessagingBehaviourEvent::ChunkReceived { file_id, progress } => {
                                info!("📦 Chunk received for {}: {:.1}%", file_id, progress * 100.0);

                                // Broadcast to WebSocket clients
                                broadcast_ws_event(&ws_tx, WsEvent::ChunkReceived {
                                    file_id: file_id.clone(),
                                    chunk_index: 0, // TODO: track actual chunk index
                                    progress,
                                    timestamp: current_timestamp(),
                                });

                                // Update API state progress
                                api_state.update_file_progress(&file_id, progress).await;
                            }
                            MessagingBehaviourEvent::TransferComplete { file_id } => {
                                info!("✅ File transfer complete: {}", file_id);

                                // Broadcast to WebSocket clients
                                // TODO: Get actual name and size from file_manager
                                broadcast_ws_event(&ws_tx, WsEvent::TransferComplete {
                                    file_id: file_id.clone(),
                                    name: "unknown".to_string(),
                                    size: 0,
                                    timestamp: current_timestamp(),
                                });

                                // Update API state
                                api_state.update_file_status(&file_id, FileStatus::Complete).await;
                                api_state.update_file_progress(&file_id, 1.0).await;
                            }
                            MessagingBehaviourEvent::TransferFailed { file_id, reason } => {
                                info!("❌ File transfer failed {}: {}", file_id, reason);

                                // Broadcast to WebSocket clients
                                broadcast_ws_event(&ws_tx, WsEvent::TransferFailed {
                                    file_id: file_id.clone(),
                                    reason: reason.clone(),
                                    timestamp: current_timestamp(),
                                });

                                // Update API state
                                api_state.update_file_status(&file_id, FileStatus::Failed).await;
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ = discovery_interval.tick() => {
                let connected_peers: Vec<_> = swarm.connected_peers().cloned().collect();
                if !connected_peers.is_empty() {
                    info!("📡 Broadcasting discovery to {} peers", connected_peers.len());
                    swarm.behaviour_mut().messaging.broadcast_discovery();
                } else {
                    info!("⏳ No peers connected yet, waiting for discovery...");
                }
            }
            _ = status_interval.tick() => {
                // Update stats every 5 seconds
                let peer_count = swarm.connected_peers().count();
                let uptime_seconds = start_time.elapsed().as_secs();

                // Broadcast to WebSocket clients
                broadcast_ws_event(&ws_tx, WsEvent::NodeStatus {
                    peer_count,
                    active_uploads: 0, // TODO: get from file_manager
                    active_downloads: 0, // TODO: get from file_manager
                    timestamp: current_timestamp(),
                });

                // Update REST API state
                api_state.update_stats(NodeStats {
                    peer_count,
                    active_uploads: 0, // TODO: get from file_manager
                    active_downloads: 0, // TODO: get from file_manager
                    uptime_seconds,
                    bytes_sent: 0, // TODO: track bytes
                    bytes_received: 0, // TODO: track bytes
                }).await;

                // Update peer list in API
                let peers: Vec<PeerInfo> = swarm.connected_peers()
                    .map(|peer_id| PeerInfo {
                        peer_id: peer_id.to_string(),
                        addresses: vec![], // TODO: get actual addresses
                        connected_since: current_timestamp(), // TODO: track actual connection time
                        protocol_version: "corelink/1.0.0".to_string(),
                    })
                    .collect();
                api_state.update_peers(peers).await;
            }
            line = lines.next_line() => {
                if let Ok(Some(cmd)) = line {
                    match cmd.trim() {
                        "offer" => {
                            // Create test file if doesn't exist
                            let test_file = PathBuf::from("test.txt");
                            if !test_file.exists() {
                                std::fs::write(&test_file, b"Hello CoreLink! This is a test file.\nChunk-based transfer protocol working!\nSHA256 verification enabled.")?;
                                info!("📝 Created test.txt");
                            }
                            // Offer file
                            match swarm.behaviour_mut().messaging.offer_file(&test_file) {
                                Ok(metadata) => {
                                    info!("📤 Offering: {} ({} bytes, {} chunks)",
                                          metadata.name, metadata.size, metadata.total_chunks);
                                }
                                Err(e) => info!("❌ Failed: {}", e),
                            }
                        }
                        "help" => {
                            info!("Commands:");
                            info!("  offer - Share test.txt with connected peers");
                            info!("  help  - Show this help");
                        }
                        "" => {} // Ignore empty input
                        _ => info!("Unknown: '{}'. Type 'help'", cmd),
                    }
                }
            }
        }
    }
}

/// Broadcast an event to all connected WebSocket clients
fn broadcast_ws_event(tx: &WsEventSender, event: WsEvent) {
    if let Err(_e) = tx.send(event) {
        // No subscribers is ok, don't log error
        // Only log if there are actual subscribers who failed to receive
        if tx.receiver_count() > 0 {
            tracing::warn!("Failed to broadcast WebSocket event");
        }
    }
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
