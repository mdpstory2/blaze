//! File record management and file-related operations for Blaze VCS

use crate::config::{CHUNK_SIZE, LARGE_FILE_THRESHOLD};
use crate::errors::{BlazeError, Result, ResultExt};
use crate::utils::{get_mtime, is_binary_file};
use blake3::Hasher;
use memmap2::MmapOptions;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Represents a file record in the Blaze VCS
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileRecord {
    /// Relative path of the file from repository root
    pub path: String,
    /// List of chunk hashes that make up this file
    pub chunks: Vec<String>,
    /// Total size of the file in bytes
    pub size: u64,
    /// Modification time as Unix timestamp
    pub mtime: u64,
    /// File permissions (Unix style)
    pub permissions: u32,
    /// Whether the file is executable
    pub is_executable: bool,
}

impl FileRecord {
    /// Create a new FileRecord from a file path
    pub fn from_path<P: AsRef<Path>>(
        file_path: P,
        repo_root: P,
        chunks: Vec<String>,
    ) -> Result<Self> {
        let file_path = file_path.as_ref();
        let repo_root = repo_root.as_ref();

        let relative_path = file_path
            .strip_prefix(repo_root)
            .map_err(|e| BlazeError::Path(format!("Invalid file path: {}", e)))?;

        let metadata = std::fs::metadata(file_path).context("Failed to read file metadata")?;

        let mtime = get_mtime(file_path)?;
        let permissions = metadata.permissions().mode();
        let is_executable = permissions & 0o111 != 0;
        let size = metadata.len();

        Ok(FileRecord {
            path: relative_path.to_string_lossy().to_string(),
            chunks,
            size,
            mtime,
            permissions,
            is_executable,
        })
    }

    /// Check if this file record is different from the current file state
    pub fn is_different_from_disk<P: AsRef<Path>>(&self, repo_root: P) -> Result<bool> {
        let file_path = repo_root.as_ref().join(&self.path);

        if !file_path.exists() {
            return Ok(true);
        }

        let current_mtime = get_mtime(&file_path)?;
        if current_mtime != self.mtime {
            return Ok(true);
        }

        let metadata =
            std::fs::metadata(&file_path).context("Failed to read current file metadata")?;

        if metadata.len() != self.size {
            return Ok(true);
        }

        let current_permissions = metadata.permissions().mode();
        if current_permissions != self.permissions {
            return Ok(true);
        }

        Ok(false)
    }

    /// Get the total number of chunks for this file
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Check if this is a binary file based on its extension
    pub fn is_binary(&self) -> bool {
        is_binary_file(&self.path)
    }

    /// Get a human-readable summary of this file record
    pub fn summary(&self) -> String {
        format!(
            "{} ({} chunks, {} bytes, {})",
            self.path,
            self.chunks.len(),
            crate::utils::format_size(self.size),
            if self.is_executable {
                "executable"
            } else {
                "regular"
            }
        )
    }
}

/// Represents a chunk of file data
#[derive(Debug, Clone)]
pub struct FileChunk {
    /// Hash of the chunk content
    pub hash: String,
    /// Size of the chunk in bytes
    pub size: usize,
    /// Raw chunk data
    pub data: Vec<u8>,
}

impl FileChunk {
    /// Create a new chunk from raw data
    pub fn new(data: Vec<u8>) -> Self {
        let hash = compute_chunk_hash(&data);
        let size = data.len();

        FileChunk { hash, size, data }
    }

    /// Verify that the chunk data matches its hash
    pub fn verify(&self) -> bool {
        compute_chunk_hash(&self.data) == self.hash
    }
}

/// Compute the BLAKE3 hash of chunk data
pub fn compute_chunk_hash(data: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize().to_hex().to_string()
}

/// Chunk a file into smaller pieces for storage
pub fn chunk_file<P: AsRef<Path>>(file_path: P) -> Result<Vec<FileChunk>> {
    let file_path = file_path.as_ref();
    let file = File::open(file_path)
        .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

    let file_size = file.metadata()?.len();

    if file_size > LARGE_FILE_THRESHOLD {
        chunk_large_file(file, file_size)
    } else {
        chunk_regular_file(file)
    }
}

/// Chunk a regular-sized file using buffered reading
fn chunk_regular_file(mut file: File) -> Result<Vec<FileChunk>> {
    let mut chunks = Vec::new();
    let mut buffer = vec![0u8; CHUNK_SIZE];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        let chunk_data = buffer[..bytes_read].to_vec();
        chunks.push(FileChunk::new(chunk_data));
    }

    Ok(chunks)
}

