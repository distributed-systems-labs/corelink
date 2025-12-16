use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{error, info, warn};

/// Events that are broadcast to WebSocket clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsEvent {
    /// Peer connected to the network
    PeerConnected {
        peer_id: String,
        address: String,
        timestamp: u64,
    },

    /// Peer disconnected from the network
    PeerDisconnected { peer_id: String, timestamp: u64 },

    /// File offered by a peer
    FileOffered {
        peer_id: String,
        file_id: String,
        name: String,
        size: u64,
        chunks: u32,
        timestamp: u64,
    },

    /// Chunk received during download
    ChunkReceived {
        file_id: String,
        chunk_index: u32,
        progress: f32,
        timestamp: u64,
    },

    /// File transfer completed
    TransferComplete {
        file_id: String,
        name: String,
        size: u64,
        timestamp: u64,
    },

    /// File transfer failed
    TransferFailed {
        file_id: String,
        reason: String,
        timestamp: u64,
    },

    /// Node status update
    NodeStatus {
        peer_count: usize,
        active_uploads: usize,
        active_downloads: usize,
        timestamp: u64,
    },
}

/// WebSocket event sender (clone this to broadcast events)
pub type WsEventSender = broadcast::Sender<WsEvent>;

/// Start WebSocket server on specified address
pub async fn start_websocket_server(
    addr: &str,
) -> Result<WsEventSender, Box<dyn std::error::Error>> {
    // Create broadcast channel (capacity: 100 events)
    let (tx, _rx) = broadcast::channel::<WsEvent>(100);
    let tx_clone = tx.clone();

    let listener = TcpListener::bind(addr).await?;
    info!("🌐 WebSocket server listening on {}", addr);

    // Spawn task to accept connections
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    info!("📱 WebSocket client connected: {}", peer_addr);
                    let tx = tx_clone.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, tx).await {
                            warn!("WebSocket connection error: {}", e);
                        }
                        info!("📱 WebSocket client disconnected: {}", peer_addr);
                    });
                }
                Err(e) => {
                    error!("Failed to accept WebSocket connection: {}", e);
                }
            }
        }
    });

    Ok(tx)
}

/// Handle individual WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    event_tx: WsEventSender,
) -> Result<(), Box<dyn std::error::Error>> {
    // Upgrade to WebSocket
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Subscribe to events
    let mut event_rx = event_tx.subscribe();

    // Send initial connection confirmation
    let welcome = WsEvent::NodeStatus {
        peer_count: 0,
        active_uploads: 0,
        active_downloads: 0,
        timestamp: current_timestamp(),
    };
    let msg = serde_json::to_string(&welcome)?;
    ws_sender.send(Message::Text(msg)).await?;

    // Handle both incoming messages and outgoing events
    loop {
        tokio::select! {
            // Receive event from broadcast channel
            event = event_rx.recv() => {
                match event {
                    Ok(evt) => {
                        let json = serde_json::to_string(&evt)?;
                        if let Err(e) = ws_sender.send(Message::Text(json)).await {
                            warn!("Failed to send event: {}", e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("WebSocket client lagged, skipped {} events", skipped);
                    }
                    Err(_) => break,
                }
            }

            // Receive message from WebSocket client (ping/pong)
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        ws_sender.send(Message::Pong(data)).await?;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    Some(Err(e)) => {
                        warn!("WebSocket receive error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_websocket_server_starts() {
        let result = start_websocket_server("127.0.0.1:0").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_ws_event_serialization() {
        let event = WsEvent::PeerConnected {
            peer_id: "12D3Koo...".to_string(),
            address: "/ip4/127.0.0.1/tcp/4001".to_string(),
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("PeerConnected"));
        assert!(json.contains("12D3Koo"));
    }
}
