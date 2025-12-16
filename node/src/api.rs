use axum::{
    extract::State,
    http::{StatusCode, Method},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

/// Shared API state
#[derive(Clone)]
pub struct ApiState {
    inner: Arc<RwLock<ApiStateInner>>,
}

struct ApiStateInner {
    stats: NodeStats,
    peers: Vec<PeerInfo>,
    files: Vec<FileInfo>,
}

impl ApiState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ApiStateInner {
                stats: NodeStats {
                    peer_count: 0,
                    active_uploads: 0,
                    active_downloads: 0,
                    uptime_seconds: 0,
                    bytes_sent: 0,
                    bytes_received: 0,
                },
                peers: Vec::new(),
                files: Vec::new(),
            })),
        }
    }

    pub async fn update_stats(&self, stats: NodeStats) {
        let mut inner = self.inner.write().await;
        inner.stats = stats;
    }

    pub async fn update_peers(&self, peers: Vec<PeerInfo>) {
        let mut inner = self.inner.write().await;
        inner.peers = peers;
    }

    pub async fn add_file(&self, file: FileInfo) {
        let mut inner = self.inner.write().await;
        // Update existing file or add new one
        if let Some(existing) = inner.files.iter_mut().find(|f| f.file_id == file.file_id) {
            *existing = file;
        } else {
            inner.files.push(file);
        }
    }

    pub async fn update_file_status(&self, file_id: &str, status: FileStatus) {
        let mut inner = self.inner.write().await;
        if let Some(file) = inner.files.iter_mut().find(|f| f.file_id == file_id) {
            file.status = status;
        }
    }

    pub async fn update_file_progress(&self, file_id: &str, progress: f32) {
        let mut inner = self.inner.write().await;
        if let Some(file) = inner.files.iter_mut().find(|f| f.file_id == file_id) {
            file.progress = progress;
        }
    }

    pub async fn get_stats(&self) -> NodeStats {
        self.inner.read().await.stats.clone()
    }

    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        self.inner.read().await.peers.clone()
    }

    pub async fn get_files(&self) -> Vec<FileInfo> {
        self.inner.read().await.files.clone()
    }
}

/// Node statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStats {
    pub peer_count: usize,
    pub active_uploads: usize,
    pub active_downloads: usize,
    pub uptime_seconds: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub connected_since: u64,
    pub protocol_version: String,
}

/// File information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub file_id: String,
    pub name: String,
    pub size: u64,
    pub chunks: u32,
    pub status: FileStatus,
    pub progress: f32,
    pub peer_id: Option<String>,
}

/// File transfer status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Offering,
    Downloading,
    Complete,
    Failed,
}

/// Request to offer a file
#[derive(Debug, Deserialize)]
pub struct OfferFileRequest {
    pub path: String,
}

/// Start the REST API server
pub async fn start_api_server(addr: &str, state: ApiState) -> Result<(), Box<dyn std::error::Error>> {
    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/peers", get(peers_handler))
        .route("/api/files", get(files_handler))
        .route("/api/files/offer", post(offer_file_handler))
        .layer(cors)
        .with_state(state);

    info!("🌐 REST API server listening on {}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint
async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "corelink-node",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Get node statistics
async fn stats_handler(State(state): State<ApiState>) -> impl IntoResponse {
    let stats = state.get_stats().await;
    Json(stats)
}

/// Get connected peers
async fn peers_handler(State(state): State<ApiState>) -> impl IntoResponse {
    let peers = state.get_peers().await;
    Json(peers)
}

/// Get files
async fn files_handler(State(state): State<ApiState>) -> impl IntoResponse {
    let files = state.get_files().await;
    Json(files)
}

/// Offer a file (placeholder - actual implementation will be in main.rs)
async fn offer_file_handler(
    State(_state): State<ApiState>,
    Json(request): Json<OfferFileRequest>,
) -> impl IntoResponse {
    // This is a placeholder - the actual file offering logic needs to be coordinated
    // with the main swarm, so this endpoint will need to send a message to the main loop
    // For now, return a simple response
    info!("📤 API request to offer file: {}", request.path);

    // TODO: Implement actual file offering via channel to main loop
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "File offering via API not yet implemented",
            "message": "Use the CLI 'offer' command for now",
        }))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_state() {
        let state = ApiState::new();

        // Test stats update
        let stats = NodeStats {
            peer_count: 2,
            active_uploads: 1,
            active_downloads: 1,
            uptime_seconds: 100,
            bytes_sent: 1024,
            bytes_received: 2048,
        };
        state.update_stats(stats.clone()).await;

        let retrieved = state.get_stats().await;
        assert_eq!(retrieved.peer_count, 2);
        assert_eq!(retrieved.bytes_sent, 1024);
    }

    #[tokio::test]
    async fn test_file_updates() {
        let state = ApiState::new();

        let file = FileInfo {
            file_id: "test123".to_string(),
            name: "test.txt".to_string(),
            size: 1024,
            chunks: 2,
            status: FileStatus::Downloading,
            progress: 0.0,
            peer_id: Some("peer1".to_string()),
        };

        state.add_file(file).await;

        // Update progress
        state.update_file_progress("test123", 0.5).await;

        let files = state.get_files().await;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].progress, 0.5);

        // Update status
        state.update_file_status("test123", FileStatus::Complete).await;

        let files = state.get_files().await;
        assert_eq!(files[0].status, FileStatus::Complete);
    }
}
