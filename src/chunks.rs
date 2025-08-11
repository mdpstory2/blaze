//! Chunk storage and management for Blaze VCS

use crate::errors::{BlazeError, Result, ResultExt};
use crate::files::FileChunk;

use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Chunk storage manager for handling chunk persistence
pub struct ChunkStore {
    /// Base directory for chunk storage
    chunks_dir: PathBuf,
    /// Cache of loaded chunks (hash -> data)
    chunk_cache: HashMap<String, Vec<u8>>,
    /// Maximum cache size in bytes
    max_cache_size: usize,
    /// Current cache size in bytes
    current_cache_size: usize,
}

impl ChunkStore {
    /// Create a new chunk store
    pub fn new<P: AsRef<Path>>(chunks_dir: P) -> Result<Self> {
        let chunks_dir = chunks_dir.as_ref().to_path_buf();

        // Create chunks directory if it doesn't exist
        if !chunks_dir.exists() {
            fs::create_dir_all(&chunks_dir).with_context(|| {
                format!(
                    "Failed to create chunks directory: {}",
                    chunks_dir.display()
                )
            })?;
        }

        Ok(ChunkStore {
            chunks_dir,
            chunk_cache: HashMap::new(),
            max_cache_size: 64 * 1024 * 1024, // 64MB cache
            current_cache_size: 0,
        })
    }

