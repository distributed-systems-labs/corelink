pub mod identity;
pub mod message;
pub mod storage;
pub mod consensus;
pub mod crypto;
pub mod network;
pub mod protocol;

pub use identity::{NodeId, Identity};
pub use message::{Message, MessageType};
pub use network::{NetworkState, PeerInfo};
pub use protocol::{CoreLinkProtocol, CoreLinkCodec};

#[derive(Debug, thiserror::Error)]
pub enum CoreLinkError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Consensus error: {0}")]
    Consensus(String),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Crypto error: {0}")]
    Crypto(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CoreLinkError>;