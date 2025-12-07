mod file_transfer;
mod messaging_behaviour;
mod protocol_handler;

use futures::StreamExt;
use libp2p::{
    identify, identity, mdns, noise, ping, swarm::SwarmEvent, tcp, yamux, Multiaddr,
    SwarmBuilder,
};
use std::error::Error;
use std::time::Duration;
use tokio::time;
use tracing::{info, Level};

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
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args.iter()
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
        .with_behaviour(|key| -> Result<CoreLinkBehaviour, Box<dyn Error + Send + Sync>> {
            let peer_id = key.public().to_peer_id();
            Ok(CoreLinkBehaviour {
                ping: ping::Behaviour::new(ping::Config::new()),
                identify: identify::Behaviour::new(identify::Config::new(
                    "/corelink/1.0.0".to_string(),
                    key.public(),
                )),
                mdns: mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    peer_id,
                )?,
                messaging: MessagingBehaviour::new()?,
            })
        })?
        .with_swarm_config(|c| {
            c.with_idle_connection_timeout(Duration::from_secs(60))
                .with_per_connection_event_buffer_size(64)
        })
        .build();

    // Listen on all interfaces
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", port).parse()?;
    swarm.listen_on(listen_addr.clone())?;

    info!("👂 Listening on {}", listen_addr);

    // Discovery broadcast interval
    let mut discovery_interval = time::interval(Duration::from_secs(10));

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
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        info!("❌ Connection closed with {}: {:?}", peer_id, cause);
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
                            }
                            MessagingBehaviourEvent::ChunkReceived { file_id, progress } => {
                                info!("📦 Chunk received for {}: {:.1}%", file_id, progress * 100.0);
                            }
                            MessagingBehaviourEvent::TransferComplete { file_id } => {
                                info!("✅ File transfer complete: {}", file_id);
                            }
                            MessagingBehaviourEvent::TransferFailed { file_id, reason } => {
                                info!("❌ File transfer failed {}: {}", file_id, reason);
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
        }
    }
}