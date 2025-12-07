pub mod consensus;
pub mod crypto;
pub mod file;
pub mod identity;
pub mod message;
pub mod network;
pub mod protocol;
pub mod storage;

pub use file::{FileMetadata, FileChunk, FileTransfer};
pub use identity::{Identity, NodeId};
pub use message::{Message, MessageType};
pub use network::{NetworkState, PeerInfo};
pub use protocol::{CoreLinkCodec, CoreLinkProtocol};

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
