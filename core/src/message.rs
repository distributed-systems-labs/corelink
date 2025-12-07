use crate::{NodeId, FileMetadata, FileChunk};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub from: NodeId,
    pub to: Option<NodeId>,
    pub msg_type: MessageType,
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Discovery(DiscoveryMessage),
    DataTransfer(DataMessage),
    Consensus(ConsensusMessage),
    Ping,
    Pong,
    // File transfer protocol messages
    FileOffer(FileMetadata),
    FileRequest {
        file_id: String,
        requester: NodeId,
    },
    ChunkRequest {
        file_id: String,
        chunk_index: u32,
    },
    ChunkData(FileChunk),
    ChunkRequestBatch {
        file_id: String,
        chunk_indices: Vec<u32>,
    },
    TransferComplete {
        file_id: String,
        success: bool,
    },
    TransferCancel {
        file_id: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub capabilities: Vec<String>,
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMessage {
    pub data_hash: [u8; 32],
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusMessage {
    pub proposal_id: [u8; 32],
    pub proposal_type: ProposalType,
    pub votes: Vec<Vote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    DataValidation,
    NodeAddition,
    NodeRemoval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub voter: NodeId,
    pub approve: bool,
    pub physical_proof: Option<PhysicalProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalProof {
    pub signal_strength: i32,
    pub distance_estimate: Option<f32>,
    pub timestamp: u64,
}
