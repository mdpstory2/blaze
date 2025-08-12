//! # Blaze
//!
//! A blazingly fast, chunk-based version control system designed to be faster and easier than Git.
//!
//! Blaze uses advanced chunking algorithms and parallel processing to provide lightning-fast
//! version control operations while maintaining data integrity and ease of use.

pub mod chunks;
pub mod cli;
pub mod config;
pub mod core;
pub mod database;
pub mod errors;
pub mod files;
pub mod utils;

// Re-export main types for convenience
pub use crate::cli::{Cli, Commands};
pub use crate::config::*;
pub use crate::core::Blaze;
pub use crate::errors::{BlazeError, Result};
pub use crate::files::FileRecord;

/// Initialize and run the Blaze CLI
pub fn run() -> Result<()> {
    crate::cli::run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lib_exports() {
        // Basic smoke test to ensure exports work
        let _config = CHUNK_SIZE;
        assert_eq!(CHUNK_SIZE, 65536);
    }
}
