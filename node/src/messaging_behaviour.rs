use crate::protocol_handler::{CoreLinkHandler, CoreLinkHandlerEvent};
use corelink_core::{Message, MessageType, message::DiscoveryMessage, Identity};
use libp2p_identity::PeerId;
use libp2p_swarm::{NetworkBehaviour, ToSwarm};
use std::collections::{HashMap, VecDeque};
use std::task::{Context, Poll};
use tracing::info;

#[derive(Debug)]
pub enum MessagingEvent {
    MessageReceived { peer: PeerId, message: Message },
    MessageSent { peer: PeerId },
}

pub struct MessagingBehaviour {
    events: VecDeque<MessagingEvent>,
    connected_peers: HashMap<PeerId, ()>,
    identity: Identity,
}

impl MessagingBehaviour {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            connected_peers: HashMap::new(),
            identity: Identity::generate(),
        }
    }

    pub fn send_message(&mut self, peer: PeerId, _message: Message) {
        info!("Queueing message to peer: {}", peer);
        self.events.push_back(MessagingEvent::MessageSent { peer });
    }

    pub fn broadcast_discovery(&mut self) {
        let discovery = Message {
            from: self.identity.node_id(),
            to: None,
            msg_type: MessageType::Discovery(DiscoveryMessage {
                capabilities: vec!["storage".to_string(), "compute".to_string()],
                protocol_version: "1.0.0".to_string(),
            }),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: vec![],
        };

        let peers: Vec<PeerId> = self.connected_peers.keys().copied().collect();
        for peer in peers {
            self.send_message(peer, discovery.clone());
        }
    }
}

impl NetworkBehaviour for MessagingBehaviour {
    type ConnectionHandler = CoreLinkHandler;
    type ToSwarm = MessagingEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: libp2p_swarm::ConnectionId,
        peer: PeerId,
        _local_addr: &libp2p_core::Multiaddr,
        _remote_addr: &libp2p_core::Multiaddr,
    ) -> Result<libp2p_swarm::THandler<Self>, libp2p_swarm::ConnectionDenied> {
        info!("Established inbound connection with {}", peer);
        self.connected_peers.insert(peer, ());
        Ok(CoreLinkHandler::new())
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: libp2p_swarm::ConnectionId,
        peer: PeerId,
        _addr: &libp2p_core::Multiaddr,
        _role_override: libp2p_core::Endpoint,
    ) -> Result<libp2p_swarm::THandler<Self>, libp2p_swarm::ConnectionDenied> {
        info!("Established outbound connection with {}", peer);
        self.connected_peers.insert(peer, ());
        Ok(CoreLinkHandler::new())
    }

    fn on_swarm_event(&mut self, event: libp2p_swarm::FromSwarm) {
        match event {
            libp2p_swarm::FromSwarm::ConnectionClosed(e) => {
                self.connected_peers.remove(&e.peer_id);
            }
            _ => {}
        }
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        _connection_id: libp2p_swarm::ConnectionId,
        event: libp2p_swarm::THandlerOutEvent<Self>,
    ) {
        match event {
            CoreLinkHandlerEvent::MessageReceived(msg) => {
                self.events.push_back(MessagingEvent::MessageReceived {
                    peer: peer_id,
                    message: msg,
                });
            }
            CoreLinkHandlerEvent::MessageSent => {
                info!("Message sent to {}", peer_id);
            }
        }
    }

    fn poll(
        &mut self,
        _cx: &mut Context,
    ) -> Poll<ToSwarm<Self::ToSwarm, libp2p_swarm::THandlerInEvent<Self>>> {
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }
        Poll::Pending
    }
}