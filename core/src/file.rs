use libp2p_identity::PeerId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const DEFAULT_CHUNK_SIZE: u32 = 64 * 1024; // 64KB

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileMetadata {
    pub file_id: String,
    pub name: String,
    pub size: u64,
    pub chunk_size: u32,
    pub total_chunks: u32,
    pub chunk_hashes: Vec<[u8; 32]>,
    pub mime_type: Option<String>,
    pub created_at: u64,
}

impl FileMetadata {
    pub fn new(name: String, size: u64, chunk_hashes: Vec<[u8; 32]>) -> Self {
        let chunk_size = DEFAULT_CHUNK_SIZE;
        let total_chunks = chunk_hashes.len() as u32; // Use actual number of chunk hashes
        let file_id = uuid::Uuid::new_v4().to_string();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            file_id,
            name,
            size,
            chunk_size,
            total_chunks,
            chunk_hashes,
            mime_type: None,
            created_at,
        }
    }

    pub fn with_mime_type(mut self, mime_type: String) -> Self {
        self.mime_type = Some(mime_type);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub file_id: String,
    pub chunk_index: u32,
    pub data: Vec<u8>,
    pub hash: [u8; 32],
}

impl FileChunk {
    pub fn new(file_id: String, chunk_index: u32, data: Vec<u8>) -> Self {
        let hash = calculate_chunk_hash(&data);
        Self {
            file_id,
            chunk_index,
            data,
            hash,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileTransfer {
    pub metadata: FileMetadata,
    pub downloaded_chunks: HashSet<u32>,
    pub missing_chunks: Vec<u32>,
    pub output_path: PathBuf,
    pub progress: f32,
    pub started_at: u64,
    pub peers: Vec<PeerId>,
}

impl FileTransfer {
    pub fn new(metadata: FileMetadata, output_path: PathBuf) -> Self {
        let missing_chunks: Vec<u32> = (0..metadata.total_chunks).collect();
        let started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            metadata,
            downloaded_chunks: HashSet::new(),
            missing_chunks,
            output_path,
            progress: 0.0,
            started_at,
            peers: Vec::new(),
        }
    }

    pub fn mark_chunk_downloaded(&mut self, chunk_index: u32) {
        if self.downloaded_chunks.insert(chunk_index) {
            self.missing_chunks.retain(|&idx| idx != chunk_index);
            self.progress = self.downloaded_chunks.len() as f32 / self.metadata.total_chunks as f32;
        }
    }

    pub fn is_complete(&self) -> bool {
        self.downloaded_chunks.len() == self.metadata.total_chunks as usize
    }

    pub fn add_peer(&mut self, peer: PeerId) {
        if !self.peers.contains(&peer) {
            self.peers.push(peer);
        }
    }
}

/// Calculate SHA256 hash of chunk data
pub fn calculate_chunk_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Verify that a chunk's data matches its hash
pub fn verify_chunk(chunk: &FileChunk) -> bool {
    let calculated_hash = calculate_chunk_hash(&chunk.data);
    calculated_hash == chunk.hash
}

/// Split a file into chunks for transfer
pub fn split_file_to_chunks(
    path: &Path,
    chunk_size: u32,
) -> io::Result<(FileMetadata, Vec<FileChunk>)> {
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let total_chunks = file_size.div_ceil(chunk_size as u64) as u32;
    let mut chunks = Vec::with_capacity(total_chunks as usize);
    let mut chunk_hashes = Vec::with_capacity(total_chunks as usize);
    let file_id = uuid::Uuid::new_v4().to_string();

    for chunk_index in 0..total_chunks {
        let mut buffer = vec![0u8; chunk_size as usize];
        let bytes_read = file.read(&mut buffer)?;
        buffer.truncate(bytes_read);

        let hash = calculate_chunk_hash(&buffer);
        chunk_hashes.push(hash);

        chunks.push(FileChunk {
            file_id: file_id.clone(),
            chunk_index,
            data: buffer,
            hash,
        });
    }

    let metadata = FileMetadata {
        file_id,
        name: file_name,
        size: file_size,
        chunk_size,
        total_chunks,
        chunk_hashes,
        mime_type: None,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    Ok((metadata, chunks))
}

/// Assemble chunks into a complete file
pub fn assemble_chunks(
    chunks: &[FileChunk],
    metadata: &FileMetadata,
    output: &Path,
) -> io::Result<()> {
    // Verify we have all chunks
    if chunks.len() != metadata.total_chunks as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Expected {} chunks, got {}",
                metadata.total_chunks,
                chunks.len()
            ),
        ));
    }

    // Create or truncate output file
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(output)?;

    // Sort chunks by index
    let mut sorted_chunks: Vec<&FileChunk> = chunks.iter().collect();
    sorted_chunks.sort_by_key(|c| c.chunk_index);

    // Verify and write each chunk
    for (i, chunk) in sorted_chunks.iter().enumerate() {
        if chunk.chunk_index != i as u32 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Missing chunk {}", i),
            ));
        }

        // Verify chunk hash matches metadata
        if chunk.hash != metadata.chunk_hashes[i] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Chunk {} hash mismatch", i),
            ));
        }

        // Verify chunk data hash
        if !verify_chunk(chunk) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Chunk {} data verification failed", i),
            ));
        }

        file.write_all(&chunk.data)?;
    }

    file.flush()?;
    Ok(())
}

