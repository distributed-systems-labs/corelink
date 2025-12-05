use crate::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PeerInfo {
    pub node_id: NodeId,
    pub address: String,
    pub last_seen: u64,
    pub capabilities: Vec<String>,
}

#[derive(Default)]
pub struct NetworkState {
    peers: Arc<RwLock<HashMap<NodeId, PeerInfo>>>,
}

impl NetworkState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn add_peer(&self, peer: PeerInfo) {
        let mut peers = self.peers.write().await;
        peers.insert(peer.node_id, peer);
    }

    pub async fn remove_peer(&self, node_id: &NodeId) {
        let mut peers = self.peers.write().await;
        peers.remove(node_id);
    }

    pub async fn get_peer(&self, node_id: &NodeId) -> Option<PeerInfo> {
        let peers = self.peers.read().await;
        peers.get(node_id).cloned()
    }

    pub async fn get_all_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }
}

impl Clone for PeerInfo {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id,
            address: self.address.clone(),
            last_seen: self.last_seen,
            capabilities: self.capabilities.clone(),
        }
    }
}
