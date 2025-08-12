//! Database operations and schema management for Blaze VCS

use crate::config::{DatabaseConfig, DB_FILE};
use crate::errors::{BlazeError, Result, ResultExt};
use crate::files::FileRecord;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Database manager for Blaze VCS
pub struct Database {
    db_path: PathBuf,
    config: DatabaseConfig,
}

/// Represents a commit in the database
#[derive(Debug, Clone)]
pub struct CommitRecord {
    pub hash: String,
    pub parent: Option<String>,
    pub message: String,
    pub timestamp: u64,
    pub tree_hash: String,
    pub files: HashMap<String, FileRecord>,
}

/// Represents a reference (branch/tag) in the database
#[derive(Debug, Clone)]
pub struct RefRecord {
    pub name: String,
    pub commit_hash: Option<String>,
}

/// Represents a chunk record in the database
#[derive(Debug, Clone)]
pub struct ChunkRecord {
    pub hash: String,
    pub size: u64,
    pub created_at: u64,
}

impl Database {
    /// Create a new database instance
    pub fn new<P: AsRef<Path>>(blaze_dir: P) -> Result<Self> {
        let db_path = blaze_dir.as_ref().join(DB_FILE);
        let config = DatabaseConfig::default();

        Ok(Database { db_path, config })
    }

    /// Create a new database with custom configuration
    pub fn with_config<P: AsRef<Path>>(blaze_dir: P, config: DatabaseConfig) -> Result<Self> {
        let db_path = blaze_dir.as_ref().join(DB_FILE);

        Ok(Database { db_path, config })
    }

