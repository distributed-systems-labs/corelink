use crate::file_transfer::{FileTransferManager, TransferStatus};
use crate::protocol_handler::{CoreLinkHandler, CoreLinkHandlerEvent};
use corelink_core::file::FileMetadata;
use corelink_core::identity::NodeId;
use corelink_core::message::{DiscoveryMessage, Message, MessageType};
use libp2p_core::{Endpoint, Multiaddr};
use libp2p_identity::PeerId;
use libp2p_swarm::{
    ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, NotifyHandler, THandler,
    THandlerInEvent, THandlerOutEvent, ToSwarm,
};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::{Path, PathBuf};
use std::task::{Context, Poll};
use tracing::{error, info, warn};

#[derive(Debug)]
pub enum MessagingBehaviourEvent {
    MessageReceived {
        from: PeerId,
        message: Message,
    },
    MessageSent {
        to: PeerId,
    },
    SendError {
        to: PeerId,
        error: String,
    },
    // File transfer events
    FileOffered {
        peer: PeerId,
        metadata: FileMetadata,
    },
    ChunkReceived {
        file_id: String,
        progress: f32,
    },
    TransferComplete {
        file_id: String,
    },
    TransferFailed {
        file_id: String,
        reason: String,
    },
}

pub struct MessagingBehaviour {
    connected_peers: HashMap<PeerId, Vec<ConnectionId>>,
    pending_handler_messages: VecDeque<(PeerId, Message)>,
    pending_events: VecDeque<MessagingBehaviourEvent>,
    file_manager: FileTransferManager,
}

