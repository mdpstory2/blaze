#!/usr/bin/env cargo script

//! FastVC Benchmark Example
//!
//! This example demonstrates FastVC performance by creating various types of files
//! and measuring operations like add, commit, and checkout.
//!
//! Run with: cargo run --example benchmark

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::TempDir;

// Simple benchmark structure
struct Benchmark {
    name: String,
    setup: Box<dyn Fn(&Path) -> std::io::Result<()>>,
    operation: Box<dyn Fn(&Path) -> std::io::Result<Duration>>,
}

impl Benchmark {
    fn new(
        name: &str,
        setup: impl Fn(&Path) -> std::io::Result<()> + 'static,
        operation: impl Fn(&Path) -> std::io::Result<Duration> + 'static,
    ) -> Self {
        Self {
            name: name.to_string(),
            setup: Box::new(setup),
            operation: Box::new(operation),
        }
    }

    fn run(&self, temp_dir: &Path) -> std::io::Result<Duration> {
        println!("Setting up benchmark: {}", self.name);
        (self.setup)(temp_dir)?;

        println!("Running benchmark: {}", self.name);
        let duration = (self.operation)(temp_dir)?;

        println!("Completed: {} in {:?}", self.name, duration);
        Ok(duration)
    }
}

fn create_test_files(dir: &Path, count: usize, size: usize, pattern: &str) -> std::io::Result<()> {
    for i in 0..count {
        let file_path = dir.join(format!("{}_{:04}.txt", pattern, i));
        let mut file = File::create(&file_path)?;

        // Create content with some variation to prevent perfect deduplication
        let content = format!(
            "File {} of {}\n{}\n{}",
            i + 1,
            count,
            "=".repeat(size / 4),
            "x".repeat(size - size / 4 - 20)
        );
        file.write_all(content.as_bytes())?;
    }
    Ok(())
}

fn create_duplicate_files(dir: &Path, count: usize, size: usize) -> std::io::Result<()> {
    let content = "duplicate content ".repeat(size / 18);

    for i in 0..count {
        let file_path = dir.join(format!("dup_{:04}.txt", i));
        let mut file = File::create(&file_path)?;
        file.write_all(content.as_bytes())?;
    }
    Ok(())
}

fn create_large_file(dir: &Path, size: usize) -> std::io::Result<()> {
    let file_path = dir.join("large_file.dat");
    let mut file = File::create(&file_path)?;

    // Create a large file with repeating pattern
    let pattern = b"FastVC is a chunk-based version control system. ";
    let pattern_len = pattern.len();
    let mut written = 0;

    while written < size {
        let to_write = std::cmp::min(pattern_len, size - written);
        file.write_all(&pattern[..to_write])?;
        written += to_write;
    }

    Ok(())
}

fn run_fastvc_command(dir: &Path, args: &[&str]) -> std::io::Result<Duration> {
    let start = Instant::now();

    let output = std::process::Command::new("../target/release/fastvc")
        .current_dir(dir)
        .args(args)
        .output()?;

    let duration = start.elapsed();

    if !output.status.success() {
        eprintln!("Command failed: fastvc {:?}", args);
        eprintln!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "FastVC command failed",
        ));
    }

    Ok(duration)
}

fn benchmark_init() -> Benchmark {
    Benchmark::new(
        "Repository Initialization",
        |_dir| Ok(()),
        |dir| run_fastvc_command(dir, &["init"]),
    )
}

fn benchmark_small_files() -> Benchmark {
    Benchmark::new(
        "Add 100 Small Files (1KB each)",
        |dir| create_test_files(dir, 100, 1024, "small"),
        |dir| {
            run_fastvc_command(dir, &["init"])?;
            run_fastvc_command(dir, &["add", "."])
        },
    )
}

fn benchmark_medium_files() -> Benchmark {
    Benchmark::new(
        "Add 50 Medium Files (100KB each)",
        |dir| create_test_files(dir, 50, 100 * 1024, "medium"),
        |dir| {
            run_fastvc_command(dir, &["init"])?;
            run_fastvc_command(dir, &["add", "."])
        },
    )
}

fn benchmark_large_file() -> Benchmark {
    Benchmark::new(
        "Add 1 Large File (10MB)",
        |dir| create_large_file(dir, 10 * 1024 * 1024),
        |dir| {
            run_fastvc_command(dir, &["init"])?;
            run_fastvc_command(dir, &["add", "."])
        },
    )
}