/// Chunk a large file using memory mapping for better performance
fn chunk_large_file(file: File, file_size: u64) -> Result<Vec<FileChunk>> {
    let mmap = unsafe { MmapOptions::new().map(&file).map_err(BlazeError::Io)? };

    let chunk_count = (file_size as usize).div_ceil(CHUNK_SIZE);
    let chunks: Result<Vec<_>> = (0..chunk_count)
        .into_par_iter()
        .map(|i| {
            let start = i * CHUNK_SIZE;
            let end = std::cmp::min(start + CHUNK_SIZE, mmap.len());
            let chunk_data = mmap[start..end].to_vec();
            Ok(FileChunk::new(chunk_data))
        })
        .collect();

    chunks
}

/// Reconstruct a file from its chunks
pub fn reconstruct_file_from_chunks(chunks: &[FileChunk]) -> Vec<u8> {
    let total_size: usize = chunks.iter().map(|c| c.size).sum();
    let mut result = Vec::with_capacity(total_size);

    for chunk in chunks {
        result.extend_from_slice(&chunk.data);
    }

    result
}

/// File processing statistics
#[derive(Debug, Default)]
pub struct FileStats {
    /// Total number of files processed
    pub total_files: usize,
    /// Total number of chunks created
    pub total_chunks: usize,
    /// Total bytes processed
    pub total_bytes: u64,
    /// Number of binary files
    pub binary_files: usize,
    /// Number of executable files
    pub executable_files: usize,
    /// Largest file size encountered
    pub largest_file: u64,
    /// Average file size
    pub average_file_size: f64,
}

impl FileStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Add statistics from a file record
    pub fn add_file(&mut self, record: &FileRecord) {
        self.total_files += 1;
        self.total_chunks += record.chunks.len();
        self.total_bytes += record.size;

        if record.is_binary() {
            self.binary_files += 1;
        }

        if record.is_executable {
            self.executable_files += 1;
        }

        if record.size > self.largest_file {
            self.largest_file = record.size;
        }

        self.average_file_size = self.total_bytes as f64 / self.total_files as f64;
    }

    /// Get a formatted summary of the statistics
    pub fn summary(&self) -> String {
        format!(
            "Files: {}, Chunks: {}, Total: {}, Binary: {}, Executable: {}, Avg size: {}",
            self.total_files,
            self.total_chunks,
            crate::utils::format_size(self.total_bytes),
            self.binary_files,
            self.executable_files,
            crate::utils::format_size(self.average_file_size as u64),
        )
    }
}

/// File change detection utilities
pub mod changes {
    use super::*;

    /// Types of file changes
    #[derive(Debug, Clone, PartialEq)]
    pub enum FileChangeType {
        /// File was added
        Added,
        /// File was modified
        Modified,
        /// File was deleted
        Deleted,
        /// File was renamed
        Renamed(String), // old path
    }

    /// Represents a file change
    #[derive(Debug, Clone)]
    pub struct FileChange {
        /// Path of the changed file
        pub path: String,
        /// Type of change
        pub change_type: FileChangeType,
        /// New file record (None for deletions)
        pub new_record: Option<FileRecord>,
        /// Old file record (None for additions)
        pub old_record: Option<FileRecord>,
    }

    impl FileChange {
        /// Create a new file addition change
        pub fn added(record: FileRecord) -> Self {
            let path = record.path.clone();
            FileChange {
                path,
                change_type: FileChangeType::Added,
                new_record: Some(record),
                old_record: None,
            }
        }

        /// Create a new file modification change
        pub fn modified(old_record: FileRecord, new_record: FileRecord) -> Self {
            let path = new_record.path.clone();
            FileChange {
                path,
                change_type: FileChangeType::Modified,
                new_record: Some(new_record),
                old_record: Some(old_record),
            }
        }

        /// Create a new file deletion change
        pub fn deleted(record: FileRecord) -> Self {
            let path = record.path.clone();
            FileChange {
                path,
                change_type: FileChangeType::Deleted,
                new_record: None,
                old_record: Some(record),
            }
        }

        /// Get a human-readable description of this change
        pub fn description(&self) -> String {
            match &self.change_type {
                FileChangeType::Added => format!("+ {}", self.path),
                FileChangeType::Modified => format!("M {}", self.path),
                FileChangeType::Deleted => format!("- {}", self.path),
                FileChangeType::Renamed(old_path) => format!("R {} -> {}", old_path, self.path),
            }
        }
    }

