use serde::{Deserialize, Serialize};

/// Node statistics from REST API
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeStats {
    pub peer_count: usize,
    pub active_uploads: usize,
    pub active_downloads: usize,
    pub uptime_seconds: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Peer information from REST API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub connected_since: u64,
    pub protocol_version: String,
}

/// File information from REST API
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Offering,
    Downloading,
    Complete,
    Failed,
}

/// Hook for REST API calls to CoreLink node
pub fn use_corelink_api(base_url: String) -> CoreLinkApi {
    CoreLinkApi { base_url }
}

#[derive(Clone)]
pub struct CoreLinkApi {
    base_url: String,
}

impl CoreLinkApi {
    /// Get node statistics
    pub async fn get_stats(&self) -> Result<NodeStats, String> {
        let url = format!("{}/api/stats", self.base_url);

        gloo_net::http::Request::get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<NodeStats>()
            .await
            .map_err(|e| e.to_string())
    }

    /// Get connected peers
    pub async fn get_peers(&self) -> Result<Vec<PeerInfo>, String> {
        let url = format!("{}/api/peers", self.base_url);

        gloo_net::http::Request::get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<Vec<PeerInfo>>()
            .await
            .map_err(|e| e.to_string())
    }

    /// Get files list
    pub async fn get_files(&self) -> Result<Vec<FileInfo>, String> {
        let url = format!("{}/api/files", self.base_url);

        gloo_net::http::Request::get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<Vec<FileInfo>>()
            .await
            .map_err(|e| e.to_string())
    }
}