fn benchmark_duplicates() -> Benchmark {
    Benchmark::new(
        "Add 200 Duplicate Files (deduplication test)",
        |dir| create_duplicate_files(dir, 200, 5 * 1024),
        |dir| {
            run_fastvc_command(dir, &["init"])?;
            run_fastvc_command(dir, &["add", "."])
        },
    )
}

fn benchmark_commit() -> Benchmark {
    Benchmark::new(
        "Commit 100 Files",
        |dir| {
            create_test_files(dir, 100, 2048, "commit_test")?;
            run_fastvc_command(dir, &["init"])?;
            run_fastvc_command(dir, &["add", "."])?;
            Ok(())
        },
        |dir| run_fastvc_command(dir, &["commit", "-m", "Benchmark commit"]),
    )
}

fn benchmark_checkout() -> Benchmark {
    Benchmark::new(
        "Checkout Files",
        |dir| {
            create_test_files(dir, 50, 4096, "checkout_test")?;
            run_fastvc_command(dir, &["init"])?;
            run_fastvc_command(dir, &["add", "."])?;
            run_fastvc_command(dir, &["commit", "-m", "Initial commit"])?;

            // Modify files to test checkout restoration
            create_test_files(dir, 50, 2048, "modified")?;
            Ok(())
        },
        |dir| {
            // Get the commit hash from log and checkout
            let log_output = std::process::Command::new("../target/release/fastvc")
                .current_dir(dir)
                .args(&["log", "--limit", "1"])
                .output()
                .expect("Failed to get log");

            let log_str = String::from_utf8_lossy(&log_output.stdout);
            if let Some(line) = log_str.lines().find(|l| l.starts_with("Commit: ")) {
                let hash = line.replace("Commit: ", "");
                return run_fastvc_command(dir, &["checkout", &hash]);
            }

            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Could not extract commit hash",
            ))
        },
    )
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
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

fn calculate_dir_size(dir: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            total += metadata.len();
        } else if metadata.is_dir() {
            total += calculate_dir_size(&entry.path())?;
        }
    }
    Ok(total)
}

fn main() -> std::io::Result<()> {
    println!("FastVC Performance Benchmark");
    println!("============================");

    // Check if fastvc binary exists
    if !Path::new("../target/release/fastvc").exists() {
        println!("Building FastVC in release mode...");
        let build_result = std::process::Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir("..")
            .status()?;

        if !build_result.success() {
            eprintln!("Failed to build FastVC");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Build failed",
            ));
        }
    }

    let benchmarks = vec![
        benchmark_init(),
        benchmark_small_files(),
        benchmark_medium_files(),
        benchmark_large_file(),
        benchmark_duplicates(),
        benchmark_commit(),
        benchmark_checkout(),
    ];

    let mut results = Vec::new();

    for benchmark in benchmarks {
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path();

        match benchmark.run(temp_path) {
            Ok(duration) => {
                results.push((benchmark.name.clone(), duration));

                // Show repository stats if .fastvc exists
                if temp_path.join(".fastvc").exists() {
                    if let Ok(repo_size) = calculate_dir_size(&temp_path.join(".fastvc")) {
                        println!("  Repository size: {}", format_size(repo_size));
                    }
                }

                println!();
            }
            Err(e) => {
                eprintln!("Benchmark '{}' failed: {}", benchmark.name, e);
                println!();
            }
        }
    }

    // Print summary
    println!("Benchmark Results Summary");
    println!("========================");

    let mut total_time = Duration::new(0, 0);
    for (name, duration) in &results {
        println!("{:<35} {:>10.3}s", name, duration.as_secs_f64());
        total_time += *duration;
    }

    println!("{:<35} {:>10.3}s", "TOTAL", total_time.as_secs_f64());

    // Performance metrics
    println!("\nPerformance Analysis");
    println!("===================");

    if let Some((_, init_time)) = results
        .iter()
        .find(|(name, _)| name.contains("Initialization"))
    {
        println!("Repository init overhead: {:.3}s", init_time.as_secs_f64());
    }

    if let Some((_, small_files_time)) = results
        .iter()
        .find(|(name, _)| name.contains("Small Files"))
    {
        let files_per_sec = 100.0 / small_files_time.as_secs_f64();
        println!("Small files throughput: {:.1} files/sec", files_per_sec);
    }

    if let Some((_, large_file_time)) = results.iter().find(|(name, _)| name.contains("Large File"))
    {
        let mb_per_sec = 10.0 / large_file_time.as_secs_f64();
        println!("Large file throughput: {:.1} MB/sec", mb_per_sec);
    }

    println!("\nBenchmark completed successfully!");
    Ok(())
}