    /// Detect changes between two sets of file records
    pub fn detect_changes(
        old_records: &HashMap<String, FileRecord>,
        new_records: &HashMap<String, FileRecord>,
    ) -> Vec<FileChange> {
        let mut changes = Vec::new();

        // Find additions and modifications
        for (path, new_record) in new_records {
            if let Some(old_record) = old_records.get(path) {
                if *old_record != *new_record {
                    changes.push(FileChange::modified(old_record.clone(), new_record.clone()));
                }
            } else {
                changes.push(FileChange::added(new_record.clone()));
            }
        }

        // Find deletions
        for (path, old_record) in old_records {
            if !new_records.contains_key(path) {
                changes.push(FileChange::deleted(old_record.clone()));
            }
        }

        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::current_timestamp;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let file_path = dir.join(name);
        let mut file = File::create(&file_path).unwrap();
        file.write_all(content).unwrap();
        file_path
    }

    #[test]
    fn test_file_record_creation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = create_test_file(temp_dir.path(), "test.txt", b"Hello, world!");

        let chunks = vec!["abc123".to_string()];
        let record =
            FileRecord::from_path(&file_path, &temp_dir.path().to_path_buf(), chunks.clone())
                .unwrap();

        assert_eq!(record.path, "test.txt");
        assert_eq!(record.chunks, chunks);
        assert_eq!(record.size, 13);
    }

    #[test]
    fn test_chunk_computation() {
        let data = b"Hello, world!";
        let hash = compute_chunk_hash(data);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // BLAKE3 produces 256-bit hashes (64 hex chars)
    }

    #[test]
    fn test_file_chunking() {
        let temp_dir = TempDir::new().unwrap();
        let content = b"Hello, world! This is a test file.";
        let file_path = create_test_file(temp_dir.path(), "test.txt", content);

        let chunks = chunk_file(&file_path).unwrap();
        assert_eq!(chunks.len(), 1); // Small file should be one chunk

        let reconstructed = reconstruct_file_from_chunks(&chunks);
        assert_eq!(reconstructed, content);
    }

    #[test]
    fn test_file_chunk_verification() {
        let data = vec![1, 2, 3, 4, 5];
        let chunk = FileChunk::new(data);
        assert!(chunk.verify());

        let mut invalid_chunk = chunk.clone();
        invalid_chunk.data[0] = 99;
        assert!(!invalid_chunk.verify());
    }

    #[test]
    fn test_file_stats() {
        let mut stats = FileStats::new();

        let record1 = FileRecord {
            path: "test1.txt".to_string(),
            chunks: vec!["hash1".to_string()],
            size: 100,
            mtime: current_timestamp(),
            permissions: 0o644,
            is_executable: false,
        };

        let record2 = FileRecord {
            path: "test2.exe".to_string(),
            chunks: vec!["hash2".to_string()],
            size: 200,
            mtime: current_timestamp(),
            permissions: 0o755,
            is_executable: true,
        };

        stats.add_file(&record1);
        stats.add_file(&record2);

        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.total_chunks, 2);
        assert_eq!(stats.total_bytes, 300);
        assert_eq!(stats.executable_files, 1);
        assert_eq!(stats.binary_files, 1); // .exe is considered binary
    }

    #[test]
    fn test_change_detection() {
        let mut old_records = HashMap::new();
        let mut new_records = HashMap::new();

        let record1 = FileRecord {
            path: "file1.txt".to_string(),
            chunks: vec!["hash1".to_string()],
            size: 100,
            mtime: 1000,
            permissions: 0o644,
            is_executable: false,
        };

        let record1_modified = FileRecord {
            path: "file1.txt".to_string(),
            chunks: vec!["hash1_new".to_string()],
            size: 150,
            mtime: 2000,
            permissions: 0o644,
            is_executable: false,
        };

        let record2 = FileRecord {
            path: "file2.txt".to_string(),
            chunks: vec!["hash2".to_string()],
            size: 200,
            mtime: 1000,
            permissions: 0o644,
            is_executable: false,
        };

        // Old state: file1, file2
        old_records.insert("file1.txt".to_string(), record1.clone());
        old_records.insert("file2.txt".to_string(), record2.clone());

        // New state: file1 (modified), file3 (new)
        new_records.insert("file1.txt".to_string(), record1_modified.clone());
        new_records.insert("file3.txt".to_string(), record2.clone());

        let changes = changes::detect_changes(&old_records, &new_records);

        assert_eq!(changes.len(), 3);

        // Should have: Modified file1, Added file3, Deleted file2
        let change_types: Vec<_> = changes.iter().map(|c| &c.change_type).collect();
        assert!(change_types.contains(&&changes::FileChangeType::Modified));
        assert!(change_types.contains(&&changes::FileChangeType::Added));
        assert!(change_types.contains(&&changes::FileChangeType::Deleted));
    }
}
