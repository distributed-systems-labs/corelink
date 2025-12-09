use corelink_core::file::{
    split_file_to_chunks, verify_chunk, write_chunk_to_file, FileChunk, FileMetadata, FileTransfer,
};
use libp2p_identity::PeerId;
use lru::LruCache;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub enum TransferStatus {
    ChunkReceived { progress: f32 },
    TransferComplete,
    VerificationFailed { chunk_index: u32 },
}

pub struct FileTransferManager {
    active_uploads: HashMap<String, FileMetadata>,
    active_downloads: HashMap<String, FileTransfer>,
    chunk_cache: LruCache<(String, u32), Vec<u8>>,
    pub storage_path: PathBuf,
}

impl FileTransferManager {
    pub fn new(storage_path: PathBuf) -> io::Result<Self> {
        // Create storage directories
        let uploads_path = storage_path.join("uploads");
        let downloads_path = storage_path.join("downloads");
        let complete_path = storage_path.join("complete");

        fs::create_dir_all(&uploads_path)?;
        fs::create_dir_all(&downloads_path)?;
        fs::create_dir_all(&complete_path)?;

        info!("📁 FileTransferManager initialized at: {:?}", storage_path);
        info!("   Uploads: {:?}", uploads_path);
        info!("   Downloads: {:?}", downloads_path);
        info!("   Complete: {:?}", complete_path);

        Ok(Self {
            active_uploads: HashMap::new(),
            active_downloads: HashMap::new(),
            chunk_cache: LruCache::new(NonZeroUsize::new(100).unwrap()),
            storage_path,
        })
    }

