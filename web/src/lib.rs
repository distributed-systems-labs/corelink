mod hooks;

use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use hooks::{use_websocket::*, use_api::*};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App/> })
}

#[component]
fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/corelink-web.css"/>
        <Title text="CoreLink Dashboard"/>
        <Router>
            <main>
                <Routes>
                    <Route path="" view=HomePage/>
                    <Route path="/nodes" view=NodesPage/>
                    <Route path="/network" view=NetworkPage/>
                    <Route path="/storage" view=StoragePage/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    // Configuration - default to node on port 4001
    let ws_url = "ws://localhost:8001".to_string();
    let api_url = "http://localhost:7001".to_string();

    // API client
    let api = use_corelink_api(api_url.clone());
    let api_for_init = api.clone();
    let api_for_events = api.clone();

    // State from API
    let (stats, set_stats) = create_signal(NodeStats::default());
    let (peers, set_peers) = create_signal(Vec::<PeerInfo>::new());
    let (files, set_files) = create_signal(Vec::<FileInfo>::new());

    // State from WebSocket events
    let (events, set_events) = create_signal(Vec::<WsEvent>::new());

    // Fetch initial data
    create_effect(move |_| {
        let api = api_for_init.clone();
        spawn_local(async move {
            if let Ok(s) = api.get_stats().await {
                set_stats.set(s);
            }
            if let Ok(p) = api.get_peers().await {
                set_peers.set(p);
            }
            if let Ok(f) = api.get_files().await {
                set_files.set(f);
            }
        });
    });

    // Handle WebSocket events
    let on_event = Callback::new(move |event: WsEvent| {
        logging::log!("WebSocket event: {:?}", event);

        // Add to event log
        set_events.update(|events| {
            events.push(event.clone());
            if events.len() > 50 {
                events.remove(0); // Keep last 50 events
            }
        });

        // Update state based on event
        match event {
            WsEvent::PeerConnected { .. } | WsEvent::PeerDisconnected { .. } => {
                // Refresh peers list
                let api = api_for_events.clone();
                spawn_local(async move {
                    if let Ok(p) = api.get_peers().await {
                        set_peers.set(p);
                    }
                });
            }
            WsEvent::FileOffered { .. } | WsEvent::TransferComplete { .. } => {
                // Refresh files list
                let api = api_for_events.clone();
                spawn_local(async move {
                    if let Ok(f) = api.get_files().await {
                        set_files.set(f);
                    }
                });
            }
            WsEvent::NodeStatus { peer_count, active_uploads, active_downloads, .. } => {
                // Update stats directly
                set_stats.update(|s| {
                    s.peer_count = peer_count;
                    s.active_uploads = active_uploads;
                    s.active_downloads = active_downloads;
                });
            }
            _ => {}
        }
    });

    view! {
        <div class="container">
            <h1>"CoreLink Dashboard"</h1>

            // WebSocket connection status
            <UseWebSocket url=ws_url on_event=on_event />

            // Node statistics
            <div class="stats">
                <StatCard title="Connected Peers" value=move || stats.get().peer_count.to_string()/>
                <StatCard title="Active Uploads" value=move || stats.get().active_uploads.to_string()/>
                <StatCard title="Active Downloads" value=move || stats.get().active_downloads.to_string()/>
                <StatCard title="Uptime" value=move || format!("{}s", stats.get().uptime_seconds)/>
            </div>

            <nav class="nav-links">
                <A href="/nodes">"Nodes"</A>
                <A href="/network">"Network Topology"</A>
                <A href="/storage">"Storage"</A>
            </nav>

            // Connected peers section
            <div class="section">
                <h2>"Connected Peers"</h2>
                <div class="peers-list">
                    {move || {
                        let peer_list = peers.get();
                        if peer_list.is_empty() {
                            view! { <p class="empty-message">"No peers connected"</p> }.into_view()
                        } else {
                            peer_list.into_iter().map(|peer| {
                                view! {
                                    <div class="peer-card">
                                        <div class="peer-id">{peer.peer_id.clone()}</div>
                                        <div class="peer-protocol">{peer.protocol_version.clone()}</div>
                                    </div>
                                }
                            }).collect_view()
                        }
                    }}
                </div>
            </div>

            // Files section
            <div class="section">
                <h2>"Files"</h2>
                <div class="files-list">
                    {move || {
                        let file_list = files.get();
                        if file_list.is_empty() {
                            view! { <p class="empty-message">"No files"</p> }.into_view()
                        } else {
                            file_list.into_iter().map(|file| {
                                let progress_width = format!("{}%", file.progress * 100.0);
                                view! {
                                    <div class="file-card">
                                        <div class="file-name">{file.name.clone()}</div>
                                        <div class="file-status">{format!("{:?}", file.status)}</div>
                                        <div class="file-progress">
                                            <div class="progress-bar">
                                                <div class="progress-fill" style:width={progress_width}></div>
                                            </div>
                                            <span>{format!("{:.1}%", file.progress * 100.0)}</span>
                                        </div>
                                        <div class="file-size">{format_bytes(file.size)}</div>
                                    </div>
                                }
                            }).collect_view()
                        }
                    }}
                </div>
            </div>

            // Event log
            <div class="section">
                <h2>"Recent Events"</h2>
                <div class="events-list">
                    {move || {
                        let event_list = events.get();
                        if event_list.is_empty() {
                            view! { <p class="empty-message">"No events yet"</p> }.into_view()
                        } else {
                            event_list.iter().rev().take(10).map(|event| {
                                view! {
                                    <div class="event-item">
                                        {format_event(event)}
                                    </div>
                                }
                            }).collect_view()
                        }
                    }}
                </div>
            </div>
        </div>
    }
}

