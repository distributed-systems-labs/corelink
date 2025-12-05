mod protocol_handler;
mod messaging_behaviour;

use clap::Parser;
use futures::StreamExt;
use libp2p_core::{Multiaddr, Transport as _};
use libp2p_identity::Keypair;
use libp2p_mdns as mdns;
use libp2p_noise as noise;
use libp2p_ping as ping;
use libp2p_swarm::{NetworkBehaviour, Swarm, SwarmEvent};
use libp2p_tcp as tcp;
use libp2p_yamux as yamux;
use messaging_behaviour::{MessagingBehaviour, MessagingEvent};
use std::error::Error;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "corelink-node")]
#[command(about = "CoreLink mesh network node")]
struct Cli {
    #[arg(short, long, default_value = "0")]
    port: u16,
    
    #[arg(short, long)]
    name: Option<String>,
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "CoreLinkEvent")]
struct CoreLinkBehaviour {
    mdns: mdns::tokio::Behaviour,
    ping: ping::Behaviour,
    messaging: MessagingBehaviour,
}

#[derive(Debug)]
enum CoreLinkEvent {
    Mdns(mdns::Event),
    Ping(ping::Event),
    Messaging(MessagingEvent),
}

impl From<mdns::Event> for CoreLinkEvent {
    fn from(event: mdns::Event) -> Self {
        CoreLinkEvent::Mdns(event)
    }
}

impl From<ping::Event> for CoreLinkEvent {
    fn from(event: ping::Event) -> Self {
        CoreLinkEvent::Ping(event)
    }
}

impl From<MessagingEvent> for CoreLinkEvent {
    fn from(event: MessagingEvent) -> Self {
        CoreLinkEvent::Messaging(event)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    info!("Starting CoreLink node...");
    
    let local_key = Keypair::generate_ed25519();
    let local_peer_id = local_key.public().to_peer_id();
    info!("Node ID: {}", local_peer_id);
    
    let transport = tcp::tokio::Transport::new(tcp::Config::default())
        .upgrade(libp2p_core::upgrade::Version::V1Lazy)
        .authenticate(noise::Config::new(&local_key)?)
        .multiplex(yamux::Config::default())
        .boxed();
    
    let behaviour = CoreLinkBehaviour {
        mdns: mdns::tokio::Behaviour::new(
            mdns::Config::default(),
            local_peer_id,
        )?,
        ping: ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(5))),
        messaging: MessagingBehaviour::new(),
    };
    
    let mut swarm = Swarm::new(
        transport,
        behaviour,
        local_peer_id,
        libp2p_swarm::Config::with_executor(Box::new(|fut| {
            tokio::spawn(fut);
        })).with_idle_connection_timeout(Duration::from_secs(60)),
    );
    
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", cli.port).parse()?;
    swarm.listen_on(listen_addr)?;
    
    info!("Node started on port {}", cli.port);
    
    let mut discovery_interval = tokio::time::interval(Duration::from_secs(10));
    
    loop {
        tokio::select! {
            _ = discovery_interval.tick() => {
                info!("Broadcasting discovery message...");
                swarm.behaviour_mut().messaging.broadcast_discovery();
            }
            
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on {}", address);
                    }
                    SwarmEvent::Behaviour(CoreLinkEvent::Mdns(event)) => {
                        match event {
                            mdns::Event::Discovered(peers) => {
                                for (peer_id, addr) in peers {
                                    info!("Discovered peer: {} at {}", peer_id, addr);
                                    
                                    if let Err(e) = swarm.dial(addr.clone()) {
                                        warn!("Failed to dial peer {}: {}", peer_id, e);
                                    } else {
                                        info!("Dialing peer: {}", peer_id);
                                    }
                                }
                            }
                            mdns::Event::Expired(peers) => {
                                for (peer_id, addr) in peers {
                                    warn!("Peer expired: {} at {}", peer_id, addr);
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(CoreLinkEvent::Ping(event)) => {
                        info!("🏓 Ping: {:?}", event);
                    }
                    SwarmEvent::Behaviour(CoreLinkEvent::Messaging(event)) => {
                        match event {
                            MessagingEvent::MessageReceived { peer, message } => {
                                info!("💬 Message from {}: {:?}", peer, message.msg_type);
                            }
                            MessagingEvent::MessageSent { peer } => {
                                info!("✅ Message sent to {}", peer);
                            }
                        }
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        info!("✅ Connected to peer: {} via {}", peer_id, endpoint.get_remote_address());
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        info!("❌ Connection closed with {}: {:?}", peer_id, cause);
                    }
                    _ => {}
                }
            }
        }
    }
}