#!/usr/bin/env cargo script

//! FastVC vs Git Performance Comparison
//!
//! This benchmark compares FastVC against Git for various operations
//! to provide objective performance analysis.
//!
//! Run with: cargo run --example git_comparison

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::TempDir;

struct BenchmarkResult {
    operation: String,
    fastvc_time: Duration,
    git_time: Duration,
    fastvc_size: u64,
    git_size: u64,
}

impl BenchmarkResult {
    fn speedup(&self) -> f64 {
        self.git_time.as_secs_f64() / self.fastvc_time.as_secs_f64()
    }

    fn size_ratio(&self) -> f64 {
        self.fastvc_size as f64 / self.git_size as f64
    }
}

fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else {
        format!("{:.3}s", secs)
    }
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

fn calculate_dir_size(path: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    if !path.exists() {
        return Ok(0);
    }

    for entry in fs::read_dir(path)? {
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

struct TestRepo {
    temp_dir: TempDir,
    path: PathBuf,
    fastvc_binary: PathBuf,
}

impl TestRepo {
    fn new() -> std::io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().to_path_buf();
        let current_dir = std::env::current_dir()?;
        let fastvc_binary = current_dir.join("target/release/fastvc");

        Ok(Self {
            temp_dir,
            path,
            fastvc_binary,
        })
    }

    fn create_file(&self, relative_path: &str, content: &[u8]) -> std::io::Result<()> {
        let file_path = self.path.join(relative_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(file_path, content)?;
        Ok(())
    }

    fn run_fastvc(&self, args: &[&str]) -> std::io::Result<Duration> {
        let start = Instant::now();
        let output = Command::new(&self.fastvc_binary)
            .current_dir(&self.path)
            .args(args)
            .output()?;

        let duration = start.elapsed();

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("FastVC command failed: {:?}", args),
            ));
        }

        Ok(duration)
    }

    fn run_git(&self, args: &[&str]) -> std::io::Result<Duration> {
        let start = Instant::now();
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(args)
            .output()?;

        let duration = start.elapsed();

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Git command failed: {:?}", args),
            ));
        }

        Ok(duration)
    }

    fn get_fastvc_size(&self) -> std::io::Result<u64> {
        calculate_dir_size(&self.path.join(".fastvc"))
    }

    fn get_git_size(&self) -> std::io::Result<u64> {
        calculate_dir_size(&self.path.join(".git"))
    }
}

fn benchmark_init() -> std::io::Result<BenchmarkResult> {
    println!("🔹 Benchmarking: Repository Initialization");

    let fastvc_repo = TestRepo::new()?;
    let git_repo = TestRepo::new()?;

    // Benchmark FastVC init
    let fastvc_time = fastvc_repo.run_fastvc(&["init"])?;
    let fastvc_size = fastvc_repo.get_fastvc_size()?;

    // Benchmark Git init
    let git_time = git_repo.run_git(&["init"])?;
    let git_size = git_repo.get_git_size()?;

    Ok(BenchmarkResult {
        operation: "Repository Init".to_string(),
        fastvc_time,
        git_time,
        fastvc_size,
        git_size,
    })
}

fn benchmark_small_files() -> std::io::Result<BenchmarkResult> {
    println!("🔹 Benchmarking: 100 Small Files (1KB each)");

    let fastvc_repo = TestRepo::new()?;
    let git_repo = TestRepo::new()?;

    // Create identical test files
    let content = "Small file content.\n".repeat(50); // ~1KB
    for i in 0..100 {
        let filename = format!("small_{:03}.txt", i);
        fastvc_repo.create_file(&filename, content.as_bytes())?;
        git_repo.create_file(&filename, content.as_bytes())?;
    }

    // Initialize repositories
    fastvc_repo.run_fastvc(&["init"])?;
    git_repo.run_git(&["init"])?;
    git_repo.run_git(&["config", "user.email", "test@example.com"])?;
    git_repo.run_git(&["config", "user.name", "Test User"])?;

    // Benchmark FastVC
    let fastvc_add_time = fastvc_repo.run_fastvc(&["add", "."])?;
    let fastvc_commit_time = fastvc_repo.run_fastvc(&["commit", "-m", "Add small files"])?;
    let fastvc_total_time = fastvc_add_time + fastvc_commit_time;
    let fastvc_size = fastvc_repo.get_fastvc_size()?;

    // Benchmark Git
    let git_add_time = git_repo.run_git(&["add", "."])?;
    let git_commit_time = git_repo.run_git(&["commit", "-m", "Add small files"])?;
    let git_total_time = git_add_time + git_commit_time;
    let git_size = git_repo.get_git_size()?;

    Ok(BenchmarkResult {
        operation: "100 Small Files".to_string(),
        fastvc_time: fastvc_total_time,
        git_time: git_total_time,
        fastvc_size,
        git_size,
    })
}

