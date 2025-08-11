//! Utility functions for Blaze VCS

use crate::config::BINARY_EXTENSIONS;
use crate::errors::{BlazeError, Result};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Format file sizes in human-readable format (B, KB, MB, GB, TB)
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Format duration in human-readable format (s, m s, h m)
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

/// Format elapsed time since a timestamp
pub fn format_elapsed_time(timestamp: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now >= timestamp {
        let elapsed = now - timestamp;
        if elapsed < 60 {
            "just now".to_string()
        } else if elapsed < 3600 {
            let minutes = elapsed / 60;
            format!(
                "{} minute{} ago",
                minutes,
                if minutes == 1 { "" } else { "s" }
            )
        } else if elapsed < 86400 {
            let hours = elapsed / 3600;
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else {
            let days = elapsed / 86400;
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        }
    } else {
        "in the future".to_string()
    }
}

/// Get the current Unix timestamp
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Check if a file should be treated as binary based on its extension
pub fn is_binary_file<P: AsRef<Path>>(path: P) -> bool {
    if let Some(extension) = path.as_ref().extension() {
        if let Some(ext_str) = extension.to_str() {
            return BINARY_EXTENSIONS.contains(&ext_str.to_lowercase().as_str());
        }
    }
    false
}

/// Normalize a path to use forward slashes and remove redundant components
pub fn normalize_path<P: AsRef<Path>>(path: P) -> String {
    path.as_ref()
        .to_string_lossy()
        .replace('\\', "/")
        .split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// Check if a path matches any of the ignore patterns
pub fn should_ignore_path<P: AsRef<Path>>(path: P, patterns: &[&str]) -> bool {
    let path_str = normalize_path(path);

    for pattern in patterns {
        if pattern.ends_with('/') {
            // Directory pattern
            let dir_pattern = &pattern[..pattern.len() - 1];
            if path_str.starts_with(dir_pattern)
                && (path_str.len() == dir_pattern.len()
                    || path_str.chars().nth(dir_pattern.len()) == Some('/'))
            {
                return true;
            }
        } else if pattern.starts_with("*.") {
            // Extension pattern
            let ext = &pattern[2..];
            if path_str.ends_with(&format!(".{}", ext)) {
                return true;
            }
        } else if pattern.contains('*') {
            // Simple glob pattern - basic implementation
            if simple_glob_match(pattern, &path_str) {
                return true;
            }
        } else {
            // Exact match
            if path_str == *pattern {
                return true;
            }
        }
    }
    false
}

/// Simple glob pattern matching (supports * wildcard)
fn simple_glob_match(pattern: &str, text: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('*').collect();

    if pattern_parts.len() == 1 {
        return pattern == text;
    }

    let mut text_pos = 0;

    for (i, part) in pattern_parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if i == 0 {
            // First part must match from the beginning
            if !text[text_pos..].starts_with(part) {
                return false;
            }
            text_pos += part.len();
        } else if i == pattern_parts.len() - 1 {
            // Last part must match at the end
            return text[text_pos..].ends_with(part);
        } else {
            // Middle part must be found somewhere
            if let Some(pos) = text[text_pos..].find(part) {
                text_pos += pos + part.len();
            } else {
                return false;
            }
        }
    }

    true
}

/// Create a progress bar with consistent styling
pub fn create_progress_bar(total: u64, message: &str) -> indicatif::ProgressBar {
    use indicatif::{ProgressBar, ProgressStyle};

    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} {msg}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    pb.set_message(message.to_string());
    pb
}

/// Safely read a file's metadata
pub fn safe_metadata<P: AsRef<Path>>(path: P) -> Result<std::fs::Metadata> {
    std::fs::metadata(path.as_ref()).map_err(|e| {
        BlazeError::FileSystem(format!(
            "Failed to read metadata for {}: {}",
            path.as_ref().display(),
            e
        ))
    })
}

/// Get file modification time as Unix timestamp
pub fn get_mtime<P: AsRef<Path>>(path: P) -> Result<u64> {
    let metadata = safe_metadata(path)?;
    let mtime = metadata
        .modified()
        .map_err(|e| BlazeError::FileSystem(format!("Failed to get modification time: {}", e)))?;

    let timestamp = mtime
        .duration_since(UNIX_EPOCH)
        .map_err(|e| BlazeError::FileSystem(format!("Invalid modification time: {}", e)))?
        .as_secs();

    Ok(timestamp)
}

/// Convert bytes to a hex string
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Convert hex string to bytes
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return Err(BlazeError::Validation(
            "Hex string must have even length".to_string(),
        ));
    }

    let mut bytes = Vec::new();
    for chunk in hex.as_bytes().chunks(2) {
        let hex_byte = std::str::from_utf8(chunk)
            .map_err(|e| BlazeError::Validation(format!("Invalid hex string: {}", e)))?;
        let byte = u8::from_str_radix(hex_byte, 16).map_err(|e| {
            BlazeError::Validation(format!("Invalid hex byte '{}': {}", hex_byte, e))
        })?;
        bytes.push(byte);
    }

    Ok(bytes)
}

/// Truncate a string to a maximum length with ellipsis
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m");
    }

    #[test]
    fn test_is_binary_file() {
        assert!(is_binary_file("test.exe"));
        assert!(is_binary_file("image.jpg"));
        assert!(!is_binary_file("script.sh"));
        assert!(!is_binary_file("readme.txt"));
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("./foo/bar"), "foo/bar");
        assert_eq!(normalize_path("foo\\bar"), "foo/bar");
        assert_eq!(normalize_path("foo//bar"), "foo/bar");
    }

    #[test]
    fn test_should_ignore_path() {
        let patterns = &[".git/", "*.tmp", "node_modules/"];

        assert!(should_ignore_path(".git/config", patterns));
        assert!(should_ignore_path("test.tmp", patterns));
        assert!(should_ignore_path("node_modules/package", patterns));
        assert!(!should_ignore_path("src/main.rs", patterns));
    }

    #[test]
    fn test_simple_glob_match() {
        assert!(simple_glob_match("*.txt", "readme.txt"));
        assert!(simple_glob_match("test*", "test123"));
        assert!(simple_glob_match("*test*", "mytest123"));
        assert!(!simple_glob_match("*.txt", "readme.md"));
    }

    #[test]
    fn test_bytes_to_hex() {
        assert_eq!(bytes_to_hex(&[0x00, 0xff, 0x42]), "00ff42");
        assert_eq!(bytes_to_hex(&[]), "");
    }

    #[test]
    fn test_hex_to_bytes() {
        assert_eq!(hex_to_bytes("00ff42").unwrap(), vec![0x00, 0xff, 0x42]);
        assert_eq!(hex_to_bytes("").unwrap(), Vec::<u8>::new());
        assert!(hex_to_bytes("0").is_err()); // Odd length
        assert!(hex_to_bytes("gg").is_err()); // Invalid hex
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 2), "hi");
        assert_eq!(truncate_string("test", 3), "...");
    }
}