#[component]
fn StatCard<F>(title: &'static str, value: F) -> impl IntoView
where
    F: Fn() -> String + 'static,
{
    view! {
        <div class="stat-card">
            <h3>{title}</h3>
            <p class="stat-value">{value}</p>
        </div>
    }
}

#[component]
fn NodesPage() -> impl IntoView {
    view! {
        <div class="container">
            <h1>"Network Nodes"</h1>
            <p>"Configure and monitor your CoreLink nodes"</p>
            <div class="node-config">
                <label>"API URL: " <input type="text" placeholder="http://localhost:7001" /></label>
                <label>"WebSocket URL: " <input type="text" placeholder="ws://localhost:8001" /></label>
                <button>"Connect"</button>
            </div>
        </div>
    }
}

#[component]
fn NetworkPage() -> impl IntoView {
    view! {
        <div class="container">
            <h1>"Network Topology"</h1>
            <div class="topology-view">
                <p>"Network visualization will appear here"</p>
            </div>
        </div>
    }
}

#[component]
fn StoragePage() -> impl IntoView {
    view! {
        <div class="container">
            <h1>"Distributed Storage"</h1>
            <div class="storage-view">
                <p>"Storage management interface"</p>
            </div>
        </div>
    }
}

// Helper functions

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_event(event: &WsEvent) -> String {
    match event {
        WsEvent::PeerConnected { peer_id, .. } => {
            format!("✅ Peer connected: {}", &peer_id[..12])
        }
        WsEvent::PeerDisconnected { peer_id, .. } => {
            format!("❌ Peer disconnected: {}", &peer_id[..12])
        }
        WsEvent::FileOffered { name, size, .. } => {
            format!("📁 File offered: {} ({})", name, format_bytes(*size))
        }
        WsEvent::ChunkReceived { file_id, progress, .. } => {
            format!("📦 Chunk received: {} ({:.1}%)", &file_id[..8], progress * 100.0)
        }
        WsEvent::TransferComplete { name, .. } => {
            format!("✅ Transfer complete: {}", name)
        }
        WsEvent::TransferFailed { file_id, reason, .. } => {
            format!("❌ Transfer failed: {} - {}", &file_id[..8], reason)
        }
        WsEvent::NodeStatus { peer_count, .. } => {
            format!("📊 Status update: {} peers", peer_count)
        }
    }
}