fn benchmark_large_file() -> std::io::Result<BenchmarkResult> {
    println!("🔹 Benchmarking: Single Large File (10MB)");

    let fastvc_repo = TestRepo::new()?;
    let git_repo = TestRepo::new()?;

    // Create a 10MB file
    let content = vec![b'X'; 10 * 1024 * 1024]; // 10MB
    fastvc_repo.create_file("large_file.dat", &content)?;
    git_repo.create_file("large_file.dat", &content)?;

    // Initialize repositories
    fastvc_repo.run_fastvc(&["init"])?;
    git_repo.run_git(&["init"])?;
    git_repo.run_git(&["config", "user.email", "test@example.com"])?;
    git_repo.run_git(&["config", "user.name", "Test User"])?;

    // Benchmark FastVC
    let fastvc_add_time = fastvc_repo.run_fastvc(&["add", "."])?;
    let fastvc_commit_time = fastvc_repo.run_fastvc(&["commit", "-m", "Add large file"])?;
    let fastvc_total_time = fastvc_add_time + fastvc_commit_time;
    let fastvc_size = fastvc_repo.get_fastvc_size()?;

    // Benchmark Git
    let git_add_time = git_repo.run_git(&["add", "."])?;
    let git_commit_time = git_repo.run_git(&["commit", "-m", "Add large file"])?;
    let git_total_time = git_add_time + git_commit_time;
    let git_size = git_repo.get_git_size()?;

    Ok(BenchmarkResult {
        operation: "10MB Large File".to_string(),
        fastvc_time: fastvc_total_time,
        git_time: git_total_time,
        fastvc_size,
        git_size,
    })
}

fn benchmark_duplicates() -> std::io::Result<BenchmarkResult> {
    println!("🔹 Benchmarking: Duplicate Content (Deduplication Test)");

    let fastvc_repo = TestRepo::new()?;
    let git_repo = TestRepo::new()?;

    // Create 50 files with identical content (should deduplicate well)
    let content = "This is duplicate content that should compress well.\n".repeat(1000); // ~50KB per file
    for i in 0..50 {
        let filename = format!("dup_{:03}.txt", i);
        fastvc_repo.create_file(&filename, content.as_bytes())?;
        git_repo.create_file(&filename, content.as_bytes())?;
    }

    // Initialize repositories
    fastvc_repo.run_fastvc(&["init"])?;
    git_repo.run_git(&["init"])?;
    git_repo.run_git(&["config", "user.email", "test@example.com"])?;
    git_repo.run_git(&["config", "user.name", "Test User"])?;

    // Benchmark FastVC
    let fastvc_add_time = fastvc_repo.run_fastvc(&["add", "."])?;
    let fastvc_commit_time = fastvc_repo.run_fastvc(&["commit", "-m", "Add duplicate files"])?;
    let fastvc_total_time = fastvc_add_time + fastvc_commit_time;
    let fastvc_size = fastvc_repo.get_fastvc_size()?;

    // Benchmark Git
    let git_add_time = git_repo.run_git(&["add", "."])?;
    let git_commit_time = git_repo.run_git(&["commit", "-m", "Add duplicate files"])?;
    let git_total_time = git_add_time + git_commit_time;
    let git_size = git_repo.get_git_size()?;

    Ok(BenchmarkResult {
        operation: "50 Duplicate Files".to_string(),
        fastvc_time: fastvc_total_time,
        git_time: git_total_time,
        fastvc_size,
        git_size,
    })
}

