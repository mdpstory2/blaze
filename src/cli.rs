//! Command-line interface for Blaze VCS

use crate::core::Blaze;
use crate::errors::{BlazeError, Result};
use clap::{Parser, Subcommand};

/// Blaze - A blazingly fast, chunk-based version control system
#[derive(Parser)]
#[command(name = "blaze")]
#[command(about = "A blazingly fast, chunk-based version control system")]
#[command(
    long_about = "Blaze is a next-generation version control system designed to be faster and easier than Git.
It uses advanced chunking algorithms and parallel processing to provide lightning-fast operations
while maintaining data integrity and ease of use."
)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available Blaze commands
#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Blaze repository
    #[command(alias = "i")]
    Init {
        /// Directory to initialize (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
        /// Don't create .blazeignore file
        #[arg(long)]
        no_ignore: bool,
        /// Set custom chunk size in KB (default: 64)
        #[arg(long, value_name = "SIZE")]
        chunk_size: Option<usize>,
    },

    /// Add files to the staging area
    #[command(alias = "a")]
    Add {
        /// Files or patterns to add
        files: Vec<String>,
        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
        /// Add all files (including ignored ones)
        #[arg(long)]
        all: bool,
        /// Dry run - show what would be added without actually adding
        #[arg(long)]
        dry_run: bool,
    },

    /// Create a new commit with staged changes
    #[command(alias = "c")]
    Commit {
        /// Commit message
        #[arg(short, long)]
        message: String,
        /// Automatically stage all modified files
        #[arg(short = 'a', long)]
        all: bool,
        /// Show files being committed
        #[arg(short, long)]
        verbose: bool,
        /// Allow empty commits
        #[arg(long)]
        allow_empty: bool,
    },

    /// Show commit history
    #[command(alias = "l")]
    Log {
        /// Maximum number of commits to show
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
        /// Show commits in one line format
        #[arg(long)]
        oneline: bool,
        /// Show detailed statistics for each commit
        #[arg(long)]
        stat: bool,
        /// Show commits since a specific commit
        #[arg(long)]
        since: Option<String>,
    },

    /// Show working tree status
    #[command(alias = "s")]
    Status {
        /// Show short format output
        #[arg(short, long)]
        short: bool,
        /// Show ignored files
        #[arg(long)]
        ignored: bool,
        /// Show untracked files
        #[arg(short, long, default_value = "normal")]
        untracked_files: UntrackedFiles,
    },

    /// Checkout a specific commit or restore files
    #[command(alias = "co")]
    Checkout {
        /// Commit hash, branch name, or file path
        target: String,
        /// Force checkout even if working directory is dirty
        #[arg(short, long)]
        force: bool,
        /// Create a new branch
        #[arg(short = 'b', long)]
        new_branch: Option<String>,
    },

    /// List, create, or delete branches
    #[command(alias = "br")]
    Branch {
        /// Branch name to create or delete
        name: Option<String>,
        /// Delete the specified branch
        #[arg(short = 'd', long)]
        delete: bool,
        /// Force delete even if not merged
        #[arg(short = 'D', long)]
        force_delete: bool,
        /// Show all branches
        #[arg(short = 'a', long)]
        all: bool,
    },

    /// Show repository statistics and health information
    #[command(alias = "info")]
    Stats {
        /// Show detailed chunk statistics
        #[arg(long)]
        chunks: bool,
        /// Show file type breakdown
        #[arg(long)]
        files: bool,
        /// Show storage efficiency metrics
        #[arg(long)]
        storage: bool,
    },

    /// Verify repository integrity and fix issues
    #[command(alias = "check")]
    Verify {
        /// Attempt to fix found issues
        #[arg(long)]
        fix: bool,
        /// Check chunk integrity
        #[arg(long)]
        chunks: bool,
        /// Verbose output showing all checks
        #[arg(short, long)]
        verbose: bool,
    },

    /// Optimize repository storage and performance
    #[command(alias = "opt")]
    Optimize {
        /// Perform garbage collection on unused chunks
        #[arg(long)]
        gc: bool,
        /// Repack chunks for better compression
        #[arg(long)]
        repack: bool,
        /// Show what would be optimized without doing it
        #[arg(long)]
        dry_run: bool,
    },
}

/// Options for showing untracked files
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum UntrackedFiles {
    /// Hide untracked files
    No,
    /// Show untracked files (default)
    Normal,
    /// Show all untracked files including those in ignored directories
    All,
}

