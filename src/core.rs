//! Core repository implementation for Blaze VCS

use crate::chunks::ChunkStore;
use crate::cli::UntrackedFiles;
use crate::config::{BLAZE_DIR, DEFAULT_IGNORE_PATTERNS, LOCK_FILE};
use crate::database::{CommitRecord, Database};
use crate::errors::{BlazeError, Result, ResultExt};
use crate::files::{changes::FileChange, chunk_file, FileRecord, FileStats};
use crate::utils::{
    create_progress_bar, current_timestamp, format_elapsed_time, format_size, should_ignore_path,
};
use blake3::Hasher;
use fs2::FileExt;

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Main Blaze VCS repository manager
pub struct Blaze {
    /// Path to the repository root
    pub repo_path: PathBuf,
    /// Path to the .blaze directory
    blaze_path: PathBuf,
    /// Database manager
    database: Database,
    /// Chunk storage manager
    chunk_store: ChunkStore,
    /// Repository lock file path
    lock_file: PathBuf,
}

impl Blaze {
    /// Create a new Blaze repository instance
    pub fn new<P: AsRef<Path>>(repo_path: P) -> Result<Self> {
        let repo_path = repo_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| repo_path.as_ref().to_path_buf());

        let blaze_path = repo_path.join(BLAZE_DIR);
        let chunks_path = blaze_path.join("chunks");
        let lock_file = blaze_path.join(LOCK_FILE);

        let database = Database::new(&blaze_path)?;
        let chunk_store = ChunkStore::new(&chunks_path)?;