impl MessagingBehaviour {
    pub fn new() -> io::Result<Self> {
        let file_manager = FileTransferManager::new(PathBuf::from("./storage"))?;
        Ok(Self {
            connected_peers: HashMap::new(),
            pending_handler_messages: VecDeque::new(),
            pending_events: VecDeque::new(),
            file_manager,
        })
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

    /// Offer a file for transfer to the network
    pub fn offer_file(&mut self, path: &Path) -> io::Result<FileMetadata> {
        let metadata = self.file_manager.offer_file(path)?;
        info!(
            "📤 Offering file: {} ({} bytes, {} chunks)",
            metadata.name, metadata.size, metadata.total_chunks
        );

        // Broadcast file offer to all connected peers
        let peers: Vec<PeerId> = self.connected_peers.keys().copied().collect();
        let dummy_pubkey = ed25519_dalek::VerifyingKey::from_bytes(&[0u8; 32]).unwrap();

        for peer in peers {
            let offer_msg = Message {
                msg_type: MessageType::FileOffer(metadata.clone()),
                from: NodeId::from_pubkey(&dummy_pubkey),
                to: None,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                signature: vec![],
            };
            self.send_message(peer, offer_msg);
        }

        Ok(metadata)
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

                // Handle file transfer messages
                match &msg.msg_type {
                    MessageType::FileOffer(metadata) => {
                        info!(
                            "📁 File offered by {}: {} ({} bytes)",
                            peer_id, metadata.name, metadata.size
                        );
                        self.pending_events
                            .push_back(MessagingBehaviourEvent::FileOffered {
                                peer: peer_id,
                                metadata: metadata.clone(),
                            });
                    }
                    MessageType::ChunkRequest {
                        file_id,
                        chunk_index,
                    } => {
                        // Handle chunk request - serve the chunk
                        match self
                            .file_manager
                            .handle_chunk_request(file_id, *chunk_index)
                        {
                            Ok(Some(chunk)) => {
                                let dummy_pubkey =
                                    ed25519_dalek::VerifyingKey::from_bytes(&[0u8; 32]).unwrap();
                                let chunk_msg = Message {
                                    msg_type: MessageType::ChunkData(chunk),
                                    from: NodeId::from_pubkey(&dummy_pubkey),
                                    to: None,
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                    signature: vec![],
                                };
                                self.send_message(peer_id, chunk_msg);
                            }
                            Ok(None) => {
                                warn!("Chunk {} not found for file {}", chunk_index, file_id);
                            }
                            Err(e) => {
                                error!("Failed to handle chunk request for {}: {}", file_id, e);
                            }
                        }
                    }
                    MessageType::ChunkData(chunk) => {
                        // Handle received chunk
                        let file_id = chunk.file_id.clone();
                        match self.file_manager.handle_chunk_received(chunk.clone()) {
                            Ok(TransferStatus::ChunkReceived { progress }) => {
                                info!(
                                    "📦 Chunk received for {}: {:.1}%",
                                    file_id,
                                    progress * 100.0
                                );
                                self.pending_events.push_back(
                                    MessagingBehaviourEvent::ChunkReceived {
                                        file_id: file_id.clone(),
                                        progress,
                                    },
                                );

                                // Request next batch of chunks
                                let chunks_to_request =
                                    self.file_manager.get_next_chunks_to_request(&file_id, 5);
                                if !chunks_to_request.is_empty() {
                                    let dummy_pubkey =
                                        ed25519_dalek::VerifyingKey::from_bytes(&[0u8; 32])
                                            .unwrap();
                                    for chunk_index in chunks_to_request {
                                        let request_msg = Message {
                                            msg_type: MessageType::ChunkRequest {
                                                file_id: file_id.clone(),
                                                chunk_index,
                                            },
                                            from: NodeId::from_pubkey(&dummy_pubkey),
                                            to: None,
                                            timestamp: std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_secs(),
                                            signature: vec![],
                                        };
                                        self.send_message(peer_id, request_msg);
                                    }
                                }
                            }
                            Ok(TransferStatus::TransferComplete) => {
                                info!("✅ Transfer complete: {}", file_id);
                                self.pending_events.push_back(
                                    MessagingBehaviourEvent::TransferComplete {
                                        file_id: file_id.clone(),
                                    },
                                );

                                // Send completion acknowledgment
                                let dummy_pubkey =
                                    ed25519_dalek::VerifyingKey::from_bytes(&[0u8; 32]).unwrap();
                                let complete_msg = Message {
                                    msg_type: MessageType::TransferComplete {
                                        file_id,
                                        success: true,
                                    },
                                    from: NodeId::from_pubkey(&dummy_pubkey),
                                    to: None,
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                    signature: vec![],
                                };
                                self.send_message(peer_id, complete_msg);
                            }
                            Ok(TransferStatus::VerificationFailed { chunk_index }) => {
                                error!(
                                    "❌ Chunk verification failed: {} chunk {}",
                                    file_id, chunk_index
                                );
                                self.pending_events.push_back(
                                    MessagingBehaviourEvent::TransferFailed {
                                        file_id: file_id.clone(),
                                        reason: format!(
                                            "Chunk {} verification failed",
                                            chunk_index
                                        ),
                                    },
                                );

                                // Send cancellation message
                                let dummy_pubkey =
                                    ed25519_dalek::VerifyingKey::from_bytes(&[0u8; 32]).unwrap();
                                let cancel_msg = Message {
                                    msg_type: MessageType::TransferCancel {
                                        file_id: file_id.clone(),
                                        reason: format!(
                                            "Chunk {} verification failed",
                                            chunk_index
                                        ),
                                    },
                                    from: NodeId::from_pubkey(&dummy_pubkey),
                                    to: None,
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                    signature: vec![],
                                };
                                self.send_message(peer_id, cancel_msg);
                            }
                            Err(e) => {
                                error!("Failed to handle chunk: {}", e);
                                self.pending_events.push_back(
                                    MessagingBehaviourEvent::TransferFailed {
                                        file_id,
                                        reason: e.to_string(),
                                    },
                                );
                            }
                        }
                    }
                    _ => {
                        // Other message types - emit as generic MessageReceived
                        self.pending_events
                            .push_back(MessagingBehaviourEvent::MessageReceived {
                                from: peer_id,
                                message: msg,
                            });
                    }
                }
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