    /// Initialize the database schema
    pub fn init(&self) -> Result<()> {
        let conn = self.open_connection()?;

        conn.execute_batch(&format!(
            r#"
            PRAGMA foreign_keys = {};
            PRAGMA journal_mode = {};
            PRAGMA cache_size = -{};
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;

            CREATE TABLE IF NOT EXISTS chunks (
                hash TEXT PRIMARY KEY,
                size INTEGER NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );

            CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                chunks TEXT NOT NULL,
                size INTEGER NOT NULL,
                mtime INTEGER NOT NULL,
                permissions INTEGER NOT NULL,
                is_executable INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS commits (
                hash TEXT PRIMARY KEY,
                parent TEXT,
                message TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                tree_hash TEXT NOT NULL,
                files_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS refs (
                name TEXT PRIMARY KEY,
                commit_hash TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_created_at ON chunks(created_at);
            CREATE INDEX IF NOT EXISTS idx_commits_timestamp ON commits(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_commits_parent ON commits(parent);
            CREATE INDEX IF NOT EXISTS idx_files_mtime ON files(mtime);
            "#,
            if self.config.enable_foreign_keys {
                "ON"
            } else {
                "OFF"
            },
            if self.config.enable_wal_mode {
                "WAL"
            } else {
                "DELETE"
            },
            self.config.cache_size
        ))?;

        Ok(())
    }

    /// Store a chunk record
    pub fn store_chunk(&self, hash: &str, size: u64) -> Result<()> {
        let conn = self.open_connection()?;
        let timestamp = current_timestamp();

        conn.execute(
            "INSERT OR IGNORE INTO chunks (hash, size, created_at) VALUES (?, ?, ?)",
            params![hash, size as i64, timestamp as i64],
        )
        .context("Failed to store chunk record")?;

        Ok(())
    }

    /// Store multiple chunks in a transaction
    pub fn store_chunks(&self, chunks: &[(String, u64)]) -> Result<()> {
        let mut conn = self.open_connection()?;
        let tx = conn
            .transaction()
            .context("Failed to begin chunk storage transaction")?;

        let timestamp = current_timestamp();

        {
            let mut stmt = tx
                .prepare("INSERT OR IGNORE INTO chunks (hash, size, created_at) VALUES (?, ?, ?)")
                .context("Failed to prepare chunk insert statement")?;

            for (hash, size) in chunks {
                stmt.execute(params![hash, *size as i64, timestamp as i64])
                    .context("Failed to insert chunk record")?;
            }
        }

        tx.commit().context("Failed to commit chunk transaction")?;
        Ok(())
    }

    /// Check if a chunk exists
    pub fn chunk_exists(&self, hash: &str) -> Result<bool> {
        let conn = self.open_connection()?;

        let exists = conn
            .query_row(
                "SELECT 1 FROM chunks WHERE hash = ? LIMIT 1",
                params![hash],
                |_| Ok(()),
            )
            .optional()
            .context("Failed to check chunk existence")?;

        Ok(exists.is_some())
    }

    /// Get chunk record by hash
    pub fn get_chunk(&self, hash: &str) -> Result<Option<ChunkRecord>> {
        let conn = self.open_connection()?;

        let record = conn
            .query_row(
                "SELECT hash, size, created_at FROM chunks WHERE hash = ?",
                params![hash],
                |row| {
                    Ok(ChunkRecord {
                        hash: row.get(0)?,
                        size: row.get::<_, i64>(1)? as u64,
                        created_at: row.get::<_, i64>(2)? as u64,
                    })
                },
            )
            .optional()
            .context("Failed to get chunk record")?;

        Ok(record)
    }

    /// Get all chunk hashes
    pub fn get_all_chunk_hashes(&self) -> Result<Vec<String>> {
        let conn = self.open_connection()?;

        let mut stmt = conn
            .prepare("SELECT hash FROM chunks")
            .context("Failed to prepare chunk hash query")?;

        let hashes: Result<Vec<_>> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|row| row.map_err(BlazeError::from))
            .collect();

        hashes.context("Failed to collect chunk hashes")
    }

    /// Delete chunks by hash
    pub fn delete_chunks(&self, hashes: &[String]) -> Result<usize> {
        let mut conn = self.open_connection()?;
        let tx = conn
            .transaction()
            .context("Failed to begin chunk deletion transaction")?;

        let mut deleted_count = 0;

        {
            let mut stmt = tx
                .prepare("DELETE FROM chunks WHERE hash = ?")
                .context("Failed to prepare chunk deletion statement")?;

            for hash in hashes {
                let changes = stmt
                    .execute(params![hash])
                    .context("Failed to delete chunk")?;
                deleted_count += changes;
            }
        }

        tx.commit()
            .context("Failed to commit chunk deletion transaction")?;

        Ok(deleted_count)
    }

    /// Store or update a file record
    pub fn store_file(&self, record: &FileRecord) -> Result<()> {
        let conn = self.open_connection()?;

        let chunks_json =
            serde_json::to_string(&record.chunks).context("Failed to serialize file chunks")?;

        conn.execute(
            "INSERT OR REPLACE INTO files (path, chunks, size, mtime, permissions, is_executable) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                record.path,
                chunks_json,
                record.size as i64,
                record.mtime as i64,
                record.permissions as i64,
                if record.is_executable { 1 } else { 0 }
            ],
        ).context("Failed to store file record")?;

        Ok(())
    }

    /// Store multiple file records in a transaction
    pub fn store_files(&self, records: &[FileRecord]) -> Result<()> {
        let mut conn = self.open_connection()?;
        let tx = conn
            .transaction()
            .context("Failed to begin file storage transaction")?;

        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO files (path, chunks, size, mtime, permissions, is_executable) VALUES (?, ?, ?, ?, ?, ?)"
            ).context("Failed to prepare file insert statement")?;

            for record in records {
                let chunks_json = serde_json::to_string(&record.chunks)
                    .context("Failed to serialize file chunks")?;

                stmt.execute(params![
                    record.path,
                    chunks_json,
                    record.size as i64,
                    record.mtime as i64,
                    record.permissions as i64,
                    if record.is_executable { 1 } else { 0 }
                ])
                .context("Failed to insert file record")?;
            }
        }