        Ok(Self {
            repo_path,
            blaze_path,
            database,
            chunk_store,
            lock_file,
        })
    }

    /// Initialize a new Blaze repository
    pub fn init(&mut self, no_ignore: bool, chunk_size: Option<usize>) -> Result<()> {
        if self.is_repo() {
            println!("Repository already exists at {}", self.blaze_path.display());
            return Ok(());
        }

        println!(
            "🔥 Initializing Blaze repository in {}",
            self.repo_path.display()
        );

        // Create directory structure
        std::fs::create_dir_all(&self.blaze_path).context("Failed to create .blaze directory")?;

        // Initialize database
        self.database.init()?;

        // Initialize chunk store
        self.chunk_store = ChunkStore::new(self.blaze_path.join("chunks"))?;

        // Create initial HEAD ref
        self.database.store_ref("HEAD", None)?;

        // Create .blazeignore if requested
        if !no_ignore {
            self.create_blazeignore()?;
        }

        // Create config file if chunk size is specified
        if let Some(size) = chunk_size {
            self.create_config(size)?;
        }

        println!("✅ Repository initialized successfully!");
        Ok(())
    }

    /// Add files to the staging area
    pub fn add(
        &mut self,
        files: Vec<String>,
        verbose: bool,
        all: bool,
        dry_run: bool,
    ) -> Result<usize> {
        if !self.is_repo() {
            return Err(BlazeError::Repository(
                "Not a Blaze repository (or any parent directories)".to_string(),
            ));
        }

        let _lock = self.acquire_lock()?;

        if all {
            // Add all files in repository
            let all_files = self.find_all_files()?;
            self.add_files(all_files, verbose, dry_run)
        } else if files.is_empty() {
            // Add all modified files
            let modified_files = self.find_modified_files()?;
            self.add_files(modified_files, verbose, dry_run)
        } else {
            // Add specific files/patterns
            let mut files_to_add = Vec::new();
            for pattern in files {
                let matched = self.find_files_matching(&pattern)?;
                files_to_add.extend(matched);
            }
            self.add_files(files_to_add, verbose, dry_run)
        }
    }

    /// Create a new commit
    pub fn commit(
        &mut self,
        message: String,
        all: bool,
        verbose: bool,
        allow_empty: bool,
    ) -> Result<String> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        let _lock = self.acquire_lock()?;

        // Auto-stage if requested
        if all {
            let modified = self.find_modified_files()?;
            self.add_files(modified, verbose, false)?;
        }

        // Get staged files
        let staged_files = self.database.get_all_files()?;

        if staged_files.is_empty() && !allow_empty {
            return Err(BlazeError::Repository(
                "No changes to commit (use --allow-empty to commit anyway)".to_string(),
            ));
        }

        // Get parent commit
        let parent_hash = self.get_head_commit()?;

        // Create commit hash
        let timestamp = current_timestamp();
        let commit_data = format!(
            "parent: {:?}\nmessage: {}\ntimestamp: {}\nfiles: {}",
            parent_hash,
            message.trim(),
            timestamp,
            staged_files.len()
        );
        let commit_hash = self.hash_data(commit_data.as_bytes());

        // Create tree hash from staged files
        let tree_hash = self.create_tree_hash(&staged_files)?;

        if verbose {
            println!("Creating commit with {} files", staged_files.len());
            for path in staged_files.keys() {
                println!("  {}", path);
            }
        }

        // Store commit
        let commit_record = CommitRecord {
            hash: commit_hash.clone(),
            parent: parent_hash,
            message: message.trim().to_string(),
            timestamp,
            tree_hash,
            files: staged_files,
        };

        self.database.store_commit(&commit_record)?;

        // Update HEAD
        self.database.store_ref("HEAD", Some(&commit_hash))?;

        Ok(commit_hash)
    }

    /// Show commit history
    pub fn log(
        &self,
        limit: usize,
        oneline: bool,
        stat: bool,
        since: Option<String>,
    ) -> Result<()> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        let commits = self.database.get_commits(Some(limit), since.as_deref())?;

        if commits.is_empty() {
            println!("No commits found");
            return Ok(());
        }

        for commit in commits {
            if oneline {
                println!(
                    "{} {}",
                    &commit.hash[..8],
                    commit.message.lines().next().unwrap_or("")
                );
            } else {
                println!("Commit: {}", commit.hash);
                if let Some(ref parent) = commit.parent {
                    println!("Parent: {}", parent);
                }
                println!("Date: {}", format_elapsed_time(commit.timestamp));
                println!("Message: {}", commit.message);

                if stat {
                    println!(
                        "Files: {} ({})",
                        commit.files.len(),
                        format_size(commit.files.values().map(|f| f.size).sum::<u64>())
                    );
                }
                println!();
            }
        }

        Ok(())
    }

    /// Show working tree status
    pub fn status(
        &self,
        short: bool,
        ignored: bool,
        untracked_files: UntrackedFiles,
    ) -> Result<()> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        // Get current state
        let staged_files = self.database.get_all_files()?;
        let working_files = self.scan_working_directory()?;
        let head_commit = self.get_head_commit();

        // Compare with HEAD
        let committed_files = if let Ok(Some(head_hash)) = head_commit {
            if let Ok(Some(commit)) = self.database.get_commit(&head_hash) {
                commit.files
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        // Detect changes
        let staged_changes = crate::files::changes::detect_changes(&committed_files, &staged_files);
        let working_changes = crate::files::changes::detect_changes(&staged_files, &working_files);

        if short {
            self.print_short_status(&staged_changes, &working_changes)?;
        } else {
            self.print_long_status(&staged_changes, &working_changes, ignored, untracked_files)?;
        }

        Ok(())
    }

    /// Checkout a specific commit
    pub fn checkout(&mut self, target: &str, force: bool) -> Result<()> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        let _lock = self.acquire_lock()?;

        // Check for uncommitted changes
        if !force && self.has_uncommitted_changes()? {
            return Err(BlazeError::Repository(
                "You have uncommitted changes. Use --force to override".to_string(),
            ));
        }

        // Find the commit
        let commit = self
            .database
            .get_commit(target)?
            .ok_or_else(|| BlazeError::Repository(format!("Commit not found: {}", target)))?;

        // Restore files
        self.restore_files(&commit.files)?;

        // Update HEAD
        self.database.store_ref("HEAD", Some(&commit.hash))?;

        println!("HEAD is now at {} {}", &commit.hash[..8], commit.message);
        Ok(())
    }

    /// Create a new branch
    pub fn create_branch(&self, name: &str) -> Result<()> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        let head_commit = self.get_head_commit()?;
        self.database.store_ref(name, head_commit.as_deref())?;
        Ok(())
    }

    /// Delete a branch
    pub fn delete_branch(&self, name: &str, _force: bool) -> Result<()> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        if name == "HEAD" {
            return Err(BlazeError::Repository("Cannot delete HEAD".to_string()));
        }

        // TODO: Check if branch is merged unless force is true

        if self.database.delete_ref(name)? {
            Ok(())
        } else {
            Err(BlazeError::Repository(format!(
                "Branch not found: {}",
                name
            )))
        }
    }

    /// List all branches
    pub fn list_branches(&self, all: bool) -> Result<()> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        let refs = self.database.get_all_refs()?;
        let current_head = self.get_head_commit().ok().flatten();

        for (name, ref_record) in refs {
            if !all && name == "HEAD" {
                continue;
            }

            let marker =
                if Some(&ref_record.commit_hash.unwrap_or_default()) == current_head.as_ref() {
                    "*"
                } else {
                    " "
                };

            println!("{} {}", marker, name);
        }

        Ok(())
    }

    /// Show repository statistics
    pub fn show_stats(&self, chunks: bool, files: bool, storage: bool) -> Result<()> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        let db_stats = self.database.get_stats()?;

        println!("📊 Blaze Repository Statistics");
        println!("═══════════════════════════════");
        println!("Commits: {}", db_stats.commit_count);
        println!("Files tracked: {}", db_stats.file_count);
        println!("Chunks stored: {}", db_stats.chunk_count);
        println!("References: {}", db_stats.ref_count);

        if storage {
            println!("\n💾 Storage Information");
            println!("─────────────────────");
            println!(
                "Total chunk storage: {}",
                format_size(db_stats.total_chunk_size)
            );
            println!("Total file size: {}", format_size(db_stats.total_file_size));

            if db_stats.total_file_size > 0 {
                let ratio = db_stats.total_chunk_size as f64 / db_stats.total_file_size as f64;
                println!("Storage efficiency: {:.1}%", (1.0 - ratio) * 100.0);
            }
        }

        if chunks {
            println!("\n🧩 Chunk Information");
            println!("───────────────────");
            let chunk_count = self.chunk_store.chunk_count()?;
            let total_size = self.chunk_store.total_storage_size()?;
            println!("Physical chunks: {}", chunk_count);
            println!("Physical storage: {}", format_size(total_size));
        }

        if files {
            println!("\n📁 File Information");
            println!("──────────────────");
            let working_files = self.scan_working_directory()?;
            let mut stats = FileStats::new();
            for file in working_files.values() {
                stats.add_file(file);
            }
            println!("{}", stats.summary());
        }

        Ok(())
    }

    /// Verify repository integrity
    pub fn verify(&mut self, fix: bool, chunks: bool, verbose: bool) -> Result<usize> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        let mut issues_found = 0;

        // Check database integrity
        if verbose {
            println!("🔍 Checking database integrity...");
        }
        let db_issues = self.database.check_integrity()?;
        issues_found += db_issues.len();

        for issue in &db_issues {
            println!("⚠️  Database issue: {}", issue);
        }

        // Check chunk integrity if requested
        if chunks {
            if verbose {
                println!("🔍 Checking chunk integrity...");
            }
            issues_found += self.verify_chunks(fix, verbose)?;
        }

        // Check file references
        if verbose {
            println!("🔍 Checking file references...");
        }
        issues_found += self.verify_file_references(fix, verbose)?;

        Ok(issues_found)
    }

    /// Optimize repository
    pub fn optimize(&mut self, gc: bool, repack: bool, dry_run: bool) -> Result<String> {
        if !self.is_repo() {
            return Err(BlazeError::Repository("Not a Blaze repository".to_string()));
        }

        let mut operations = Vec::new();

        if gc {
            let active_chunks = self.get_active_chunk_hashes()?;
            let removed = if dry_run {
                0
            } else {
                self.chunk_store.garbage_collect(&active_chunks)?
            };

            operations.push(format!("Garbage collected {} unused chunks", removed));
        }

        if repack {
            operations.push("Repacking not yet implemented".to_string());
        }

        if !dry_run {
            self.database.vacuum()?;
            operations.push("Database vacuumed".to_string());
        }

        Ok(operations.join(", "))
    }

    // Private helper methods

    fn is_repo(&self) -> bool {
        self.blaze_path.exists() && self.blaze_path.join("metadata.db").exists()
    }

    fn acquire_lock(&self) -> Result<File> {
        let lock_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.lock_file)
            .context("Failed to create lock file")?;

        lock_file
            .try_lock_exclusive()
            .context("Repository is locked by another process")?;

        Ok(lock_file)
    }

    fn create_blazeignore(&self) -> Result<()> {
        let ignore_path = self.repo_path.join(".blazeignore");
        if ignore_path.exists() {
            return Ok(());
        }

        let mut file = File::create(&ignore_path).context("Failed to create .blazeignore file")?;

        writeln!(file, "# Blaze ignore patterns")?;
        writeln!(file, ".blaze/")?;
        writeln!(file, "target/")?;
        writeln!(file, "node_modules/")?;
        writeln!(file, "*.tmp")?;
        writeln!(file, "*.log")?;
        writeln!(file, ".DS_Store")?;

        Ok(())
    }

    fn create_config(&self, chunk_size: usize) -> Result<()> {
        let config_path = self.blaze_path.join("config");
        let mut file = File::create(&config_path).context("Failed to create config file")?;

        writeln!(file, "[core]")?;
        writeln!(file, "chunk_size = {}", chunk_size * 1024)?;

        Ok(())
    }

    fn hash_data(&self, data: &[u8]) -> String {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize().to_hex().to_string()
    }

    fn create_tree_hash(&self, files: &HashMap<String, FileRecord>) -> Result<String> {
        let mut tree_data = String::new();
        let mut sorted_files: Vec<_> = files.iter().collect();
        sorted_files.sort_by_key(|(path, _)| *path);

        for (path, record) in sorted_files {
            tree_data.push_str(&format!("{}:{}\n", path, record.chunks.join(",")));
        }

        Ok(self.hash_data(tree_data.as_bytes()))
    }

    fn find_all_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let ignore_patterns = self.load_ignore_patterns()?;

        for entry in WalkDir::new(&self.repo_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !e.path().starts_with(&self.blaze_path))
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                let relative_path = entry.path().strip_prefix(&self.repo_path).unwrap();

                let patterns_refs: Vec<&str> = ignore_patterns.iter().map(|s| s.as_str()).collect();
                if !should_ignore_path(relative_path, &patterns_refs) {
                    files.push(entry.path().to_path_buf());
                }
            }
        }

        Ok(files)
    }

    fn find_modified_files(&self) -> Result<Vec<PathBuf>> {
        let staged_files = self.database.get_all_files()?;
        let mut modified = Vec::new();

        for (path, record) in staged_files {
            let full_path = self.repo_path.join(&path);
            if record.is_different_from_disk(&self.repo_path)? {
                modified.push(full_path);
            }
        }

        Ok(modified)
    }

    fn find_files_matching(&self, pattern: &str) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let ignore_patterns = self.load_ignore_patterns()?;
        let patterns_refs: Vec<&str> = ignore_patterns.iter().map(|s| s.as_str()).collect();
        let pattern_path = self.repo_path.join(pattern);

        if pattern_path.is_file() {
            // Check if single file should be ignored
            let relative_path = pattern_path.strip_prefix(&self.repo_path).unwrap();
            if !should_ignore_path(relative_path, &patterns_refs) {
                files.push(pattern_path);
            }
        } else if pattern_path.is_dir() {
            for entry in WalkDir::new(&pattern_path)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !e.path().starts_with(&self.blaze_path))
            {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let relative_path = entry.path().strip_prefix(&self.repo_path).unwrap();
                    if !should_ignore_path(relative_path, &patterns_refs) {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        } else {
            // Pattern matching
            for entry in WalkDir::new(&self.repo_path)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !e.path().starts_with(&self.blaze_path))
            {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let relative_path = entry.path().strip_prefix(&self.repo_path).unwrap();
                    if relative_path.to_string_lossy().contains(pattern)
                        && !should_ignore_path(relative_path, &patterns_refs)
                    {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        }

        Ok(files)
    }

    fn add_files(&mut self, files: Vec<PathBuf>, verbose: bool, dry_run: bool) -> Result<usize> {
        if files.is_empty() {
            return Ok(0);
        }

        let pb = create_progress_bar(files.len() as u64, "Processing files");
        let mut file_records = Vec::new();

        for file_path in files {
            pb.inc(1);

            if verbose {
                println!("Processing: {}", file_path.display());
            }

            if dry_run {
                continue;
            }

            // Chunk the file
            let chunks = chunk_file(&file_path)?;

            // Store chunks and collect hashes
            let mut chunk_hashes = Vec::new();
            for chunk in &chunks {
                let hash = chunk.hash.clone();
                let _ = self.chunk_store.store_chunk(chunk);
                chunk_hashes.push(hash);
            }

            // Create file record
            let record = FileRecord::from_path(&file_path, &self.repo_path, chunk_hashes)?;
            file_records.push(record);
        }

        pb.finish_with_message("Files processed");

        if !dry_run && !file_records.is_empty() {
            self.database.store_files(&file_records)?;
        }

        Ok(file_records.len())
    }

    fn scan_working_directory(&self) -> Result<HashMap<String, FileRecord>> {
        let mut files = HashMap::new();
        let ignore_patterns = self.load_ignore_patterns()?;

        for entry in WalkDir::new(&self.repo_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !e.path().starts_with(&self.blaze_path))
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                let relative_path = entry.path().strip_prefix(&self.repo_path).unwrap();

                let patterns_refs: Vec<&str> = ignore_patterns.iter().map(|s| s.as_str()).collect();
                if !should_ignore_path(relative_path, &patterns_refs) {
                    // Create a basic file record for comparison
                    let chunks = chunk_file(entry.path())?;
                    let chunk_hashes: Vec<String> = chunks.iter().map(|c| c.hash.clone()).collect();

                    if let Ok(record) =
                        FileRecord::from_path(entry.path(), &self.repo_path, chunk_hashes)
                    {
                        files.insert(record.path.clone(), record);
                    }
                }
            }
        }

        Ok(files)
    }

    fn load_ignore_patterns(&self) -> Result<Vec<String>> {
        let mut patterns: Vec<String> = DEFAULT_IGNORE_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect();

        let ignore_file = self.repo_path.join(".blazeignore");
        if ignore_file.exists() {
            let file = File::open(&ignore_file)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    patterns.push(line.to_string());
                }
            }
        }

        Ok(patterns)
    }

    fn get_head_commit(&self) -> Result<Option<String>> {
        if let Some(head_ref) = self.database.get_ref("HEAD")? {
            Ok(head_ref.commit_hash)
        } else {
            Ok(None)
        }
    }

    fn has_uncommitted_changes(&self) -> Result<bool> {
        let staged = self.database.get_all_files()?;
        let working = self.scan_working_directory()?;

        Ok(staged != working)
    }

    fn restore_files(&mut self, files: &HashMap<String, FileRecord>) -> Result<()> {
        for record in files.values() {
            let file_path = self.repo_path.join(&record.path);

            // Create parent directories
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Load chunks and reconstruct file
            let chunk_data: Result<Vec<_>> = record
                .chunks
                .iter()
                .map(|hash| self.chunk_store.load_chunk(hash))
                .collect();

            let chunks_data = chunk_data?;
            let file_data: Vec<u8> = chunks_data.into_iter().flatten().collect();

            std::fs::write(&file_path, &file_data)?;

            // Restore permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&file_path)?.permissions();
                perms.set_mode(record.permissions);
                std::fs::set_permissions(&file_path, perms)?;
            }
        }

        Ok(())
    }

    fn print_short_status(&self, staged: &[FileChange], working: &[FileChange]) -> Result<()> {
        for change in staged {
            print!("A  ");
            println!("{}", change.path);
        }

        for change in working {
            print!(" M ");
            println!("{}", change.path);
        }

        Ok(())
    }

    fn print_long_status(
        &self,
        staged: &[FileChange],
        working: &[FileChange],
        _ignored: bool,
        _untracked: UntrackedFiles,
    ) -> Result<()> {
        if !staged.is_empty() {
            println!("Changes to be committed:");
            for change in staged {
                println!("  {}", change.description());
            }
            println!();
        }

        if !working.is_empty() {
            println!("Changes not staged for commit:");
            for change in working {
                println!("  {}", change.description());
            }
            println!();
        }

        if staged.is_empty() && working.is_empty() {
            println!("nothing to commit, working tree clean");
        }

        Ok(())
    }

    fn verify_chunks(&mut self, fix: bool, verbose: bool) -> Result<usize> {
        let chunk_hashes = self.database.get_all_chunk_hashes()?;
        let mut issues = 0;

        for hash in chunk_hashes {
            if !self.chunk_store.chunk_exists(&hash) {
                if verbose {
                    println!("⚠️  Missing chunk: {}", hash);
                }
                issues += 1;

                if fix {
                    self.database.delete_chunks(&[hash])?;
                    if verbose {
                        println!("🔧 Removed reference to missing chunk");
                    }
                }
            }
        }

        Ok(issues)
    }

    fn verify_file_references(&self, _fix: bool, verbose: bool) -> Result<usize> {
        let files = self.database.get_all_files()?;
        let mut issues = 0;

        for (path, record) in files {
            for chunk_hash in &record.chunks {
                if !self.chunk_store.chunk_exists(chunk_hash) {
                    if verbose {
                        println!("⚠️  File {} references missing chunk {}", path, chunk_hash);
                    }
                    issues += 1;
                }
            }
        }

        Ok(issues)
    }

    fn get_active_chunk_hashes(&self) -> Result<Vec<String>> {
        let files = self.database.get_all_files()?;
        let commits = self.database.get_commits(None, None)?;

        let mut active_hashes = HashSet::new();

        // Collect from staged files
        for (_, record) in files {
            active_hashes.extend(record.chunks);
        }

        // Collect from commits
        for commit in commits {
            for (_, record) in commit.files {
                active_hashes.extend(record.chunks);
            }
        }

        Ok(active_hashes.into_iter().collect())
    }
}
