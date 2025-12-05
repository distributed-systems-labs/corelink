use crate::protocol_handler::{CoreLinkHandler, CoreLinkHandlerEvent};
use corelink_core::identity::NodeId;
use corelink_core::message::{DiscoveryMessage, Message, MessageType};
use libp2p_core::{Endpoint, Multiaddr};
use libp2p_identity::PeerId;
use libp2p_swarm::{
    ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, NotifyHandler, THandler,
    THandlerInEvent, THandlerOutEvent, ToSwarm,
};
use std::collections::{HashMap, VecDeque};
use std::task::{Context, Poll};
use tracing::info;

#[derive(Debug)]
pub enum MessagingBehaviourEvent {
    MessageReceived { from: PeerId, message: Message },
    MessageSent { to: PeerId },
    SendError { to: PeerId, error: String },
}

pub struct MessagingBehaviour {
    connected_peers: HashMap<PeerId, Vec<ConnectionId>>,
    pending_handler_messages: VecDeque<(PeerId, Message)>,
    pending_events: VecDeque<MessagingBehaviourEvent>,
}

impl MessagingBehaviour {
    pub fn new() -> Self {
        Self {
            connected_peers: HashMap::new(),
            pending_handler_messages: VecDeque::new(),
            pending_events: VecDeque::new(),
        }
    }

    pub fn send_message(&mut self, peer: PeerId, message: Message) {
        info!("Queueing message to peer: {}", peer);
        self.pending_handler_messages.push_back((peer, message));
    }

    pub fn broadcast_discovery(&mut self) {
        let peers: Vec<PeerId> = self.connected_peers.keys().copied().collect();
        info!("📡 Broadcasting discovery to {} peers", peers.len());

        let discovery_data = DiscoveryMessage {
            capabilities: vec!["storage".to_string(), "compute".to_string()],
            protocol_version: "1.0.0".to_string(),
        };

        // Dummy NodeId - ideally this would be the real node's ID
        let dummy_pubkey = ed25519_dalek::VerifyingKey::from_bytes(&[0u8; 32]).unwrap();

        let discovery_msg = Message {
            msg_type: MessageType::Discovery(discovery_data),
            from: NodeId::from_pubkey(&dummy_pubkey),
            to: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: vec![],
        };

        for peer in peers {
            self.send_message(peer, discovery_msg.clone());
        }
    }
}

impl NetworkBehaviour for MessagingBehaviour {
    type ConnectionHandler = CoreLinkHandler;
    type ToSwarm = MessagingBehaviourEvent;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        info!("🔵 Creating handler for inbound connection");
        Ok(CoreLinkHandler::new())
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _addr: &Multiaddr,
        _role_override: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        info!("🔴 Creating handler for outbound connection");
        Ok(CoreLinkHandler::new())
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        if let FromSwarm::ConnectionEstablished(e) = event {
            info!(
                "Established {} connection with {}",
                if e.endpoint.is_dialer() {
                    "outbound"
                } else {
                    "inbound"
                },
                e.peer_id
            );

            self.connected_peers
                .entry(e.peer_id)
                .or_default()
                .push(e.connection_id);
        } else if let FromSwarm::ConnectionClosed(e) = event {
            if let Some(conns) = self.connected_peers.get_mut(&e.peer_id) {
                conns.retain(|id| id != &e.connection_id);
                if conns.is_empty() {
                    self.connected_peers.remove(&e.peer_id);
                    info!("All connections closed with {}", e.peer_id);
                }
            }
        }
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        _connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match event {
            CoreLinkHandlerEvent::MessageReceived(msg) => {
                info!("📨 Received message from {}: {:?}", peer_id, msg.msg_type);
                self.pending_events
                    .push_back(MessagingBehaviourEvent::MessageReceived {
                        from: peer_id,
                        message: msg,
                    });
            }
            CoreLinkHandlerEvent::MessageSent => {
                info!("✅ Message sent to {}", peer_id);
                self.pending_events
                    .push_back(MessagingBehaviourEvent::MessageSent { to: peer_id });
            }
            CoreLinkHandlerEvent::SendError(error) => {
                info!("❌ Failed to send message to {}: {}", peer_id, error);
                self.pending_events
                    .push_back(MessagingBehaviourEvent::SendError { to: peer_id, error });
            }
        }
    }

    fn poll(&mut self, _cx: &mut Context) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // First emit any pending events to the swarm
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ToSwarm::GenerateEvent(event));
        }

        // Then handle sending messages to handlers
        if let Some((peer, message)) = self.pending_handler_messages.pop_front() {
            return Poll::Ready(ToSwarm::NotifyHandler {
                peer_id: peer,
                handler: NotifyHandler::Any,
                event: message,
            });
        }

        Poll::Pending
    }
}