        tx.commit().context("Failed to commit file transaction")?;
        Ok(())
    }

    /// Get a file record by path
    pub fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let conn = self.open_connection()?;

        let record = conn
            .query_row(
                "SELECT path, chunks, size, mtime, permissions, is_executable FROM files WHERE path = ?",
                params![path],
                parse_file_record,
            )
            .optional()
            .context("Failed to get file record")?;

        Ok(record)
    }

    /// Get all file records
    pub fn get_all_files(&self) -> Result<HashMap<String, FileRecord>> {
        let conn = self.open_connection()?;

        let mut stmt = conn
            .prepare("SELECT path, chunks, size, mtime, permissions, is_executable FROM files")
            .context("Failed to prepare file query")?;

        let files: Result<HashMap<_, _>> = stmt
            .query_map([], parse_file_record)?
            .map(|result| {
                let record = result?;
                Ok((record.path.clone(), record))
            })
            .collect();

        files.context("Failed to collect file records")
    }

    /// Delete a file record
    pub fn delete_file(&self, path: &str) -> Result<bool> {
        let conn = self.open_connection()?;

        let changes = conn
            .execute("DELETE FROM files WHERE path = ?", params![path])
            .context("Failed to delete file record")?;

        Ok(changes > 0)
    }

    /// Clear all file records
    pub fn clear_files(&self) -> Result<usize> {
        let conn = self.open_connection()?;

        let changes = conn
            .execute("DELETE FROM files", [])
            .context("Failed to clear file records")?;

        Ok(changes)
    }

    /// Store a commit record
    pub fn store_commit(&self, record: &CommitRecord) -> Result<()> {
        let conn = self.open_connection()?;

        let files_json =
            serde_json::to_string(&record.files).context("Failed to serialize commit files")?;

        conn.execute(
            "INSERT INTO commits (hash, parent, message, timestamp, tree_hash, files_json) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                record.hash,
                record.parent,
                record.message,
                record.timestamp as i64,
                record.tree_hash,
                files_json
            ],
        ).context("Failed to store commit record")?;

        Ok(())
    }

    /// Get a commit record by hash (supports partial hashes)
    pub fn get_commit(&self, hash_prefix: &str) -> Result<Option<CommitRecord>> {
        let conn = self.open_connection()?;

        let search_pattern = format!("{}%", hash_prefix);

        let record = conn
            .query_row(
                "SELECT hash, parent, message, timestamp, tree_hash, files_json FROM commits WHERE hash LIKE ? ORDER BY timestamp DESC LIMIT 1",
                params![search_pattern],
                parse_commit_record,
            )
            .optional()
            .context("Failed to get commit record")?;

        Ok(record)
    }

    /// Get commits with optional limit and parent filtering
    pub fn get_commits(
        &self,
        limit: Option<usize>,
        since: Option<&str>,
    ) -> Result<Vec<CommitRecord>> {
        let conn = self.open_connection()?;

        let (query, params): (String, Vec<rusqlite::types::Value>) = if let Some(since_hash) = since
        {
            (
                format!(
                    "SELECT hash, parent, message, timestamp, tree_hash, files_json FROM commits
                     WHERE timestamp >= (SELECT timestamp FROM commits WHERE hash LIKE ?)
                     ORDER BY timestamp DESC {}",
                    limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default()
                ),
                vec![format!("{}%", since_hash).into()],
            )
        } else {
            (
                format!(
                    "SELECT hash, parent, message, timestamp, tree_hash, files_json FROM commits
                     ORDER BY timestamp DESC {}",
                    limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default()
                ),
                vec![],
            )
        };

        let mut stmt = conn
            .prepare(&query)
            .context("Failed to prepare commit query")?;

        let commits: Result<Vec<_>> = stmt
            .query_map(rusqlite::params_from_iter(params), parse_commit_record)?
            .map(|row| row.map_err(BlazeError::from))
            .collect();

        commits.context("Failed to collect commit records")
    }

    /// Get commit count
    pub fn get_commit_count(&self) -> Result<usize> {
        let conn = self.open_connection()?;

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commits", [], |row| row.get(0))
            .context("Failed to get commit count")?;

        Ok(count as usize)
    }

    /// Store or update a reference
    pub fn store_ref(&self, name: &str, commit_hash: Option<&str>) -> Result<()> {
        let conn = self.open_connection()?;

        conn.execute(
            "INSERT OR REPLACE INTO refs (name, commit_hash) VALUES (?, ?)",
            params![name, commit_hash],
        )
        .context("Failed to store reference")?;

        Ok(())
    }

    /// Get a reference by name
    pub fn get_ref(&self, name: &str) -> Result<Option<RefRecord>> {
        let conn = self.open_connection()?;

        let record = conn
            .query_row(
                "SELECT name, commit_hash FROM refs WHERE name = ?",
                params![name],
                |row| {
                    Ok(RefRecord {
                        name: row.get(0)?,
                        commit_hash: row.get(1)?,
                    })
                },
            )
            .optional()
            .context("Failed to get reference")?;

        Ok(record)
    }

    /// Get all references
    pub fn get_all_refs(&self) -> Result<HashMap<String, RefRecord>> {
        let conn = self.open_connection()?;

        let mut stmt = conn
            .prepare("SELECT name, commit_hash FROM refs")
            .context("Failed to prepare refs query")?;

        let refs: Result<HashMap<_, _>> = stmt
            .query_map([], |row| {
                Ok(RefRecord {
                    name: row.get(0)?,
                    commit_hash: row.get(1)?,
                })
            })?
            .map(|result| {
                let record = result?;
                Ok((record.name.clone(), record))
            })
            .collect();

        refs.context("Failed to collect reference records")
    }

    /// Delete a reference
    pub fn delete_ref(&self, name: &str) -> Result<bool> {
        let conn = self.open_connection()?;

        let changes = conn
            .execute("DELETE FROM refs WHERE name = ?", params![name])
            .context("Failed to delete reference")?;

        Ok(changes > 0)
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DatabaseStats> {
        let conn = self.open_connection()?;

        let chunk_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .context("Failed to get chunk count")?;

        let file_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .context("Failed to get file count")?;

        let commit_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commits", [], |row| row.get(0))
            .context("Failed to get commit count")?;

        let ref_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM refs", [], |row| row.get(0))
            .context("Failed to get ref count")?;

        let total_chunk_size: Option<i64> = conn
            .query_row("SELECT SUM(size) FROM chunks", [], |row| row.get(0))
            .context("Failed to get total chunk size")?;

        let total_file_size: Option<i64> = conn
            .query_row("SELECT SUM(size) FROM files", [], |row| row.get(0))
            .context("Failed to get total file size")?;

        Ok(DatabaseStats {
            chunk_count: chunk_count as usize,
            file_count: file_count as usize,
            commit_count: commit_count as usize,
            ref_count: ref_count as usize,
            total_chunk_size: total_chunk_size.unwrap_or(0) as u64,
            total_file_size: total_file_size.unwrap_or(0) as u64,
        })
    }

    /// Vacuum the database to reclaim space
    pub fn vacuum(&self) -> Result<()> {
        let conn = self.open_connection()?;
        conn.execute_batch("VACUUM;")
            .context("Failed to vacuum database")?;
        Ok(())
    }

    /// Check database integrity
    pub fn check_integrity(&self) -> Result<Vec<String>> {
        let conn = self.open_connection()?;

        let mut stmt = conn
            .prepare("PRAGMA integrity_check")
            .context("Failed to prepare integrity check")?;

        let issues: Result<Vec<_>> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|row| row.map_err(BlazeError::from))
            .collect();

        let issues = issues.context("Failed to collect integrity check results")?;

        // Filter out "ok" results
        Ok(issues
            .into_iter()
            .filter(|issue: &String| issue.to_lowercase() != "ok")
            .collect())
    }

    // Private helper methods

    fn open_connection(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)
            .with_context(|| format!("Failed to open database: {}", self.db_path.display()))?;

        conn.busy_timeout(std::time::Duration::from_secs(self.config.timeout as u64))
            .context("Failed to set database timeout")?;

        Ok(conn)
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub chunk_count: usize,
    pub file_count: usize,
    pub commit_count: usize,
    pub ref_count: usize,
    pub total_chunk_size: u64,
    pub total_file_size: u64,
}

