use leptos::*;
use serde::{Deserialize, Serialize};
use gloo_net::websocket::{futures::WebSocket, Message};
use futures::StreamExt;

/// WebSocket events from CoreLink node
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsEvent {
    PeerConnected {
        peer_id: String,
        address: String,
        timestamp: u64,
    },
    PeerDisconnected {
        peer_id: String,
        timestamp: u64,
    },
    FileOffered {
        peer_id: String,
        file_id: String,
        name: String,
        size: u64,
        chunks: u32,
        timestamp: u64,
    },
    ChunkReceived {
        file_id: String,
        chunk_index: u32,
        progress: f32,
        timestamp: u64,
    },
    TransferComplete {
        file_id: String,
        name: String,
        size: u64,
        timestamp: u64,
    },
    TransferFailed {
        file_id: String,
        reason: String,
        timestamp: u64,
    },
    NodeStatus {
        peer_count: usize,
        active_uploads: usize,
        active_downloads: usize,
        timestamp: u64,
    },
}

/// WebSocket connection status
#[derive(Debug, Clone, PartialEq)]
pub enum WsStatus {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

/// Hook for WebSocket connection to CoreLink node
#[component]
pub fn UseWebSocket(
    /// WebSocket URL (e.g., "ws://localhost:8001")
    url: String,
    /// Callback for received events
    on_event: Callback<WsEvent>,
) -> impl IntoView {
    let (status, set_status) = create_signal(WsStatus::Connecting);

    // Connect to WebSocket on mount
    create_effect(move |_| {
        let url = url.clone();

        spawn_local(async move {
            match WebSocket::open(&url) {
                Ok(ws) => {
                    set_status.set(WsStatus::Connected);
                    let (_write, mut read) = ws.split();

                    // Read messages from WebSocket
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Ok(event) = serde_json::from_str::<WsEvent>(&text) {
                                    on_event.call(event);
                                } else {
                                    logging::warn!("Failed to parse WebSocket event: {}", text);
                                }
                            }
                            Err(e) => {
                                logging::error!("WebSocket error: {:?}", e);
                                set_status.set(WsStatus::Error(e.to_string()));
                                break;
                            }
                            _ => {}
                        }
                    }

                    set_status.set(WsStatus::Disconnected);
                }
                Err(e) => {
                    logging::error!("Failed to connect to WebSocket: {:?}", e);
                    set_status.set(WsStatus::Error(e.to_string()));
                }
            }
        });
    });

    view! {
        <div class="websocket-status">
            {move || match status.get() {
                WsStatus::Connecting => view! { <span>"🔄 Connecting..."</span> }.into_view(),
                WsStatus::Connected => view! { <span class="text-green-500">"✅ Connected"</span> }.into_view(),
                WsStatus::Disconnected => view! { <span class="text-yellow-500">"⚠️ Disconnected"</span> }.into_view(),
                WsStatus::Error(e) => view! { <span class="text-red-500">"❌ Error: " {e}</span> }.into_view(),
            }}
        </div>
    }
}