/// Write a single chunk to a file at the correct offset (for incremental assembly)
pub fn write_chunk_to_file(
    chunk: &FileChunk,
    metadata: &FileMetadata,
    output: &Path,
) -> io::Result<()> {
    // Verify chunk
    if !verify_chunk(chunk) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Chunk {} verification failed", chunk.chunk_index),
        ));
    }

    // Verify against metadata
    if chunk.chunk_index >= metadata.total_chunks {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid chunk index {}", chunk.chunk_index),
        ));
    }

    if chunk.hash != metadata.chunk_hashes[chunk.chunk_index as usize] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Chunk {} hash mismatch with metadata", chunk.chunk_index),
        ));
    }

    // Calculate offset
    let offset = chunk.chunk_index as u64 * metadata.chunk_size as u64;

    // Open file and seek to offset (don't truncate - we're writing chunks incrementally)
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(output)?;

    file.seek(SeekFrom::Start(offset))?;
    file.write_all(&chunk.data)?;
    file.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_chunk_hash_calculation() {
        let data = b"Hello, World!";
        let hash1 = calculate_chunk_hash(data);
        let hash2 = calculate_chunk_hash(data);
        assert_eq!(hash1, hash2);

        let different_data = b"Hello, World!!";
        let hash3 = calculate_chunk_hash(different_data);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_chunk_verification() {
        let data = b"Test data".to_vec();
        let chunk = FileChunk::new("test-id".to_string(), 0, data);
        assert!(verify_chunk(&chunk));

        let mut bad_chunk = chunk.clone();
        bad_chunk.data.push(0);
        assert!(!verify_chunk(&bad_chunk));
    }

    #[test]
    fn test_split_and_assemble() -> io::Result<()> {
        // Create temporary file with test data
        let mut temp_file = NamedTempFile::new()?;
        let test_data = b"This is test data that will be split into chunks and reassembled.";
        temp_file.write_all(test_data)?;
        temp_file.flush()?;

        // Split into chunks
        let (metadata, chunks) = split_file_to_chunks(temp_file.path(), 10)?;
        assert_eq!(chunks.len(), metadata.total_chunks as usize);

        // Verify all chunks
        for chunk in &chunks {
            assert!(verify_chunk(chunk));
        }

        // Assemble back
        let output_file = NamedTempFile::new()?;
        assemble_chunks(&chunks, &metadata, output_file.path())?;

        // Verify output matches input
        let mut output_data = Vec::new();
        File::open(output_file.path())?.read_to_end(&mut output_data)?;
        assert_eq!(output_data, test_data);

        Ok(())
    }

    #[test]
    fn test_file_transfer_progress() {
        let temp_file = NamedTempFile::new().unwrap();
        let metadata = FileMetadata::new("test.txt".to_string(), 1000, vec![[0u8; 32]; 10]);
        let mut transfer = FileTransfer::new(metadata, temp_file.path().to_path_buf());

        assert_eq!(transfer.progress, 0.0);
        assert!(!transfer.is_complete());

        transfer.mark_chunk_downloaded(0);
        assert_eq!(transfer.progress, 0.1);

        for i in 1..10 {
            transfer.mark_chunk_downloaded(i);
        }

        assert_eq!(transfer.progress, 1.0);
        assert!(transfer.is_complete());
    }
}