fn benchmark_mixed_files() -> std::io::Result<BenchmarkResult> {
    println!("🔹 Benchmarking: Mixed File Types (Realistic Repository)");

    let fastvc_repo = TestRepo::new()?;
    let git_repo = TestRepo::new()?;

    // Create a realistic mix of files
    // Source code files
    for i in 0..20 {
        let content = format!(
            "// Source file {}\nfn main() {{\n    println!(\"Hello from file {}\");\n}}\n",
            i, i
        );
        let filename = format!("src/file_{:02}.rs", i);
        fastvc_repo.create_file(&filename, content.as_bytes())?;
        git_repo.create_file(&filename, content.as_bytes())?;
    }

    // Configuration files
    let config_content = "{\n  \"name\": \"test-project\",\n  \"version\": \"1.0.0\"\n}\n";
    fastvc_repo.create_file("package.json", config_content.as_bytes())?;
    git_repo.create_file("package.json", config_content.as_bytes())?;

    // README
    let readme_content = "# Test Project\n\nThis is a test project for benchmarking.\n\n## Installation\n\nRun the build script.\n";
    fastvc_repo.create_file("README.md", readme_content.as_bytes())?;
    git_repo.create_file("README.md", readme_content.as_bytes())?;

    // Binary-like files
    for i in 0..5 {
        let binary_content = vec![i as u8; 10240]; // 10KB of binary data
        let filename = format!("assets/image_{}.dat", i);
        fastvc_repo.create_file(&filename, &binary_content)?;
        git_repo.create_file(&filename, &binary_content)?;
    }

    // Initialize repositories
    fastvc_repo.run_fastvc(&["init"])?;
    git_repo.run_git(&["init"])?;
    git_repo.run_git(&["config", "user.email", "test@example.com"])?;
    git_repo.run_git(&["config", "user.name", "Test User"])?;

    // Benchmark FastVC
    let fastvc_add_time = fastvc_repo.run_fastvc(&["add", "."])?;
    let fastvc_commit_time = fastvc_repo.run_fastvc(&["commit", "-m", "Initial project setup"])?;
    let fastvc_total_time = fastvc_add_time + fastvc_commit_time;
    let fastvc_size = fastvc_repo.get_fastvc_size()?;

    // Benchmark Git
    let git_add_time = git_repo.run_git(&["add", "."])?;
    let git_commit_time = git_repo.run_git(&["commit", "-m", "Initial project setup"])?;
    let git_total_time = git_add_time + git_commit_time;
    let git_size = git_repo.get_git_size()?;

    Ok(BenchmarkResult {
        operation: "Mixed File Types".to_string(),
        fastvc_time: fastvc_total_time,
        git_time: git_total_time,
        fastvc_size,
        git_size,
    })
}

fn print_results_table(results: &[BenchmarkResult]) {
    println!("\n📊 Performance Comparison Results");
    println!("═════════════════════════════════════════════════════════════════════════════");
    println!(
        "{:<20} {:>12} {:>12} {:>8} {:>12} {:>12} {:>8}",
        "Operation", "FastVC", "Git", "Speedup", "FastVC Size", "Git Size", "Size Ratio"
    );
    println!("─────────────────────────────────────────────────────────────────────────────");

    for result in results {
        let speedup = if result.speedup() > 1.0 {
            format!("{:.1}x faster", result.speedup())
        } else {
            format!("{:.1}x slower", 1.0 / result.speedup())
        };

        let size_ratio = if result.size_ratio() < 1.0 {
            format!("{:.1}x smaller", 1.0 / result.size_ratio())
        } else {
            format!("{:.1}x larger", result.size_ratio())
        };

        println!(
            "{:<20} {:>12} {:>12} {:>8} {:>12} {:>12} {:>8}",
            result.operation,
            format_duration(result.fastvc_time),
            format_duration(result.git_time),
            speedup,
            format_size(result.fastvc_size),
            format_size(result.git_size),
            size_ratio
        );
    }
    println!("═════════════════════════════════════════════════════════════════════════════");
}

