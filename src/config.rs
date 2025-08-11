//! Configuration constants and settings for Blaze VCS

/// Size of chunks for file processing (64KB)
pub const CHUNK_SIZE: usize = 64 * 1024;

/// Threshold for considering a file "large" (100MB)
pub const LARGE_FILE_THRESHOLD: u64 = 100 * 1024 * 1024;

/// Name of the Blaze repository directory
pub const BLAZE_DIR: &str = ".blaze";

/// Name of the metadata database file
pub const DB_FILE: &str = "metadata.db";

/// Name of the chunks directory
pub const CHUNKS_DIR: &str = "chunks";

/// Name of the repository lock file
pub const LOCK_FILE: &str = "repo.lock";

/// Default commit message when none is provided
pub const DEFAULT_COMMIT_MESSAGE: &str = "Quick commit";

/// Get the maximum number of parallel threads for chunk processing
pub fn get_max_parallel_threads() -> usize {
    num_cpus::get().max(1)
}

/// Progress bar refresh rate in milliseconds
pub const PROGRESS_REFRESH_RATE: u64 = 100;

/// Database connection timeout in seconds
pub const DB_TIMEOUT: u32 = 30;

/// Maximum size for in-memory file processing before using disk buffering
pub const MAX_MEMORY_BUFFER: usize = 32 * 1024 * 1024; // 32MB

/// Compression level for chunk storage (0-9, where 9 is highest compression)
pub const COMPRESSION_LEVEL: u32 = 6;

/// File extensions that should always be treated as binary
pub const BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "bin", "dat", "db", "sqlite", "sqlite3", "jpg", "jpeg", "png",
    "gif", "bmp", "ico", "webp", "svg", "mp3", "wav", "flac", "ogg", "m4a", "aac", "mp4", "avi",
    "mkv", "mov", "wmv", "flv", "webm", "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "zip",
    "rar", "7z", "tar", "gz", "bz2", "xz",
];

/// File patterns to ignore by default (similar to .gitignore)
pub const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    ".blaze/",
    ".git/",
    ".svn/",
    ".hg/",
    "node_modules/",
    "target/",
    "build/",
    "dist/",
    "*.tmp",
    "*.temp",
    "*.swp",
    "*.swo",
    "*~",
    ".DS_Store",
    "Thumbs.db",
];

/// Application information
pub mod app_info {
    pub const NAME: &str = "blaze";
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
    pub const DESCRIPTION: &str = "A blazingly fast, chunk-based version control system";
    pub const AUTHOR: &str = "Blaze Contributors";
    pub const HOMEPAGE: &str = "https://github.com/blazevcs/blaze";
}

/// Performance tuning configuration
pub struct PerformanceConfig {
    /// Number of worker threads for parallel processing
    pub worker_threads: usize,
    /// Size of the read buffer for file I/O
    pub read_buffer_size: usize,
    /// Size of the write buffer for file I/O
    pub write_buffer_size: usize,
    /// Enable memory mapping for large files
    pub use_memory_mapping: bool,
    /// Enable compression for chunk storage
    pub enable_compression: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            worker_threads: get_max_parallel_threads(),
            read_buffer_size: CHUNK_SIZE,
            write_buffer_size: CHUNK_SIZE,
            use_memory_mapping: true,
            enable_compression: true,
        }
    }
}

/// Database configuration
pub struct DatabaseConfig {
    /// Connection timeout in seconds
    pub timeout: u32,
    /// Enable WAL mode for better concurrent access
    pub enable_wal_mode: bool,
    /// Cache size in KB
    pub cache_size: i32,
    /// Enable foreign key constraints
    pub enable_foreign_keys: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            timeout: DB_TIMEOUT,
            enable_wal_mode: true,
            cache_size: 8192, // 8MB
            enable_foreign_keys: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert!(CHUNK_SIZE > 0);
        assert!(LARGE_FILE_THRESHOLD > CHUNK_SIZE as u64);
        assert!(!BLAZE_DIR.is_empty());
        assert!(!DB_FILE.is_empty());
        assert!(!CHUNKS_DIR.is_empty());
        assert!(!LOCK_FILE.is_empty());
    }

    #[test]
    fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        assert!(config.worker_threads > 0);
        assert!(config.read_buffer_size > 0);
        assert!(config.write_buffer_size > 0);
    }

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert!(config.timeout > 0);
        assert!(config.cache_size > 0);
    }
}
