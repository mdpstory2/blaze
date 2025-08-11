//! Error types and handling for Blaze VCS

use std::fmt;

/// Result type alias for Blaze operations
pub type Result<T> = std::result::Result<T, BlazeError>;

/// Main error type for Blaze VCS operations
#[derive(Debug)]
pub enum BlazeError {
    /// I/O related errors
    Io(std::io::Error),
    /// Database related errors
    Database(rusqlite::Error),
    /// File system errors
    FileSystem(String),
    /// Repository errors
    Repository(String),
    /// Configuration errors
    Config(String),
    /// Chunk processing errors
    Chunk(String),
    /// Lock file errors
    Lock(String),
    /// Serialization/deserialization errors
    Serialization(String),
    /// Hash computation errors
    Hash(String),
    /// Path resolution errors
    Path(String),
    /// Permission errors
    Permission(String),
    /// Validation errors
    Validation(String),
    /// Network errors (for future remote operations)
    Network(String),
    /// Generic error with custom message
    Generic(String),
}

impl fmt::Display for BlazeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlazeError::Io(err) => write!(f, "I/O error: {}", err),
            BlazeError::Database(err) => write!(f, "Database error: {}", err),
            BlazeError::FileSystem(msg) => write!(f, "File system error: {}", msg),
            BlazeError::Repository(msg) => write!(f, "Repository error: {}", msg),
            BlazeError::Config(msg) => write!(f, "Configuration error: {}", msg),
            BlazeError::Chunk(msg) => write!(f, "Chunk processing error: {}", msg),
            BlazeError::Lock(msg) => write!(f, "Lock file error: {}", msg),
            BlazeError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            BlazeError::Hash(msg) => write!(f, "Hash computation error: {}", msg),
            BlazeError::Path(msg) => write!(f, "Path error: {}", msg),
            BlazeError::Permission(msg) => write!(f, "Permission error: {}", msg),
            BlazeError::Validation(msg) => write!(f, "Validation error: {}", msg),
            BlazeError::Network(msg) => write!(f, "Network error: {}", msg),
            BlazeError::Generic(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for BlazeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BlazeError::Io(err) => Some(err),
            BlazeError::Database(err) => Some(err),
            _ => None,
        }
    }
}

// Automatic conversions from common error types
impl From<std::io::Error> for BlazeError {
    fn from(err: std::io::Error) -> Self {
        BlazeError::Io(err)
    }
}

impl From<rusqlite::Error> for BlazeError {
    fn from(err: rusqlite::Error) -> Self {
        BlazeError::Database(err)
    }
}

impl From<serde_json::Error> for BlazeError {
    fn from(err: serde_json::Error) -> Self {
        BlazeError::Serialization(err.to_string())
    }
}

impl From<walkdir::Error> for BlazeError {
    fn from(err: walkdir::Error) -> Self {
        BlazeError::FileSystem(err.to_string())
    }
}

impl From<anyhow::Error> for BlazeError {
    fn from(err: anyhow::Error) -> Self {
        BlazeError::Generic(err.to_string())
    }
}

// Helper macros for creating specific error types
#[macro_export]
macro_rules! repository_error {
    ($msg:expr) => {
        $crate::errors::BlazeError::Repository($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::errors::BlazeError::Repository(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! config_error {
    ($msg:expr) => {
        $crate::errors::BlazeError::Config($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::errors::BlazeError::Config(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! chunk_error {
    ($msg:expr) => {
        $crate::errors::BlazeError::Chunk($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::errors::BlazeError::Chunk(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! validation_error {
    ($msg:expr) => {
        $crate::errors::BlazeError::Validation($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::errors::BlazeError::Validation(format!($fmt, $($arg)*))
    };
}

/// Extension trait for Results to add context easily
pub trait ResultExt<T> {
    /// Add context to an error
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// Add static context to an error
    fn context(self, msg: &str) -> Result<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Into<BlazeError>,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|err| {
            let base_err = err.into();
            let context = f();
            BlazeError::Generic(format!("{}: {}", context, base_err))
        })
    }

    fn context(self, msg: &str) -> Result<T> {
        self.with_context(|| msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = BlazeError::Repository("test error".to_string());
        assert_eq!(err.to_string(), "Repository error: test error");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let blaze_err: BlazeError = io_err.into();

        match blaze_err {
            BlazeError::Io(_) => (),
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn test_result_ext() {
        let result: std::result::Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));

        let blaze_result = result.context("additional context");
        assert!(blaze_result.is_err());
        assert!(blaze_result
            .unwrap_err()
            .to_string()
            .contains("additional context"));
    }

    #[test]
    fn test_error_macros() {
        let err = repository_error!("test message");
        match err {
            BlazeError::Repository(msg) => assert_eq!(msg, "test message"),
            _ => panic!("Expected Repository error"),
        }

        let err = config_error!("config {} failed", "test");
        match err {
            BlazeError::Config(msg) => assert_eq!(msg, "config test failed"),
            _ => panic!("Expected Config error"),
        }
    }
}