    /// Store a chunk and return its hash
    pub fn store_chunk(&mut self, chunk: &FileChunk) -> Result<String> {
        let chunk_path = self.get_chunk_path(&chunk.hash);

        // Don't store if chunk already exists
        if chunk_path.exists() {
            return Ok(chunk.hash.clone());
        }

        // Create subdirectory if needed (first 2 chars of hash)
        if let Some(parent) = chunk_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create chunk subdirectory: {}", parent.display())
            })?;
        }

        // Write compressed chunk data
        let compressed_data = self.compress_chunk_data(&chunk.data)?;

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&chunk_path)
            .with_context(|| format!("Failed to create chunk file: {}", chunk_path.display()))?;

        file.write_all(&compressed_data)
            .context("Failed to write chunk data")?;

        // Add to cache if there's space
        self.maybe_cache_chunk(&chunk.hash, chunk.data.clone());

        Ok(chunk.hash.clone())
    }

    /// Store multiple chunks in parallel
    pub fn store_chunks(&mut self, chunks: &[FileChunk]) -> Result<Vec<String>> {
        // Group chunks by their subdirectory to minimize directory creation overhead
        let mut chunks_by_subdir: HashMap<String, Vec<&FileChunk>> = HashMap::new();

        for chunk in chunks {
            let subdir = self.get_chunk_subdir(&chunk.hash);
            chunks_by_subdir.entry(subdir).or_default().push(chunk);
        }

        // Create all necessary subdirectories first
        for subdir in chunks_by_subdir.keys() {
            let subdir_path = self.chunks_dir.join(subdir);
            if !subdir_path.exists() {
                fs::create_dir_all(&subdir_path).with_context(|| {
                    format!(
                        "Failed to create chunk subdirectory: {}",
                        subdir_path.display()
                    )
                })?;
            }
        }

        // Store chunks in parallel
        let results: Result<Vec<_>> = chunks
            .par_iter()
            .map(|chunk| {
                let chunk_path = self.get_chunk_path(&chunk.hash);

                // Skip if already exists
                if chunk_path.exists() {
                    return Ok(chunk.hash.clone());
                }

                // Compress and write chunk
                let compressed_data = self.compress_chunk_data(&chunk.data)?;

                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&chunk_path)
                    .with_context(|| {
                        format!("Failed to create chunk file: {}", chunk_path.display())
                    })?;

                file.write_all(&compressed_data)
                    .context("Failed to write chunk data")?;

                Ok(chunk.hash.clone())
            })
            .collect();

        let hashes = results?;

        // Update cache for all chunks
        for chunk in chunks {
            self.maybe_cache_chunk(&chunk.hash, chunk.data.clone());
        }

        Ok(hashes)
    }

    /// Load a chunk by its hash
    pub fn load_chunk(&mut self, hash: &str) -> Result<Vec<u8>> {
        // Check cache first
        if let Some(data) = self.chunk_cache.get(hash) {
            return Ok(data.clone());
        }

        let chunk_path = self.get_chunk_path(hash);

        if !chunk_path.exists() {
            return Err(BlazeError::Chunk(format!("Chunk not found: {}", hash)));
        }

        // Read and decompress chunk data
        let mut file = File::open(&chunk_path)
            .with_context(|| format!("Failed to open chunk file: {}", chunk_path.display()))?;

        let mut compressed_data = Vec::new();
        file.read_to_end(&mut compressed_data)
            .context("Failed to read chunk data")?;

        let data = self.decompress_chunk_data(&compressed_data)?;

        // Verify chunk integrity
        let computed_hash = crate::files::compute_chunk_hash(&data);
        if computed_hash != hash {
            return Err(BlazeError::Chunk(format!(
                "Chunk integrity check failed: expected {}, got {}",
                hash, computed_hash
            )));
        }

        // Cache the loaded chunk
        self.maybe_cache_chunk(hash, data.clone());

        Ok(data)
    }

    /// Load multiple chunks in parallel
    pub fn load_chunks(&mut self, hashes: &[String]) -> Result<Vec<Vec<u8>>> {
        let results: Result<Vec<_>> = hashes
            .par_iter()
            .map(|hash| self.load_chunk_uncached(hash))
            .collect();

        let chunks_data = results?;

        // Update cache for all loaded chunks
        for (hash, data) in hashes.iter().zip(chunks_data.iter()) {
            self.maybe_cache_chunk(hash, data.clone());
        }

        Ok(chunks_data)
    }

    /// Check if a chunk exists in storage
    pub fn chunk_exists(&self, hash: &str) -> bool {
        self.chunk_cache.contains_key(hash) || self.get_chunk_path(hash).exists()
    }

    /// Get the number of chunks in storage
    pub fn chunk_count(&self) -> Result<usize> {
        let mut count = 0;

        for entry in fs::read_dir(&self.chunks_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                for subentry in fs::read_dir(entry.path())? {
                    let subentry = subentry?;
                    if subentry.path().is_file() {
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    /// Calculate total storage size of all chunks
    pub fn total_storage_size(&self) -> Result<u64> {
        let mut total_size = 0;

        for entry in fs::read_dir(&self.chunks_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                for subentry in fs::read_dir(entry.path())? {
                    let subentry = subentry?;
                    if subentry.path().is_file() {
                        total_size += subentry.metadata()?.len();
                    }
                }
            }
        }

        Ok(total_size)
    }

    /// Remove unused chunks (garbage collection)
    pub fn garbage_collect(&mut self, active_hashes: &[String]) -> Result<usize> {
        let active_set: std::collections::HashSet<_> = active_hashes.iter().collect();
        let mut removed_count = 0;

        for entry in fs::read_dir(&self.chunks_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                for subentry in fs::read_dir(entry.path())? {
                    let subentry = subentry?;
                    if subentry.path().is_file() {
                        if let Some(filename) = subentry.file_name().to_str() {
                            let full_hash =
                                format!("{}{}", entry.file_name().to_string_lossy(), filename);

                            if !active_set.contains(&full_hash) {
                                fs::remove_file(subentry.path())?;
                                self.chunk_cache.remove(&full_hash);
                                removed_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }

    /// Clear the in-memory cache
    pub fn clear_cache(&mut self) {
        self.chunk_cache.clear();
        self.current_cache_size = 0;
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize, usize) {
        (
            self.chunk_cache.len(),
            self.current_cache_size,
            self.max_cache_size,
        )
    }

    // Private helper methods

    fn get_chunk_path(&self, hash: &str) -> PathBuf {
        if hash.len() < 2 {
            return self.chunks_dir.join(hash);
        }

        let subdir = &hash[..2];
        let filename = &hash[2..];
        self.chunks_dir.join(subdir).join(filename)
    }

    fn get_chunk_subdir(&self, hash: &str) -> String {
        if hash.len() < 2 {
            "00".to_string()
        } else {
            hash[..2].to_string()
        }
    }

    fn compress_chunk_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        // For now, we'll use a simple approach without external compression
        // In a real implementation, you might want to use zstd or similar
        if data.len() < 64 {
            // Don't compress very small chunks
            let mut result = vec![0]; // 0 = uncompressed
            result.extend_from_slice(data);
            Ok(result)
        } else {
            // Simple run-length encoding for demonstration
            let mut result = vec![1]; // 1 = compressed
            result.extend_from_slice(data); // TODO: Implement actual compression
            Ok(result)
        }
    }

    fn decompress_chunk_data(&self, compressed: &[u8]) -> Result<Vec<u8>> {
        if compressed.is_empty() {
            return Err(BlazeError::Chunk("Empty compressed data".to_string()));
        }

        match compressed[0] {
            0 => Ok(compressed[1..].to_vec()), // Uncompressed
            1 => Ok(compressed[1..].to_vec()), // TODO: Implement actual decompression
            _ => Err(BlazeError::Chunk("Unknown compression type".to_string())),
        }
    }

    fn load_chunk_uncached(&self, hash: &str) -> Result<Vec<u8>> {
        let chunk_path = self.get_chunk_path(hash);

        if !chunk_path.exists() {
            return Err(BlazeError::Chunk(format!("Chunk not found: {}", hash)));
        }

        let mut file = File::open(&chunk_path)
            .with_context(|| format!("Failed to open chunk file: {}", chunk_path.display()))?;

        let mut compressed_data = Vec::new();
        file.read_to_end(&mut compressed_data)
            .context("Failed to read chunk data")?;

        let data = self.decompress_chunk_data(&compressed_data)?;

        // Verify chunk integrity
        let computed_hash = crate::files::compute_chunk_hash(&data);
        if computed_hash != hash {
            return Err(BlazeError::Chunk(format!(
                "Chunk integrity check failed: expected {}, got {}",
                hash, computed_hash
            )));
        }

        Ok(data)
    }

    fn maybe_cache_chunk(&mut self, hash: &str, data: Vec<u8>) {
        let data_size = data.len();

        // Don't cache if data is too large for cache
        if data_size > self.max_cache_size / 4 {
            return;
        }

        // Evict old entries if cache is getting full
        while self.current_cache_size + data_size > self.max_cache_size
            && !self.chunk_cache.is_empty()
        {
            if let Some((old_hash, old_data)) = self.chunk_cache.iter().next() {
                let old_hash = old_hash.clone();
                let old_size = old_data.len();
                self.chunk_cache.remove(&old_hash);
                self.current_cache_size -= old_size;
            } else {
                break;
            }
        }

        // Add to cache
        self.chunk_cache.insert(hash.to_string(), data);
        self.current_cache_size += data_size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::FileChunk;
    use tempfile::TempDir;

    #[test]
    fn test_chunk_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let chunks_dir = temp_dir.path().join("chunks");

        let _store = ChunkStore::new(&chunks_dir).unwrap();
        assert!(chunks_dir.exists());
    }

    #[test]
    fn test_store_and_load_chunk() {
        let temp_dir = TempDir::new().unwrap();
        let chunks_dir = temp_dir.path().join("chunks");

        let mut store = ChunkStore::new(&chunks_dir).unwrap();

        let data = b"Hello, world!".to_vec();
        let chunk = FileChunk::new(data.clone());

        let hash = store.store_chunk(&chunk).unwrap();
        assert_eq!(hash, chunk.hash);

        let loaded_data = store.load_chunk(&hash).unwrap();
        assert_eq!(loaded_data, data);
    }

    #[test]
    fn test_chunk_exists() {
        let temp_dir = TempDir::new().unwrap();
        let chunks_dir = temp_dir.path().join("chunks");

        let mut store = ChunkStore::new(&chunks_dir).unwrap();

        let data = b"Test data".to_vec();
        let chunk = FileChunk::new(data);

        assert!(!store.chunk_exists(&chunk.hash));

        store.store_chunk(&chunk).unwrap();
        assert!(store.chunk_exists(&chunk.hash));
    }

    #[test]
    fn test_store_multiple_chunks() {
        let temp_dir = TempDir::new().unwrap();
        let chunks_dir = temp_dir.path().join("chunks");

        let mut store = ChunkStore::new(&chunks_dir).unwrap();

        let chunks = vec![
            FileChunk::new(b"Chunk 1".to_vec()),
            FileChunk::new(b"Chunk 2".to_vec()),
            FileChunk::new(b"Chunk 3".to_vec()),
        ];

        let hashes = store.store_chunks(&chunks).unwrap();
        assert_eq!(hashes.len(), 3);

        for (chunk, hash) in chunks.iter().zip(hashes.iter()) {
            assert_eq!(&chunk.hash, hash);
            assert!(store.chunk_exists(hash));
        }
    }

    #[test]
    fn test_chunk_integrity() {
        let temp_dir = TempDir::new().unwrap();
        let chunks_dir = temp_dir.path().join("chunks");

        let mut store = ChunkStore::new(&chunks_dir).unwrap();

        let data = b"Integrity test data".to_vec();
        let chunk = FileChunk::new(data.clone());

        store.store_chunk(&chunk).unwrap();

        // Corrupt the stored chunk file
        let chunk_path = store.get_chunk_path(&chunk.hash);
        std::fs::write(&chunk_path, b"corrupted data").unwrap();

        // Clear cache to force reload from disk
        store.clear_cache();

        // Loading should fail due to integrity check
        let result = store.load_chunk(&chunk.hash);
        assert!(result.is_err());
    }

    #[test]
    fn test_cache_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let chunks_dir = temp_dir.path().join("chunks");

        let mut store = ChunkStore::new(&chunks_dir).unwrap();

        let data = b"Cache test data".to_vec();
        let chunk = FileChunk::new(data.clone());

        store.store_chunk(&chunk).unwrap();

        let (cached_count, _, _) = store.cache_stats();
        assert!(cached_count > 0);

        // Load again - should come from cache
        let loaded_data = store.load_chunk(&chunk.hash).unwrap();
        assert_eq!(loaded_data, data);
    }

    #[test]
    fn test_garbage_collection() {
        let temp_dir = TempDir::new().unwrap();
        let chunks_dir = temp_dir.path().join("chunks");

        let mut store = ChunkStore::new(&chunks_dir).unwrap();

        let chunks = vec![
            FileChunk::new(b"Keep this chunk".to_vec()),
            FileChunk::new(b"Remove this chunk".to_vec()),
        ];

        for chunk in &chunks {
            store.store_chunk(chunk).unwrap();
        }

        // Only keep the first chunk
        let active_hashes = vec![chunks[0].hash.clone()];
        let removed_count = store.garbage_collect(&active_hashes).unwrap();

        assert_eq!(removed_count, 1);
        assert!(store.chunk_exists(&chunks[0].hash));
        assert!(!store.chunk_exists(&chunks[1].hash));
    }
}
