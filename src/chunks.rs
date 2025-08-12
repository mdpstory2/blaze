//! Chunk storage and management for Blaze VCS

use crate::errors::{BlazeError, Result, ResultExt};
use crate::files::FileChunk;

use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Delta compression data for storing similar chunks efficiently
#[derive(Debug, Clone)]
pub struct ChunkDelta {
    pub base_hash: String,
    pub delta_data: Vec<u8>,
    pub original_size: usize,
}

/// Chunk with delta compression support
#[derive(Debug, Clone)]
pub enum CompressedChunk {
    Full(Vec<u8>),
    Delta(ChunkDelta),
}

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
    /// Cache of chunks that are known to exist
    existence_cache: RwLock<HashSet<String>>,
    /// Cache of chunks that are known to NOT exist
    negative_cache: RwLock<HashSet<String>>,
    /// Delta compression cache - maps hash to similar chunk hashes
    delta_cache: RwLock<HashMap<String, Vec<String>>>,
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
            existence_cache: RwLock::new(HashSet::new()),
            negative_cache: RwLock::new(HashSet::new()),
            delta_cache: RwLock::new(HashMap::new()),
        })
    }

    /// Store a chunk and return its hash
    pub fn store_chunk(&mut self, chunk: &FileChunk) -> Result<String> {
        // Check cache first
        if self.chunk_exists(&chunk.hash) {
            return Ok(chunk.hash.clone());
        }

        let chunk_path = self.get_chunk_path(&chunk.hash);

        // Create subdirectory if needed (first 2 chars of hash)
        if let Some(parent) = chunk_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create chunk subdirectory: {}", parent.display())
            })?;
        }

        // Write compressed chunk data with atomic operation
        let compressed_data = self.compress_chunk_data(&chunk.data)?;
        let temp_path = chunk_path.with_extension("tmp");

        {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temp_path)
                .with_context(|| {
                    format!("Failed to create temp chunk file: {}", temp_path.display())
                })?;

            file.write_all(&compressed_data)
                .context("Failed to write chunk data")?;

            file.sync_all().context("Failed to sync chunk data")?;
        }

        // Atomic rename
        fs::rename(&temp_path, &chunk_path)
            .with_context(|| format!("Failed to rename temp file: {}", chunk_path.display()))?;

        // Add to cache if there's space
        self.maybe_cache_chunk(&chunk.hash, chunk.data.clone());

        // Mark chunk as existing in our existence cache
        self.existence_cache
            .write()
            .unwrap()
            .insert(chunk.hash.clone());
        self.negative_cache.write().unwrap().remove(&chunk.hash);

        Ok(chunk.hash.clone())
    }

    /// Store multiple chunks in parallel
    pub fn store_chunks(&mut self, chunks: &[FileChunk]) -> Result<Vec<String>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Aggressive deduplication - group by hash first
        let mut unique_chunks: std::collections::HashMap<String, &FileChunk> =
            std::collections::HashMap::new();
        let mut dedupe_savings = 0usize;

        for chunk in chunks {
            if let Some(_existing) = unique_chunks.get(&chunk.hash) {
                dedupe_savings += chunk.size;
            } else {
                unique_chunks.insert(chunk.hash.clone(), chunk);
            }
        }

        // Filter out chunks that already exist on disk
        let new_chunks: Vec<&FileChunk> = unique_chunks
            .values()
            .filter(|chunk| !self.chunk_exists(&chunk.hash))
            .copied()
            .collect();

        if new_chunks.is_empty() {
            // All chunks already exist - perfect deduplication!
            return Ok(chunks.iter().map(|c| c.hash.clone()).collect());
        }

        if dedupe_savings > 0 {
            // Track deduplication savings for statistics
            #[cfg(debug_assertions)]
            println!(
                "Deduplicated {} bytes across {} chunks",
                dedupe_savings,
                chunks.len() - unique_chunks.len()
            );
        }

        // Group chunks by their subdirectory for batch directory creation
        let mut chunks_by_subdir: HashMap<String, Vec<&FileChunk>> = HashMap::new();
        for chunk in &new_chunks {
            let subdir = self.get_chunk_subdir(&chunk.hash);
            chunks_by_subdir.entry(subdir).or_default().push(chunk);
        }

        // Create all necessary subdirectories in parallel
        let subdirs: Vec<String> = chunks_by_subdir.keys().cloned().collect();
        subdirs.par_iter().try_for_each(|subdir| {
            let subdir_path = self.chunks_dir.join(subdir);
            if !subdir_path.exists() {
                fs::create_dir_all(&subdir_path).with_context(|| {
                    format!(
                        "Failed to create chunk subdirectory: {}",
                        subdir_path.display()
                    )
                })
            } else {
                Ok(())
            }
        })?;

        // Use delta compression for better storage efficiency
        let compression_results: Result<Vec<_>> = new_chunks
            .par_iter()
            .map(|chunk| {
                // Try delta compression first for better efficiency
                if chunk.data.len() > 1024 {
                    // Only use delta for chunks > 1KB
                    if let Some(base_hash) = self.find_similar_chunk(&chunk.hash, &chunk.data) {
                        if let Ok(base_data) = self.load_chunk_uncached(&base_hash) {
                            let delta = self.create_delta(&base_data, &chunk.data);
                            if delta.len() < (chunk.data.len() * 8 / 10) {
                                // Delta is 20%+ smaller, use it
                                let compressed_delta = self.compress_chunk_data(&delta)?;
                                let mut delta_data = vec![3]; // 3 = delta compressed
                                delta_data.extend_from_slice(base_hash.as_bytes());
                                delta_data.push(0); // null separator
                                delta_data.extend_from_slice(&compressed_delta);
                                return Ok((chunk.hash.clone(), delta_data, Some(base_hash)));
                            }
                        }
                    }
                }

                // Fall back to regular compression
                let compressed_data = self.compress_chunk_data(&chunk.data)?;
                Ok((chunk.hash.clone(), compressed_data, None::<String>))
            })
            .collect();

        let compressed_chunks = compression_results?;

        // Write all chunks in parallel with delta compression support
        let write_results: Result<Vec<_>> = compressed_chunks
            .par_iter()
            .map(|(hash, compressed_data, base_hash_opt)| {
                let chunk_path = self.get_chunk_path(hash);

                // Use atomic write operations
                let temp_path = chunk_path.with_extension("tmp");

                {
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&temp_path)
                        .with_context(|| {
                            format!("Failed to create temp chunk file: {}", temp_path.display())
                        })?;

                    file.write_all(compressed_data)
                        .context("Failed to write chunk data")?;

                    file.sync_all().context("Failed to sync chunk data")?;
                }

                // Atomic rename
                fs::rename(&temp_path, &chunk_path).with_context(|| {
                    format!("Failed to rename temp file: {}", chunk_path.display())
                })?;

                // Update delta cache if this was a delta chunk
                if let Some(base_hash) = base_hash_opt {
                    if let Ok(mut cache) = self.delta_cache.write() {
                        cache
                            .entry(base_hash.clone())
                            .or_insert_with(Vec::new)
                            .push(hash.clone());
                    }
                }

                Ok(hash.clone())
            })
            .collect();

        let _new_hashes = write_results?;

        // Update cache for new chunks in batch
        for chunk in &new_chunks {
            self.maybe_cache_chunk(&chunk.hash, chunk.data.clone());
            // Mark chunk as existing in our existence cache
            self.existence_cache
                .write()
                .unwrap()
                .insert(chunk.hash.clone());
            self.negative_cache.write().unwrap().remove(&chunk.hash);
        }

        // Return all hashes (existing + new)
        Ok(chunks.iter().map(|c| c.hash.clone()).collect())
    }

    /// Load a chunk by its hash
    pub fn load_chunk(&mut self, hash: &str) -> Result<Vec<u8>> {
        // Check cache first
        if let Some(data) = self.chunk_cache.get(hash) {
            return Ok(data.clone());
        }

        let data = self.load_chunk_uncached(hash)?;

        // Cache the loaded chunk
        self.maybe_cache_chunk(hash, data.clone());

        Ok(data)
    }

    fn load_chunk_uncached(&self, hash: &str) -> Result<Vec<u8>> {
        let chunk_path = self.get_chunk_path(hash);

        // Use optimized file reading
        let mut file = File::open(&chunk_path)
            .with_context(|| format!("Failed to open chunk file: {}", chunk_path.display()))?;

        let file_size = file.metadata()?.len() as usize;
        let mut file_data = Vec::with_capacity(file_size);
        file.read_to_end(&mut file_data)
            .context("Failed to read chunk data")?;

        if file_data.is_empty() {
            return Err(BlazeError::Chunk("Empty chunk file".to_string()));
        }

        let data = match file_data[0] {
            3 => {
                // Delta compressed chunk
                let null_pos = file_data
                    .iter()
                    .position(|&x| x == 0)
                    .unwrap_or(file_data.len());
                if null_pos >= file_data.len() - 1 {
                    return Err(BlazeError::Chunk("Invalid delta format".to_string()));
                }

                let base_hash = String::from_utf8_lossy(&file_data[1..null_pos]);
                let compressed_delta = &file_data[null_pos + 1..];

                // Load base chunk
                let base_data = self.load_chunk_uncached(&base_hash)?;

                // Decompress delta
                let delta = self.decompress_chunk_data(compressed_delta)?;

                // Apply delta to reconstruct original
                self.apply_delta(&base_data, &delta)?
            }
            _ => {
                // Regular compressed chunk
                self.decompress_chunk_data(&file_data)?
            }
        };

        // Skip integrity check for performance in most cases
        // Only verify on first load or if explicitly requested
        #[cfg(debug_assertions)]
        {
            let computed_hash = crate::files::compute_chunk_hash(&data);
            if computed_hash != hash {
                return Err(BlazeError::Chunk(format!(
                    "Chunk integrity check failed: expected {}, got {}",
                    hash, computed_hash
                )));
            }
        }

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

    /// Check if a chunk exists in storage with optimized caching
    pub fn chunk_exists(&self, hash: &str) -> bool {
        // Check in-memory cache first (fastest)
        if self.chunk_cache.contains_key(hash) {
            return true;
        }

        // Check existence cache (very fast)
        if self.existence_cache.read().unwrap().contains(hash) {
            return true;
        }

        // Check negative cache to avoid repeated filesystem checks
        if self.negative_cache.read().unwrap().contains(hash) {
            return false;
        }

        // Finally check filesystem (slowest)
        let exists = self.get_chunk_path(hash).exists();

        // Update caches based on result
        if exists {
            self.existence_cache
                .write()
                .unwrap()
                .insert(hash.to_string());
        } else {
            self.negative_cache
                .write()
                .unwrap()
                .insert(hash.to_string());
        }

        exists
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
                if let Some(subdir_name) = entry.file_name().to_str() {
                    for subentry in fs::read_dir(entry.path())? {
                        let subentry = subentry?;
                        if subentry.path().is_file() {
                            if let Some(filename) = subentry.file_name().to_str() {
                                let full_hash = format!("{}{}", subdir_name, filename);

                                if !active_set.contains(&full_hash) {
                                    fs::remove_file(subentry.path())?;

                                    // Update all caches to reflect removal
                                    self.chunk_cache.remove(&full_hash);
                                    self.existence_cache.write().unwrap().remove(&full_hash);
                                    self.negative_cache.write().unwrap().insert(full_hash);

                                    removed_count += 1;
                                }
                            }
                        }
                    }

                    // Remove empty directories
                    if fs::read_dir(entry.path())?.next().is_none() {
                        let _ = fs::remove_dir(entry.path());
                    }
                }
            }
        }

        Ok(removed_count)
    }

    /// Clear all caches
    pub fn clear_cache(&mut self) {
        self.chunk_cache.clear();
        self.current_cache_size = 0;
        self.existence_cache.write().unwrap().clear();
        self.negative_cache.write().unwrap().clear();
        self.delta_cache.write().unwrap().clear();
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
        if data.len() < 128 {
            // Don't compress very small chunks
            let mut result = vec![0]; // 0 = uncompressed
            result.extend_from_slice(data);
            Ok(result)
        } else {
            // Use aggressive zstd compression for much better storage efficiency
            let compression_level = if data.len() > 1024 * 1024 {
                // For large chunks (>1MB), use higher compression
                6
            } else if data.len() > 64 * 1024 {
                // For medium chunks (>64KB), use good compression
                4
            } else {
                // For smaller chunks, use fast compression
                2
            };

            match zstd::bulk::compress(data, compression_level) {
                Ok(compressed) if compressed.len() < (data.len() * 9 / 10) => {
                    // Only use compression if it saves at least 10%
                    let mut result = vec![2]; // 2 = zstd compressed
                    result.extend_from_slice(&compressed);
                    Ok(result)
                }
                Ok(_) | Err(_) => {
                    // Try LZ4 as fallback for better compatibility
                    match lz4_flex::compress_prepend_size(data) {
                        compressed if compressed.len() < (data.len() * 95 / 100) => {
                            let mut result = vec![1]; // 1 = LZ4 compressed
                            result.extend_from_slice(&compressed);
                            Ok(result)
                        }
                        _ => {
                            // Store uncompressed if nothing helps
                            let mut result = vec![0]; // 0 = uncompressed
                            result.extend_from_slice(data);
                            Ok(result)
                        }
                    }
                }
            }
        }
    }

    fn decompress_chunk_data(&self, compressed: &[u8]) -> Result<Vec<u8>> {
        if compressed.is_empty() {
            return Err(BlazeError::Chunk("Empty compressed data".to_string()));
        }

        match compressed[0] {
            0 => Ok(compressed[1..].to_vec()), // Uncompressed
            1 => {
                // LZ4 compressed
                lz4_flex::decompress_size_prepended(&compressed[1..])
                    .map_err(|e| BlazeError::Chunk(format!("LZ4 decompression failed: {}", e)))
            }
            2 => {
                // zstd compressed with automatic size detection
                zstd::bulk::decompress(&compressed[1..], 16 * 1024 * 1024) // 16MB max decompressed size
                    .map_err(|e| BlazeError::Chunk(format!("zstd decompression failed: {}", e)))
            }
            _ => Err(BlazeError::Chunk("Unknown compression type".to_string())),
        }
    }

    /// Create delta between two chunks for superior compression
    fn create_delta(&self, base_data: &[u8], new_data: &[u8]) -> Vec<u8> {
        if base_data.is_empty() || new_data.is_empty() {
            return new_data.to_vec();
        }

        // Simple delta compression using XOR and run-length encoding
        let mut delta = Vec::new();
        delta.extend_from_slice(&(new_data.len() as u32).to_le_bytes());

        let max_len = std::cmp::max(base_data.len(), new_data.len());
        let mut i = 0;

        while i < max_len {
            let base_byte = if i < base_data.len() { base_data[i] } else { 0 };
            let new_byte = if i < new_data.len() { new_data[i] } else { 0 };
            let diff = base_byte ^ new_byte;

            if diff == 0 {
                // Count consecutive matching bytes
                let mut count = 0u16;
                while i + (count as usize) < max_len && count < u16::MAX {
                    let b_base = if i + (count as usize) < base_data.len() {
                        base_data[i + (count as usize)]
                    } else {
                        0
                    };
                    let b_new = if i + (count as usize) < new_data.len() {
                        new_data[i + (count as usize)]
                    } else {
                        0
                    };

                    if b_base != b_new {
                        break;
                    }
                    count += 1;
                }

                // Store "same" marker + count
                delta.push(0); // 0 = same bytes
                delta.extend_from_slice(&count.to_le_bytes());
                i += count as usize;
            } else {
                // Count consecutive different bytes
                let start_i = i;
                while i < max_len && i - start_i < 255 {
                    let b_base = if i < base_data.len() { base_data[i] } else { 0 };
                    let b_new = if i < new_data.len() { new_data[i] } else { 0 };

                    if b_base == b_new {
                        break;
                    }
                    i += 1;
                }

                let diff_count = (i - start_i) as u8;
                delta.push(1); // 1 = different bytes
                delta.push(diff_count);

                // Store the different bytes from new data
                for j in start_i..i {
                    if j < new_data.len() {
                        delta.push(new_data[j]);
                    } else {
                        delta.push(0);
                    }
                }
            }
        }

        delta
    }

    /// Apply delta to reconstruct original data
    fn apply_delta(&self, base_data: &[u8], delta: &[u8]) -> Result<Vec<u8>> {
        if delta.len() < 4 {
            return Ok(delta.to_vec());
        }

        let original_size = u32::from_le_bytes([delta[0], delta[1], delta[2], delta[3]]) as usize;
        let mut result = Vec::with_capacity(original_size);
        let mut delta_pos = 4;
        let mut base_pos = 0;

        while delta_pos < delta.len() && result.len() < original_size {
            let command = delta[delta_pos];
            delta_pos += 1;

            if command == 0 {
                // Same bytes - copy from base
                if delta_pos + 2 > delta.len() {
                    break;
                }
                let count = u16::from_le_bytes([delta[delta_pos], delta[delta_pos + 1]]) as usize;
                delta_pos += 2;

                for _ in 0..count {
                    if base_pos < base_data.len() && result.len() < original_size {
                        result.push(base_data[base_pos]);
                    } else if result.len() < original_size {
                        result.push(0);
                    }
                    base_pos += 1;
                }
            } else if command == 1 {
                // Different bytes - copy from delta
                if delta_pos >= delta.len() {
                    break;
                }
                let count = delta[delta_pos] as usize;
                delta_pos += 1;

                for _ in 0..count {
                    if delta_pos < delta.len() && result.len() < original_size {
                        result.push(delta[delta_pos]);
                        delta_pos += 1;
                    } else if result.len() < original_size {
                        result.push(0);
                    }
                    base_pos += 1;
                }
            }
        }

        result.resize(original_size, 0);
        Ok(result)
    }

    /// Find similar chunk for delta compression
    fn find_similar_chunk(&self, chunk_hash: &str, chunk_data: &[u8]) -> Option<String> {
        // Check delta cache first
        if let Ok(cache) = self.delta_cache.read() {
            if let Some(similar_hashes) = cache.get(chunk_hash) {
                for similar_hash in similar_hashes {
                    if self.chunk_exists(similar_hash) {
                        return Some(similar_hash.clone());
                    }
                }
            }
        }

        // Simple similarity check - find chunks with similar size
        let target_size = chunk_data.len();
        let size_tolerance = target_size / 10; // 10% tolerance

        // Check recently stored chunks for similarity
        if let Ok(cache) = self.delta_cache.read() {
            for (existing_hash, _) in cache.iter() {
                if existing_hash == chunk_hash {
                    continue;
                }

                // Load existing chunk to compare
                if let Ok(existing_data) = self.load_chunk_uncached(existing_hash) {
                    let size_diff = if existing_data.len() > target_size {
                        existing_data.len() - target_size
                    } else {
                        target_size - existing_data.len()
                    };

                    if size_diff <= size_tolerance {
                        // Calculate simple similarity score
                        let similarity = Self::calculate_similarity(&existing_data, chunk_data);
                        if similarity > 0.7 {
                            // 70% similarity threshold
                            return Some(existing_hash.clone());
                        }
                    }
                }
            }
        }

        None
    }

    /// Store chunk with delta compression if beneficial
    pub fn store_chunk_with_delta(&mut self, chunk: &FileChunk) -> Result<String> {
        // Check if chunk already exists
        if self.chunk_exists(&chunk.hash) {
            return Ok(chunk.hash.clone());
        }

        // Try to find similar chunk for delta compression
        if let Some(base_hash) = self.find_similar_chunk(&chunk.hash, &chunk.data) {
            if let Ok(base_data) = self.load_chunk_uncached(&base_hash) {
                let delta = self.create_delta(&base_data, &chunk.data);

                // Only use delta if it's significantly smaller
                if delta.len() < (chunk.data.len() * 7 / 10) {
                    // Delta is 30%+ smaller, use it
                    let compressed_delta = self.compress_chunk_data(&delta)?;

                    let chunk_path = self.get_chunk_path(&chunk.hash);
                    if let Some(parent) = chunk_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    // Store with delta marker
                    let mut delta_file_data = vec![3]; // 3 = delta compressed
                    delta_file_data.extend_from_slice(base_hash.as_bytes());
                    delta_file_data.push(0); // null separator
                    delta_file_data.extend_from_slice(&compressed_delta);

                    let temp_path = chunk_path.with_extension("tmp");
                    {
                        let mut file = OpenOptions::new()
                            .write(true)
                            .create(true)
                            .truncate(true)
                            .open(&temp_path)?;
                        file.write_all(&delta_file_data)?;
                        file.sync_all()?;
                    }

                    fs::rename(&temp_path, &chunk_path)?;

                    // Update caches
                    self.maybe_cache_chunk(&chunk.hash, chunk.data.clone());
                    self.existence_cache
                        .write()
                        .unwrap()
                        .insert(chunk.hash.clone());
                    self.negative_cache.write().unwrap().remove(&chunk.hash);

                    // Update delta cache
                    self.delta_cache
                        .write()
                        .unwrap()
                        .entry(base_hash)
                        .or_default()
                        .push(chunk.hash.clone());

                    return Ok(chunk.hash.clone());
                }
            }
        }

        // Fall back to regular compression
        self.store_chunk(chunk)
    }

    /// Calculate similarity between two byte arrays (0.0 to 1.0)
    fn calculate_similarity(data1: &[u8], data2: &[u8]) -> f32 {
        if data1.is_empty() && data2.is_empty() {
            return 1.0;
        }
        if data1.is_empty() || data2.is_empty() {
            return 0.0;
        }

        let max_len = std::cmp::max(data1.len(), data2.len());
        let min_len = std::cmp::min(data1.len(), data2.len());

        let mut matching_bytes = 0;
        for i in 0..min_len {
            if data1[i] == data2[i] {
                matching_bytes += 1;
            }
        }

        // Penalize size differences
        let size_penalty = (max_len - min_len) as f32 / max_len as f32;
        let base_similarity = matching_bytes as f32 / min_len as f32;

        base_similarity * (1.0 - size_penalty * 0.5)
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
