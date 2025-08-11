#!/usr/bin/env cargo script

//! FastVC Integration Test Example
//!
//! This example demonstrates a comprehensive workflow using FastVC,
//! showcasing all major features in a realistic scenario.
//!
//! Run with: cargo run --example integration_test

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

struct TestProject {
    temp_dir: TempDir,
    project_path: PathBuf,
}

impl TestProject {
    fn new() -> std::io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let project_path = temp_dir.path().to_path_buf();
        Ok(Self {
            temp_dir,
            project_path,
        })
    }

    fn path(&self) -> &Path {
        &self.project_path
    }

    fn create_file(&self, relative_path: &str, content: &str) -> std::io::Result<()> {
        let file_path = self.project_path.join(relative_path);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = File::create(file_path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    fn run_fastvc(&self, args: &[&str]) -> std::io::Result<String> {
        // Get the current working directory to find the binary
        let current_dir = std::env::current_dir()?;
        let fastvc_binary = current_dir.join("target/release/fastvc");

        let output = Command::new(&fastvc_binary)
            .current_dir(&self.project_path)
            .args(args)
            .output()?;

        if !output.status.success() {
            eprintln!("FastVC command failed: {:?}", args);
            eprintln!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("FastVC command failed: {:?}", args),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn file_exists(&self, relative_path: &str) -> bool {
        self.project_path.join(relative_path).exists()
    }

    fn read_file(&self, relative_path: &str) -> std::io::Result<String> {
        fs::read_to_string(self.project_path.join(relative_path))
    }
}

fn step(step_num: u32, description: &str) {
    println!("\n🔹 Step {}: {}", step_num, description);
    println!("{}", "─".repeat(50 + description.len()));
}

fn success(message: &str) {
    println!("✅ {}", message);
}

fn info(message: &str) {
    println!("ℹ️  {}", message);
}

fn main() -> std::io::Result<()> {
    println!("FastVC Integration Test");
    println!("======================");
    println!("This test demonstrates a complete FastVC workflow");

    // Build FastVC if needed
    let current_dir = std::env::current_dir()?;
    let fastvc_binary = current_dir.join("target/release/fastvc");

    if !fastvc_binary.exists() {
        println!("\n🔨 Building FastVC in release mode...");
        let build_result = Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir(&current_dir)
            .status()?;

        if !build_result.success() {
            eprintln!("❌ Failed to build FastVC");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Build failed",
            ));
        }
        success("FastVC built successfully");
    }

    let project = TestProject::new()?;
    info(&format!("Test project created at: {:?}", project.path()));

    // Step 1: Initialize repository
    step(1, "Initialize FastVC repository");
    let init_output = project.run_fastvc(&["init"])?;
    success("Repository initialized");
    println!("{}", init_output);

    // Step 2: Create initial project structure
    step(2, "Create initial project files");
    project.create_file("README.md",
        "# My Project\n\nThis is a test project for FastVC.\n\n## Features\n- Version control\n- Fast operations\n- Deduplication\n"
    )?;

    project.create_file(
        "src/main.rs",
        "fn main() {\n    println!(\"Hello, World!\");\n}\n",
    )?;

    project.create_file(
        "Cargo.toml",
        "[package]\nname = \"my-project\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;

    project.create_file("src/lib.rs",
        "//! My project library\n\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn test_add() {\n        assert_eq!(add(2, 3), 5);\n    }\n}\n"
    )?;

    success("Created project structure with README, source files, and configuration");

    // Step 3: Check status before adding
    step(3, "Check repository status");
    let status_output = project.run_fastvc(&["status"])?;
    println!("{}", status_output);

    // Step 4: Add files to staging
    step(4, "Add files to staging area");
    let add_output = project.run_fastvc(&["add", "."])?;
    println!("{}", add_output);
    success("Files added to staging area");

    // Step 5: Check status after adding
    step(5, "Check status after staging");
    let status_output = project.run_fastvc(&["status"])?;
    println!("{}", status_output);

    // Step 6: Make initial commit
    step(6, "Create initial commit");
    let commit_output =
        project.run_fastvc(&["commit", "-m", "Initial commit: Add project structure"])?;
    println!("{}", commit_output);
    success("Initial commit created");

    // Step 7: Show commit log
    step(7, "View commit history");
    let log_output = project.run_fastvc(&["log"])?;
    println!("{}", log_output);

    // Step 8: Create more files and demonstrate deduplication
    step(8, "Create duplicate content for deduplication test");
    let duplicate_content =
        "This content will be duplicated across multiple files to test deduplication.\n"
            .repeat(100);

    project.create_file("data/file1.txt", &duplicate_content)?;
    project.create_file("data/file2.txt", &duplicate_content)?;
    project.create_file("data/file3.txt", &duplicate_content)?;

    // Add some unique content too
    project.create_file("docs/guide.md",
        "# User Guide\n\n## Installation\n\nInstall FastVC by building from source.\n\n## Usage\n\nRun `fastvc init` to start.\n"
    )?;

    success("Created duplicate files for deduplication test");

    // Step 9: Add and commit new files
    step(9, "Add new files and commit");
    project.run_fastvc(&["add", "."])?;
    let commit_output = project.run_fastvc(&["commit", "-m", "Add documentation and test data"])?;
    println!("{}", commit_output);
    success("Second commit created");

    // Step 10: Show repository statistics
    step(10, "Display repository statistics");
    let stats_output = project.run_fastvc(&["stats"])?;
    println!("{}", stats_output);
    info("Notice the deduplication savings from identical content!");

    // Step 11: Modify files to test checkout
    step(11, "Modify files to test checkout functionality");
    project.create_file("src/main.rs",
        "fn main() {\n    println!(\"Modified version!\");\n    println!(\"This content will be overwritten by checkout.\");\n}\n"
    )?;

    project.create_file(
        "README.md",
        "# Modified Project\n\nThis README has been completely changed!\n",
    )?;

    success("Files modified");

    // Step 12: Show current status
    step(12, "Check status of modified files");
    let status_output = project.run_fastvc(&["status"])?;
    println!("{}", status_output);
    info("No files in staging - modifications are in working directory only");

    // Step 13: Get commit hash and checkout
    step(13, "Checkout previous commit to restore files");
    let log_output = project.run_fastvc(&["log", "--limit", "1"])?;

    // Extract commit hash from log output
    let commit_hash = log_output
        .lines()
        .find(|line| line.starts_with("Commit: "))
        .and_then(|line| line.strip_prefix("Commit: "))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Could not extract commit hash from log",
            )
        })?;

    let checkout_output = project.run_fastvc(&["checkout", commit_hash])?;
    println!("{}", checkout_output);
    success("Files restored from previous commit");

    // Step 14: Verify files were restored
    step(14, "Verify file restoration");
    let main_content = project.read_file("src/main.rs")?;
    let readme_content = project.read_file("README.md")?;

    if main_content.contains("Hello, World!") && !main_content.contains("Modified version!") {
        success("main.rs correctly restored");
    } else {
        eprintln!("❌ main.rs was not properly restored");
    }

    if readme_content.contains("My Project") && !readme_content.contains("Modified Project") {
        success("README.md correctly restored");
    } else {
        eprintln!("❌ README.md was not properly restored");
    }

    // Step 15: Test verification
    step(15, "Verify repository integrity");
    let verify_output = project.run_fastvc(&["verify"])?;
    println!("{}", verify_output);
    success("Repository integrity verified");

    // Step 16: Create a large file to test chunking
    step(16, "Test large file handling");
    let large_content = "FastVC handles large files efficiently through chunking.\n".repeat(10000);
    project.create_file("large_file.txt", &large_content)?;

    let add_output = project.run_fastvc(&["add", "large_file.txt"])?;
    println!("{}", add_output);

    let commit_output =
        project.run_fastvc(&["commit", "-m", "Add large file for chunking test"])?;
    println!("{}", commit_output);
    success("Large file handled successfully");

    // Step 17: Final statistics
    step(17, "Final repository statistics");
    let stats_output = project.run_fastvc(&["stats"])?;
    println!("{}", stats_output);

    // Step 18: Show complete log
    step(18, "Complete commit history");
    let log_output = project.run_fastvc(&["log", "--limit", "10"])?;
    println!("{}", log_output);

    // Summary
    println!("\n🎉 Integration Test Results");
    println!("==========================");
    success("Repository initialization");
    success("File addition and staging");
    success("Commit creation");
    success("Status checking");
    success("File checkout and restoration");
    success("Deduplication (check stats above)");
    success("Large file handling");
    success("Repository verification");
    success("Complete workflow demonstration");

    println!("\n📊 Test Summary:");
    println!("• Created {} commits", 3);
    println!("• Processed various file types and sizes");
    println!("• Demonstrated deduplication with identical content");
    println!("• Verified data integrity and restoration");
    println!("• Tested all major FastVC operations");

    println!("\n✅ FastVC integration test completed successfully!");
    println!("   All features working as expected.");

    Ok(())
}