/// Main entry point for the CLI application
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    // Initialize the Blaze instance
    let mut blaze = Blaze::new(".")?;

    // Execute the requested command
    match cli.command {
        Commands::Init {
            path,
            no_ignore,
            chunk_size,
        } => {
            println!("🔥 Initializing Blaze repository in '{}'", path);
            let mut blaze = Blaze::new(&path)?;
            blaze.init(no_ignore, chunk_size)?;
            println!("✅ Blaze repository initialized successfully!");
        }

        Commands::Add {
            files,
            verbose,
            all,
            dry_run,
        } => {
            if dry_run {
                println!("🔍 Dry run - showing files that would be added:");
            }
            let added_files = blaze.add(files, verbose, all, dry_run)?;

            if !dry_run {
                println!(
                    "✅ Added {} file{}",
                    added_files,
                    if added_files == 1 { "" } else { "s" }
                );
            }
        }

        Commands::Commit {
            message,
            all,
            verbose,
            allow_empty,
        } => {
            println!("📝 Creating commit...");
            let commit_hash = blaze.commit(message, all, verbose, allow_empty)?;
            println!("✅ Created commit: {}", commit_hash);
        }

        Commands::Log {
            limit,
            oneline,
            stat,
            since,
        } => {
            blaze.log(limit, oneline, stat, since)?;
        }

        Commands::Status {
            short,
            ignored,
            untracked_files,
        } => {
            blaze.status(short, ignored, untracked_files)?;
        }

        Commands::Checkout {
            target,
            force,
            new_branch,
        } => {
            if let Some(branch_name) = new_branch {
                println!("🌿 Creating new branch '{}'", branch_name);
                blaze.create_branch(&branch_name)?;
            }

            println!("📂 Checking out '{}'", target);
            blaze.checkout(&target, force)?;
            println!("✅ Checkout complete");
        }

        Commands::Branch {
            name,
            delete,
            force_delete,
            all,
        } => {
            if let Some(branch_name) = name {
                if delete || force_delete {
                    blaze.delete_branch(&branch_name, force_delete)?;
                    println!("🗑️  Deleted branch '{}'", branch_name);
                } else {
                    blaze.create_branch(&branch_name)?;
                    println!("🌿 Created branch '{}'", branch_name);
                }
            } else {
                blaze.list_branches(all)?;
            }
        }

        Commands::Stats {
            chunks,
            files,
            storage,
        } => {
            blaze.show_stats(chunks, files, storage)?;
        }

        Commands::Verify {
            fix,
            chunks,
            verbose,
        } => {
            println!("🔍 Verifying repository integrity...");
            let issues = blaze.verify(fix, chunks, verbose)?;

            if issues == 0 {
                println!("✅ Repository integrity verified - no issues found");
            } else {
                println!(
                    "⚠️  Found {} issue{}",
                    issues,
                    if issues == 1 { "" } else { "s" }
                );
                if fix {
                    println!("🔧 Issues have been fixed");
                } else {
                    println!("💡 Run with --fix to attempt automatic repairs");
                }
            }
        }

        Commands::Optimize {
            gc,
            repack,
            dry_run,
        } => {
            if dry_run {
                println!("🔍 Dry run - showing optimization opportunities:");
            }

            let stats = blaze.optimize(gc, repack, dry_run)?;

            if !dry_run {
                println!("✅ Optimization complete: {}", stats);
            }
        }
    }

    Ok(())
}

/// Display help information for a specific command
pub fn show_command_help(command: &str) -> Result<()> {
    match command {
        "init" => {
            println!("blaze init - Initialize a new Blaze repository");
            println!();
            println!("USAGE:");
            println!("    blaze init [OPTIONS] [PATH]");
            println!();
            println!("OPTIONS:");
            println!("    --no-ignore         Don't create .blazeignore file");
            println!("    --chunk-size SIZE   Set custom chunk size in KB");
            println!();
            println!("EXAMPLES:");
            println!("    blaze init                    # Initialize in current directory");
            println!("    blaze init /path/to/repo      # Initialize in specific directory");
            println!("    blaze init --chunk-size 128   # Use 128KB chunks");
        }
        "add" => {
            println!("blaze add - Add files to the staging area");
            println!();
            println!("USAGE:");
            println!("    blaze add [OPTIONS] <FILES>...");
            println!();
            println!("OPTIONS:");
            println!("    -v, --verbose    Show verbose output");
            println!("    --all           Add all files including ignored ones");
            println!("    --dry-run       Show what would be added");
            println!();
            println!("EXAMPLES:");
            println!("    blaze add file.txt            # Add single file");
            println!("    blaze add src/                # Add directory");
            println!("    blaze add *.rs               # Add pattern");
            println!("    blaze add --all              # Add everything");
        }
        _ => {
            return Err(BlazeError::Validation(format!(
                "No help available for command: {}",
                command
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test basic init command
        let cli = Cli::try_parse_from(["blaze", "init"]).unwrap();
        match cli.command {
            Commands::Init {
                path,
                no_ignore,
                chunk_size,
            } => {
                assert_eq!(path, ".");
                assert!(!no_ignore);
                assert_eq!(chunk_size, None);
            }
            _ => panic!("Expected Init command"),
        }

        // Test add command with flags
        let cli = Cli::try_parse_from(["blaze", "add", "--verbose", "file.txt"]).unwrap();
        match cli.command {
            Commands::Add {
                files,
                verbose,
                all,
                dry_run,
            } => {
                assert_eq!(files, vec!["file.txt"]);
                assert!(verbose);
                assert!(!all);
                assert!(!dry_run);
            }
            _ => panic!("Expected Add command"),
        }

        // Test commit with message
        let cli = Cli::try_parse_from(["blaze", "commit", "-m", "Test commit"]).unwrap();
        match cli.command {
            Commands::Commit {
                message,
                all,
                verbose,
                allow_empty,
            } => {
                assert_eq!(message, "Test commit");
                assert!(!all);
                assert!(!verbose);
                assert!(!allow_empty);
            }
            _ => panic!("Expected Commit command"),
        }
    }

    #[test]
    fn test_untracked_files_enum() {
        use clap::ValueEnum;

        let values = UntrackedFiles::value_variants();
        assert_eq!(values.len(), 3);

        let normal = UntrackedFiles::from_str("normal", true).unwrap();
        matches!(normal, UntrackedFiles::Normal);
    }
}