    /// Offer a file for transfer by splitting it into chunks
    pub fn offer_file(&mut self, path: &Path) -> io::Result<FileMetadata> {
        if !path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {:?}", path),
            ));
        }

        info!("📤 Offering file: {:?}", path);

        // Split file into chunks
        let (metadata, chunks) = split_file_to_chunks(path, 64 * 1024)?;

        // Cache all chunks for quick access
        for chunk in chunks {
            self.chunk_cache
                .put((metadata.file_id.clone(), chunk.chunk_index), chunk.data);
        }

        // Copy file to uploads directory
        let upload_path = self.storage_path.join("uploads").join(&metadata.name);
        if let Err(e) = fs::copy(path, &upload_path) {
            warn!(
                "Failed to copy file to uploads directory: {}. File will be served from original location.",
                e
            );
        }

        info!(
            "✅ File offered: {} ({} bytes, {} chunks)",
            metadata.name, metadata.size, metadata.total_chunks
        );

        // Register as active upload
        let file_id = metadata.file_id.clone();
        self.active_uploads.insert(file_id, metadata.clone());

        Ok(metadata)
    }

    /// Request a file for download
    pub fn request_file(
        &mut self,
        metadata: FileMetadata,
        output_path: PathBuf,
        peer: PeerId,
    ) -> io::Result<String> {
        let file_id = metadata.file_id.clone();

        // Check if already downloading
        if self.active_downloads.contains_key(&file_id) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Already downloading file: {}", file_id),
            ));
        }

        info!("📥 Requesting file: {} from peer {}", metadata.name, peer);

        // Create FileTransfer to track progress
        let mut transfer = FileTransfer::new(metadata.clone(), output_path.clone());
        transfer.add_peer(peer);

        // Pre-allocate file with correct size
        if let Err(e) = fs::File::create(&output_path).and_then(|f| f.set_len(metadata.size)) {
            warn!("Failed to pre-allocate download file: {}", e);
        }

        info!(
            "📊 Download initialized: {} chunks to download",
            transfer.missing_chunks.len()
        );

        self.active_downloads.insert(file_id.clone(), transfer);

        Ok(file_id)
    }

    /// Handle a chunk request and return the chunk if available
    pub fn handle_chunk_request(
        &mut self,
        file_id: &str,
        chunk_index: u32,
    ) -> io::Result<Option<FileChunk>> {
        // Check if we're offering this file
        let metadata = match self.active_uploads.get(file_id) {
            Some(m) => m,
            None => {
                debug!("Chunk request for unknown file: {}", file_id);
                return Ok(None);
            }
        };

        // Validate chunk index
        if chunk_index >= metadata.total_chunks {
            warn!(
                "Invalid chunk index {} for file {} (max: {})",
                chunk_index, file_id, metadata.total_chunks
            );
            return Ok(None);
        }

        // Check cache first
        if let Some(data) = self.chunk_cache.get(&(file_id.to_string(), chunk_index)) {
            debug!("📦 Serving chunk {} from cache", chunk_index);
            let chunk = FileChunk::new(file_id.to_string(), chunk_index, data.clone());
            return Ok(Some(chunk));
        }

        // Load from file
        let file_path = self.storage_path.join("uploads").join(&metadata.name);
        if !file_path.exists() {
            error!("Upload file not found: {:?}", file_path);
            return Ok(None);
        }

        // Read chunk from file
        let offset = chunk_index as u64 * metadata.chunk_size as u64;
        let chunk_size = if chunk_index == metadata.total_chunks - 1 {
            // Last chunk might be smaller
            (metadata.size - offset) as usize
        } else {
            metadata.chunk_size as usize
        };

        let mut file = fs::File::open(&file_path)?;
        use std::io::{Read, Seek, SeekFrom};
        file.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0u8; chunk_size];
        file.read_exact(&mut buffer)?;

        let chunk = FileChunk::new(file_id.to_string(), chunk_index, buffer.clone());

        // Cache for future requests
        self.chunk_cache
            .put((file_id.to_string(), chunk_index), buffer);

        debug!("📦 Serving chunk {} from file", chunk_index);
        Ok(Some(chunk))
    }

    /// Handle a received chunk and write it to the download file
    pub fn handle_chunk_received(&mut self, chunk: FileChunk) -> io::Result<TransferStatus> {
        let file_id = chunk.file_id.clone();
        let chunk_index = chunk.chunk_index;

        // Get the active download
        let transfer = match self.active_downloads.get_mut(&file_id) {
            Some(t) => t,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("No active download for file: {}", file_id),
                ));
            }
        };

        // Verify chunk
        if !verify_chunk(&chunk) {
            error!(
                "❌ Chunk verification failed: {} index {}",
                file_id, chunk_index
            );
            return Ok(TransferStatus::VerificationFailed { chunk_index });
        }

        // Write chunk to file
        write_chunk_to_file(&chunk, &transfer.metadata, &transfer.output_path)?;

        // Update transfer state
        transfer.mark_chunk_downloaded(chunk_index);

        let progress = transfer.progress;
        debug!(
            "📥 Chunk {}/{} received ({:.1}%)",
            chunk_index,
            transfer.metadata.total_chunks,
            progress * 100.0
        );

        // Check if transfer is complete
        if transfer.is_complete() {
            info!("✅ Transfer complete: {}", file_id);

            // Move to complete directory
            let final_path = self
                .storage_path
                .join("complete")
                .join(&transfer.metadata.name);

            if let Err(e) = fs::rename(&transfer.output_path, &final_path) {
                warn!("Failed to move completed file: {}", e);
            } else {
                info!("📁 File saved to: {:?}", final_path);
            }

            // Remove from active downloads
            self.active_downloads.remove(&file_id);

            return Ok(TransferStatus::TransferComplete);
        }

        Ok(TransferStatus::ChunkReceived { progress })
    }

    /// Get the next batch of chunks to request for a file
    pub fn get_next_chunks_to_request(&self, file_id: &str, batch_size: usize) -> Vec<u32> {
        if let Some(transfer) = self.active_downloads.get(file_id) {
            transfer
                .missing_chunks
                .iter()
                .take(batch_size)
                .copied()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get active downloads count
    #[allow(dead_code)]
    pub fn active_downloads_count(&self) -> usize {
        self.active_downloads.len()
    }

    /// Get active uploads count
    #[allow(dead_code)]
    pub fn active_uploads_count(&self) -> usize {
        self.active_uploads.len()
    }

    /// Cancel a download
    #[allow(dead_code)]
    pub fn cancel_download(&mut self, file_id: &str) -> io::Result<()> {
        if let Some(transfer) = self.active_downloads.remove(file_id) {
            info!("🚫 Cancelled download: {}", file_id);

            // Optionally delete partial file
            if transfer.output_path.exists() {
                fs::remove_file(&transfer.output_path)?;
                debug!("Deleted partial download file: {:?}", transfer.output_path);
            }

            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("No active download: {}", file_id),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use tempfile::{tempdir, NamedTempFile};

    #[test]
    fn test_offer_file() -> io::Result<()> {
        let storage_dir = tempdir()?;
        let mut manager = FileTransferManager::new(storage_dir.path().to_path_buf())?;

        // Create test file
        let mut temp_file = NamedTempFile::new()?;
        let test_data = b"Hello, World! This is test data for file transfer.";
        temp_file.write_all(test_data)?;
        temp_file.flush()?;

        // Offer file
        let metadata = manager.offer_file(temp_file.path())?;

        assert_eq!(metadata.size, test_data.len() as u64);
        assert!(metadata.total_chunks > 0);
        assert_eq!(metadata.chunk_hashes.len(), metadata.total_chunks as usize);
        assert_eq!(manager.active_uploads_count(), 1);

        Ok(())
    }

    #[test]
    fn test_chunk_request() -> io::Result<()> {
        let storage_dir = tempdir()?;
        let mut manager = FileTransferManager::new(storage_dir.path().to_path_buf())?;

        // Create and offer test file
        let mut temp_file = NamedTempFile::new()?;
        let test_data = b"Test data for chunk request";
        temp_file.write_all(test_data)?;
        temp_file.flush()?;

        let metadata = manager.offer_file(temp_file.path())?;

        // Request first chunk
        let chunk = manager.handle_chunk_request(&metadata.file_id, 0)?;
        assert!(chunk.is_some());

        let chunk = chunk.unwrap();
        assert_eq!(chunk.chunk_index, 0);
        assert_eq!(chunk.file_id, metadata.file_id);
        assert!(verify_chunk(&chunk));

        // Request invalid chunk
        let invalid_chunk = manager.handle_chunk_request(&metadata.file_id, 999)?;
        assert!(invalid_chunk.is_none());

        Ok(())
    }

    #[test]
    fn test_chunk_received() -> io::Result<()> {
        let storage_dir = tempdir()?;
        let mut manager = FileTransferManager::new(storage_dir.path().to_path_buf())?;

        // Create test file and split into chunks
        let mut temp_file = NamedTempFile::new()?;
        let test_data = b"Test data for chunk reception";
        temp_file.write_all(test_data)?;
        temp_file.flush()?;

        let (metadata, chunks) = split_file_to_chunks(temp_file.path(), 64 * 1024)?;

        // Request file
        let peer = PeerId::random();
        let output_path = storage_dir.path().join("downloads").join("test.dat");
        let file_id = manager.request_file(metadata.clone(), output_path.clone(), peer)?;

        // Receive all chunks
        for chunk in chunks {
            let status = manager.handle_chunk_received(chunk)?;
            match status {
                TransferStatus::ChunkReceived { progress, .. } => {
                    assert!(progress >= 0.0 && progress <= 1.0);
                }
                TransferStatus::TransferComplete { .. } => {
                    // Expected for last chunk
                }
                TransferStatus::VerificationFailed { .. } => {
                    panic!("Chunk verification should not fail");
                }
            }
        }

        // Verify transfer is complete
        assert!(!manager.active_downloads.contains_key(&file_id));

        // Verify final file exists and has correct content
        let final_path = storage_dir.path().join("complete").join(&metadata.name);
        assert!(final_path.exists());

        let mut result_data = Vec::new();
        fs::File::open(final_path)?.read_to_end(&mut result_data)?;
        assert_eq!(result_data, test_data);

        Ok(())
    }

    #[test]
    fn test_full_transfer_lifecycle() -> io::Result<()> {
        let storage_dir = tempdir()?;
        let mut uploader = FileTransferManager::new(storage_dir.path().join("uploader"))?;
        let mut downloader = FileTransferManager::new(storage_dir.path().join("downloader"))?;

        // Create test file with multiple chunks
        let mut temp_file = NamedTempFile::new()?;
        let test_data: Vec<u8> = (0..200_000).map(|i| (i % 256) as u8).collect();
        temp_file.write_all(&test_data)?;
        temp_file.flush()?;

        // Uploader offers file
        let metadata = uploader.offer_file(temp_file.path())?;
        assert!(metadata.total_chunks > 1); // Ensure multiple chunks

        // Downloader requests file
        let peer = PeerId::random();
        let output_path = storage_dir
            .path()
            .join("downloader")
            .join("downloads")
            .join("test.dat");
        let file_id = downloader.request_file(metadata.clone(), output_path, peer)?;

        // Transfer all chunks
        loop {
            let chunks_to_request = downloader.get_next_chunks_to_request(&file_id, 5);
            if chunks_to_request.is_empty() {
                break;
            }

            for chunk_index in chunks_to_request {
                // Uploader provides chunk
                let chunk = uploader
                    .handle_chunk_request(&file_id, chunk_index)?
                    .expect("Chunk should be available");

                // Downloader receives chunk
                let status = downloader.handle_chunk_received(chunk)?;

                if let TransferStatus::TransferComplete { .. } = status {
                    break;
                }
            }
        }

        // Verify transfer completed
        assert_eq!(downloader.active_downloads_count(), 0);

        // Verify downloaded file
        let final_path = storage_dir
            .path()
            .join("downloader")
            .join("complete")
            .join(&metadata.name);
        assert!(final_path.exists());

        let mut result_data = Vec::new();
        fs::File::open(final_path)?.read_to_end(&mut result_data)?;
        assert_eq!(result_data.len(), test_data.len());
        assert_eq!(result_data, test_data);

        Ok(())
    }

    #[test]
    fn test_cancel_download() -> io::Result<()> {
        let storage_dir = tempdir()?;
        let mut manager = FileTransferManager::new(storage_dir.path().to_path_buf())?;

        // Create and request test file
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(b"Test data")?;
        temp_file.flush()?;

        let (metadata, _) = split_file_to_chunks(temp_file.path(), 64 * 1024)?;

        let peer = PeerId::random();
        let output_path = storage_dir.path().join("downloads").join("test.dat");
        let file_id = manager.request_file(metadata, output_path.clone(), peer)?;

        assert_eq!(manager.active_downloads_count(), 1);

        // Cancel download
        manager.cancel_download(&file_id)?;

        assert_eq!(manager.active_downloads_count(), 0);
        assert!(!output_path.exists());

        Ok(())
    }
}