fn print_analysis(results: &[BenchmarkResult]) {
    println!("\n🔍 Analysis");
    println!("══════════");

    let mut faster_count = 0;
    let mut smaller_count = 0;

    for result in results {
        if result.speedup() > 1.0 {
            faster_count += 1;
        }
        if result.size_ratio() < 1.0 {
            smaller_count += 1;
        }
    }

    println!("FastVC vs Git Performance:");
    println!(
        "• FastVC is faster in {}/{} test cases",
        faster_count,
        results.len()
    );
    println!(
        "• FastVC uses less storage in {}/{} test cases",
        smaller_count,
        results.len()
    );

    println!("\n🎯 Key Observations:");

    // Find best and worst cases
    let best_speed = results
        .iter()
        .max_by(|a, b| a.speedup().partial_cmp(&b.speedup()).unwrap());
    let worst_speed = results
        .iter()
        .min_by(|a, b| a.speedup().partial_cmp(&b.speedup()).unwrap());
    let best_storage = results
        .iter()
        .min_by(|a, b| a.size_ratio().partial_cmp(&b.size_ratio()).unwrap());

    if let Some(best) = best_speed {
        if best.speedup() > 1.0 {
            println!(
                "• Best FastVC performance: {} ({:.1}x faster than Git)",
                best.operation,
                best.speedup()
            );
        }
    }

    if let Some(worst) = worst_speed {
        if worst.speedup() < 1.0 {
            println!(
                "• Worst FastVC performance: {} ({:.1}x slower than Git)",
                worst.operation,
                1.0 / worst.speedup()
            );
        }
    }

    if let Some(best) = best_storage {
        if best.size_ratio() < 1.0 {
            println!(
                "• Best storage efficiency: {} ({:.1}x smaller than Git)",
                best.operation,
                1.0 / best.size_ratio()
            );
        }
    }

    println!("\n📝 Technical Notes:");
    println!("• Git has 15+ years of optimization and is written in C");
    println!("• FastVC is a proof-of-concept implementation in Rust");
    println!("• Git uses sophisticated delta compression and pack files");
    println!("• FastVC uses chunk-based deduplication with BLAKE3 hashing");
    println!("• For large files, FastVC's chunking approach may have advantages");
    println!("• For typical source code, Git's optimizations are hard to beat");
}

fn main() -> std::io::Result<()> {
    println!("FastVC vs Git Performance Comparison");
    println!("====================================");

    // Check prerequisites
    let current_dir = std::env::current_dir()?;
    let fastvc_binary = current_dir.join("target/release/fastvc");

    if !fastvc_binary.exists() {
        println!("🔨 Building FastVC in release mode...");
        let build_result = Command::new("cargo")
            .args(&["build", "--release"])
            .status()?;

        if !build_result.success() {
            eprintln!("❌ Failed to build FastVC");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Build failed",
            ));
        }
    }

    // Check if git is available
    if Command::new("git").arg("--version").output().is_err() {
        eprintln!("❌ Git is not available. Please install Git to run this comparison.");
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Git not found",
        ));
    }

    println!("✅ Prerequisites met. Running benchmarks...\n");

    let benchmarks = vec![
        benchmark_init,
        benchmark_small_files,
        benchmark_large_file,
        benchmark_duplicates,
        benchmark_mixed_files,
    ];

    let mut results = Vec::new();

    for benchmark in benchmarks {
        match benchmark() {
            Ok(result) => {
                println!("   ✅ Completed");
                results.push(result);
            }
            Err(e) => {
                println!("   ❌ Failed: {}", e);
            }
        }
    }

    if results.is_empty() {
        println!("❌ No benchmarks completed successfully");
        return Ok(());
    }

    print_results_table(&results);
    print_analysis(&results);

    println!("\n🏁 Benchmark completed!");
    println!("Note: Results may vary based on system configuration and file system type.");

    Ok(())
}