impl DatabaseStats {
    /// Get a formatted summary of the statistics
    pub fn summary(&self) -> String {
        format!(
            "Chunks: {}, Files: {}, Commits: {}, Refs: {}, Storage: {} chunks / {} files",
            self.chunk_count,
            self.file_count,
            self.commit_count,
            self.ref_count,
            crate::utils::format_size(self.total_chunk_size),
            crate::utils::format_size(self.total_file_size),
        )
    }
}

// Helper functions for parsing database rows

fn parse_file_record(row: &Row) -> rusqlite::Result<FileRecord> {
    let chunks_json: String = row.get(1)?;
    let chunks: Vec<String> = serde_json::from_str(&chunks_json).map_err(|_e| {
        rusqlite::Error::InvalidColumnType(1, "chunks".to_string(), rusqlite::types::Type::Text)
    })?;

    Ok(FileRecord {
        path: row.get(0)?,
        chunks,
        size: row.get::<_, i64>(2)? as u64,
        mtime: row.get::<_, i64>(3)? as u64,
        permissions: row.get::<_, i64>(4)? as u32,
        is_executable: row.get::<_, i64>(5)? != 0,
    })
}

fn parse_commit_record(row: &Row) -> rusqlite::Result<CommitRecord> {
    let files_json: String = row.get(5)?;
    let files: HashMap<String, FileRecord> = serde_json::from_str(&files_json).map_err(|_| {
        rusqlite::Error::InvalidColumnType(5, "files_json".to_string(), rusqlite::types::Type::Text)
    })?;

    Ok(CommitRecord {
        hash: row.get(0)?,
        parent: row.get(1)?,
        message: row.get(2)?,
        timestamp: row.get::<_, i64>(3)? as u64,
        tree_hash: row.get(4)?,
        files,
    })
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let config = DatabaseConfig {
            enable_foreign_keys: false, // Disable for tests
            ..Default::default()
        };
        let db = Database::with_config(temp_dir.path(), config).unwrap();
        db.init().unwrap();
        (temp_dir, db)
    }

    #[test]
    fn test_database_initialization() {
        let (_temp_dir, db) = create_test_db();

        // Check that we can get stats (which means tables were created)
        let stats = db.get_stats().unwrap();
        assert_eq!(stats.chunk_count, 0);
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.commit_count, 0);
    }

    #[test]
    fn test_chunk_operations() {
        let (_temp_dir, db) = create_test_db();

        let hash = "abc123def456";
        let size = 1024;

        // Store chunk
        db.store_chunk(hash, size).unwrap();

        // Check existence
        assert!(db.chunk_exists(hash).unwrap());
        assert!(!db.chunk_exists("nonexistent").unwrap());

        // Get chunk
        let chunk = db.get_chunk(hash).unwrap().unwrap();
        assert_eq!(chunk.hash, hash);
        assert_eq!(chunk.size, size);
    }

    #[test]
    fn test_file_operations() {
        let (_temp_dir, db) = create_test_db();

        let record = FileRecord {
            path: "test.txt".to_string(),
            chunks: vec!["chunk1".to_string(), "chunk2".to_string()],
            size: 100,
            mtime: 1234567890,
            permissions: 0o644,
            is_executable: false,
        };

        // Store file
        db.store_file(&record).unwrap();

        // Get file
        let retrieved = db.get_file("test.txt").unwrap().unwrap();
        assert_eq!(retrieved.path, record.path);
        assert_eq!(retrieved.chunks, record.chunks);
        assert_eq!(retrieved.size, record.size);

        // Delete file
        assert!(db.delete_file("test.txt").unwrap());
        assert!(!db.delete_file("test.txt").unwrap());
    }

    #[test]
    fn test_commit_operations() {
        let (_temp_dir, db) = create_test_db();

        let mut files = HashMap::new();
        files.insert(
            "test.txt".to_string(),
            FileRecord {
                path: "test.txt".to_string(),
                chunks: vec!["chunk1".to_string()],
                size: 50,
                mtime: 1234567890,
                permissions: 0o644,
                is_executable: false,
            },
        );

        let commit = CommitRecord {
            hash: "commit123".to_string(),
            parent: None,
            message: "Test commit".to_string(),
            timestamp: current_timestamp(),
            tree_hash: "tree123".to_string(),
            files,
        };

        // Store commit
        db.store_commit(&commit).unwrap();

        // Get commit (full hash)
        let retrieved = db.get_commit("commit123").unwrap().unwrap();
        assert_eq!(retrieved.hash, commit.hash);
        assert_eq!(retrieved.message, commit.message);

        // Get commit (partial hash)
        let retrieved = db.get_commit("commit").unwrap().unwrap();
        assert_eq!(retrieved.hash, commit.hash);

        // Get commits with limit
        let commits = db.get_commits(Some(1), None).unwrap();
        assert_eq!(commits.len(), 1);
    }

    #[test]
    fn test_ref_operations() {
        let (_temp_dir, db) = create_test_db();

        // Store ref
        db.store_ref("HEAD", Some("commit123")).unwrap();

        // Get ref
        let ref_record = db.get_ref("HEAD").unwrap().unwrap();
        assert_eq!(ref_record.name, "HEAD");
        assert_eq!(ref_record.commit_hash, Some("commit123".to_string()));

        // Update ref
        db.store_ref("HEAD", Some("commit456")).unwrap();
        let updated = db.get_ref("HEAD").unwrap().unwrap();
        assert_eq!(updated.commit_hash, Some("commit456".to_string()));

        // Delete ref
        assert!(db.delete_ref("HEAD").unwrap());
        assert!(db.get_ref("HEAD").unwrap().is_none());
    }

    #[test]
    fn test_database_stats() {
        let (_temp_dir, db) = create_test_db();

        // Add some data
        db.store_chunk("chunk1", 100).unwrap();
        db.store_chunk("chunk2", 200).unwrap();

        let file_record = FileRecord {
            path: "test.txt".to_string(),
            chunks: vec!["chunk1".to_string()],
            size: 100,
            mtime: 1234567890,
            permissions: 0o644,
            is_executable: false,
        };
        db.store_file(&file_record).unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.chunk_count, 2);
        assert_eq!(stats.file_count, 1);
        assert_eq!(stats.total_chunk_size, 300);
    }

    #[test]
    fn test_integrity_check() {
        let (_temp_dir, db) = create_test_db();

        let issues = db.check_integrity().unwrap();
        assert!(issues.is_empty()); // Should be no issues in a fresh database
    }
}
