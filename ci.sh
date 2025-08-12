#!/usr/bin/env bash
set -euo pipefail

# Blaze VCS CI/CD Configuration Script
# Sets up continuous integration and deployment workflows

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_NAME="blaze"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Emojis
FIRE="🔥"
ROCKET="🚀"
GEAR="⚙️"
TEST="🧪"
BUILD="🏗️"
DEPLOY="🚀"
CHECK="✅"
CROSS="❌"
INFO="ℹ️"
WARN="⚠️"
CONFIG="⚙️"

print_banner() {
    echo -e "${FIRE} Blaze VCS CI/CD Configuration ${FIRE}"
    echo -e "${CONFIG} Setting up automated workflows"
    echo ""
}

log() {
    echo -e "$1"
}

error_exit() {
    log "${CROSS} ${RED}Error: $1${NC}"
    exit 1
}

success() {
    log "${CHECK} ${GREEN}$1${NC}"
}

info() {
    log "${INFO} ${BLUE}$1${NC}"
}

warn() {
    log "${WARN} ${YELLOW}$1${NC}"
}

# Change to project directory
cd "$SCRIPT_DIR"

# Create GitHub Actions workflow
create_github_actions() {
    info "Creating GitHub Actions workflows..."

    mkdir -p .github/workflows

    # Main CI workflow
    cat > .github/workflows/ci.yml << 'EOF'
name: CI

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test Suite
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        rust: [stable, beta, nightly]
        exclude:
          - os: windows-latest
            rust: nightly
          - os: macos-latest
            rust: nightly

    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@master
      with:
        toolchain: ${{ matrix.rust }}
        components: rustfmt, clippy

    - name: Cache dependencies
      uses: Swatinem/rust-cache@v2

    - name: Check formatting
      run: cargo fmt --all -- --check

    - name: Run Clippy
      run: cargo clippy --all-targets --all-features -- -D warnings

    - name: Build
      run: cargo build --verbose

    - name: Run tests
      run: cargo test --verbose --all-features

    - name: Run integration tests
      run: cargo test --test '*' --verbose

  coverage:
    name: Code Coverage
    runs-on: ubuntu-latest
    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@stable

    - name: Install cargo-tarpaulin
      run: cargo install cargo-tarpaulin

    - name: Generate coverage report
      run: cargo tarpaulin --verbose --all-features --workspace --timeout 120 --out xml

    - name: Upload to codecov.io
      uses: codecov/codecov-action@v3
      with:
        token: ${{ secrets.CODECOV_TOKEN }}
        fail_ci_if_error: true

  security:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@stable

    - name: Install cargo-audit
      run: cargo install cargo-audit

    - name: Run security audit
      run: cargo audit

  benchmark:
    name: Performance Benchmarks
    runs-on: ubuntu-latest
    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@stable

    - name: Install Git (for comparison)
      run: sudo apt-get update && sudo apt-get install -y git

    - name: Install hyperfine
      run: cargo install hyperfine

    - name: Build release
      run: cargo build --release

    - name: Run benchmarks
      run: ./benchmark.sh

    - name: Upload benchmark results
      uses: actions/upload-artifact@v3
      with:
        name: benchmark-results
        path: benchmark_results.md

  build-release:
    name: Build Release Binaries
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            name: blaze-linux-x86_64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            name: blaze-windows-x86_64.exe
          - os: macos-latest
            target: x86_64-apple-darwin
            name: blaze-macos-x86_64
          - os: macos-latest
            target: aarch64-apple-darwin
            name: blaze-macos-aarch64

    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@stable
      with:
        targets: ${{ matrix.target }}

    - name: Cache dependencies
      uses: Swatinem/rust-cache@v2

    - name: Build release binary
      run: cargo build --release --target ${{ matrix.target }}

    - name: Rename binary (Unix)
      if: runner.os != 'Windows'
      run: mv target/${{ matrix.target }}/release/blaze target/${{ matrix.target }}/release/${{ matrix.name }}

    - name: Rename binary (Windows)
      if: runner.os == 'Windows'
      run: move target\${{ matrix.target }}\release\blaze.exe target\${{ matrix.target }}\release\${{ matrix.name }}

    - name: Upload binary
      uses: actions/upload-artifact@v3
      with:
        name: ${{ matrix.name }}
        path: target/${{ matrix.target }}/release/${{ matrix.name }}
EOF

    # Release workflow
    cat > .github/workflows/release.yml << 'EOF'
name: Release

on:
  push:
    tags:
      - 'v*'

env:
  CARGO_TERM_COLOR: always

jobs:
  create-release:
    name: Create Release
    runs-on: ubuntu-latest
    outputs:
      upload_url: ${{ steps.create_release.outputs.upload_url }}
      version: ${{ steps.get_version.outputs.version }}
    steps:
    - name: Get version from tag
      id: get_version
      run: echo "version=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

    - name: Create Release
      id: create_release
      uses: actions/create-release@v1
      env:
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      with:
        tag_name: ${{ github.ref }}
        release_name: Blaze v${{ steps.get_version.outputs.version }}
        draft: false
        prerelease: false
        body: |
          ## What's Changed

          - Performance improvements
          - Bug fixes and stability improvements
          - Enhanced documentation

          ## Installation

          Download the binary for your platform below, or install via:
          ```bash
          curl -sSL https://raw.githubusercontent.com/yourusername/blaze/main/install.sh | bash
          ```

          ## Benchmarks

          See the benchmark results artifact for performance comparisons with Git.

  build-and-upload:
    name: Build and Upload Release Assets
    needs: create-release
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            name: blaze-linux-x86_64
            asset_name: blaze-linux-x86_64.tar.gz
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            name: blaze-windows-x86_64.exe
            asset_name: blaze-windows-x86_64.zip
          - os: macos-latest
            target: x86_64-apple-darwin
            name: blaze-macos-x86_64
            asset_name: blaze-macos-x86_64.tar.gz
          - os: macos-latest
            target: aarch64-apple-darwin
            name: blaze-macos-aarch64
            asset_name: blaze-macos-aarch64.tar.gz

    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@stable
      with:
        targets: ${{ matrix.target }}

    - name: Build release binary
      run: cargo build --release --target ${{ matrix.target }}

    - name: Create archive (Unix)
      if: runner.os != 'Windows'
      run: |
        cd target/${{ matrix.target }}/release
        tar -czf ${{ matrix.asset_name }} blaze
        mv ${{ matrix.asset_name }} ../../../

    - name: Create archive (Windows)
      if: runner.os == 'Windows'
      run: |
        cd target\${{ matrix.target }}\release
        7z a ${{ matrix.asset_name }} blaze.exe
        move ${{ matrix.asset_name }} ..\..\..\

    - name: Upload Release Asset
      uses: actions/upload-release-asset@v1
      env:
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      with:
        upload_url: ${{ needs.create-release.outputs.upload_url }}
        asset_path: ./${{ matrix.asset_name }}
        asset_name: ${{ matrix.asset_name }}
        asset_content_type: application/octet-stream

  publish-crate:
    name: Publish to crates.io
    needs: create-release
    runs-on: ubuntu-latest
    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@stable

    - name: Publish to crates.io
      run: cargo publish --token ${{ secrets.CRATES_IO_TOKEN }}
EOF

    success "GitHub Actions workflows created"
}

# Create GitLab CI configuration
create_gitlab_ci() {
    info "Creating GitLab CI configuration..."

    cat > .gitlab-ci.yml << 'EOF'
stages:
  - test
  - build
  - deploy

variables:
  CARGO_HOME: $CI_PROJECT_DIR/.cargo
  RUST_BACKTRACE: full

cache:
  paths:
    - .cargo/
    - target/

before_script:
  - apt-get update -qq && apt-get install -y -qq git curl build-essential
  - curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  - source $HOME/.cargo/env
  - rustc --version && cargo --version

test:
  stage: test
  script:
    - cargo fmt --all -- --check
    - cargo clippy --all-targets --all-features -- -D warnings
    - cargo test --verbose --all-features
  only:
    - main
    - merge_requests

security_audit:
  stage: test
  script:
    - cargo install cargo-audit
    - cargo audit
  only:
    - main
    - merge_requests

build_debug:
  stage: build
  script:
    - cargo build --verbose
  artifacts:
    paths:
      - target/debug/blaze
    expire_in: 1 week
  only:
    - main

build_release:
  stage: build
  script:
    - cargo build --release --verbose
  artifacts:
    paths:
      - target/release/blaze
    expire_in: 1 month
  only:
    - tags
    - main

benchmark:
  stage: test
  script:
    - cargo build --release
    - ./benchmark.sh
  artifacts:
    paths:
      - benchmark_results.md
    expire_in: 1 week
  only:
    - main

deploy_crates:
  stage: deploy
  script:
    - cargo publish --token $CRATES_IO_TOKEN
  only:
    - tags
  when: manual
EOF

    success "GitLab CI configuration created"
}

# Create Jenkins pipeline
create_jenkins_pipeline() {
    info "Creating Jenkins pipeline..."

    cat > Jenkinsfile << 'EOF'
pipeline {
    agent any

    environment {
        CARGO_HOME = "${WORKSPACE}/.cargo"
        RUST_BACKTRACE = 'full'
    }

    stages {
        stage('Setup') {
            steps {
                sh '''
                    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
                    source $HOME/.cargo/env
                    rustup component add rustfmt clippy
                '''
            }
        }

        stage('Lint') {
            steps {
                sh '''
                    source $HOME/.cargo/env
                    cargo fmt --all -- --check
                    cargo clippy --all-targets --all-features -- -D warnings
                '''
            }
        }

        stage('Test') {
            steps {
                sh '''
                    source $HOME/.cargo/env
                    cargo test --verbose --all-features
                '''
            }
            post {
                always {
                    publishTestResults testResultsPattern: 'target/nextest/default/junit.xml'
                }
            }
        }

        stage('Security Audit') {
            steps {
                sh '''
                    source $HOME/.cargo/env
                    cargo install cargo-audit
                    cargo audit
                '''
            }
        }

        stage('Build Release') {
            when {
                anyOf {
                    branch 'main'
                    tag pattern: 'v\\d+\\.\\d+\\.\\d+', comparator: 'REGEXP'
                }
            }
            steps {
                sh '''
                    source $HOME/.cargo/env
                    cargo build --release
                '''
            }
            post {
                success {
                    archiveArtifacts artifacts: 'target/release/blaze', fingerprint: true
                }
            }
        }

        stage('Benchmark') {
            when {
                branch 'main'
            }
            steps {
                sh '''
                    source $HOME/.cargo/env
                    cargo build --release
                    ./benchmark.sh
                '''
            }
            post {
                always {
                    archiveArtifacts artifacts: 'benchmark_results.md'
                }
            }
        }
    }

    post {
        always {
            cleanWs()
        }
        failure {
            emailext (
                subject: "Build Failed: ${env.JOB_NAME} - ${env.BUILD_NUMBER}",
                body: "Build failed. Check console output at ${env.BUILD_URL}",
                to: "${env.CHANGE_AUTHOR_EMAIL}"
            )
        }
    }
}
EOF

    success "Jenkins pipeline created"
}

# Create Docker configuration for CI
create_docker_ci() {
    info "Creating Docker configuration for CI..."

    cat > Dockerfile.ci << 'EOF'
FROM rust:1.75-slim as builder

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    git \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source
COPY . .

# Build the application
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/blaze /usr/local/bin/blaze

# Create non-root user
RUN useradd -r -s /bin/false blaze

USER blaze

ENTRYPOINT ["blaze"]
EOF

    cat > docker-compose.ci.yml << 'EOF'
version: '3.8'

services:
  blaze-test:
    build:
      context: .
      dockerfile: Dockerfile.ci
    volumes:
      - .:/workspace
    working_dir: /workspace
    command: ["cargo", "test", "--all-features"]

  blaze-benchmark:
    build:
      context: .
      dockerfile: Dockerfile.ci
    volumes:
      - .:/workspace
    working_dir: /workspace
    command: ["./benchmark.sh"]
    depends_on:
      - git-server

  git-server:
    image: git:latest
    volumes:
      - git-repos:/git-repos
    command: ["git", "daemon", "--base-path=/git-repos", "--export-all", "--reuseaddr"]

volumes:
  git-repos:
EOF

    success "Docker CI configuration created"
}

# Create pre-commit hooks
create_pre_commit_hooks() {
    info "Creating pre-commit hooks..."

    mkdir -p .git/hooks

    cat > .git/hooks/pre-commit << 'EOF'
#!/bin/sh
# Blaze VCS pre-commit hook

set -e

echo "🔥 Running pre-commit checks for Blaze VCS..."

# Check if cargo is available
if ! command -v cargo >/dev/null 2>&1; then
    echo "❌ Cargo not found. Please install Rust."
    exit 1
fi

# Format check
echo "🎨 Checking code formatting..."
if ! cargo fmt --all -- --check; then
    echo "❌ Code formatting issues found. Run 'cargo fmt' to fix them."
    exit 1
fi

# Clippy check
echo "📎 Running Clippy..."
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    echo "❌ Clippy found issues. Please fix them before committing."
    exit 1
fi

# Tests
echo "🧪 Running tests..."
if ! cargo test --all-features; then
    echo "❌ Tests failed. Please fix them before committing."
    exit 1
fi

echo "✅ All pre-commit checks passed!"
EOF

    chmod +x .git/hooks/pre-commit

    # Create pre-push hook
    cat > .git/hooks/pre-push << 'EOF'
#!/bin/sh
# Blaze VCS pre-push hook

set -e

echo "🚀 Running pre-push checks for Blaze VCS..."

# Security audit
if command -v cargo-audit >/dev/null 2>&1; then
    echo "🔒 Running security audit..."
    cargo audit
else
    echo "⚠️ cargo-audit not installed, skipping security check"
fi

# Build release to ensure it works
echo "🏗️ Testing release build..."
cargo build --release

echo "✅ All pre-push checks passed!"
EOF

    chmod +x .git/hooks/pre-push

    success "Git hooks created and made executable"
}

# Create coverage configuration
create_coverage_config() {
    info "Creating coverage configuration..."

    cat > .coveragerc << 'EOF'
[run]
source = src/
omit =
    */tests/*
    */test_*
    */examples/*

[report]
exclude_lines =
    pragma: no cover
    def __repr__
    raise AssertionError
    raise NotImplementedError
EOF

    # Tarpaulin configuration
    cat > tarpaulin.toml << 'EOF'
[tool.tarpaulin]
# Tarpaulin configuration for Blaze VCS

# Coverage configuration
line = true
branch = false
count = false
all-features = true
workspace = true
timeout = 120
fail-under = 80

# Output formats
out = ["Html", "Xml"]

# Exclusions
exclude = [
    "*/tests/*",
    "*/examples/*",
]

# Run options
run-types = [
    "Lib",
    "Bins",
]
EOF

    success "Coverage configuration created"
}

# Show help
show_help() {
    echo "Blaze VCS CI/CD Configuration Script"
    echo ""
    echo "Usage: $0 <command>"
    echo ""
    echo "Commands:"
    echo "  all                Set up all CI/CD configurations"
    echo "  github             Set up GitHub Actions workflows"
    echo "  gitlab             Set up GitLab CI configuration"
    echo "  jenkins            Set up Jenkins pipeline"
    echo "  docker             Set up Docker CI configuration"
    echo "  hooks              Set up Git pre-commit/pre-push hooks"
    echo "  coverage           Set up coverage configuration"
    echo "  help               Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 all             # Set up everything"
    echo "  $0 github          # Only GitHub Actions"
    echo "  $0 hooks           # Only Git hooks"
    echo ""
}

# Main function
main() {
    case "${1:-help}" in
        "all")
            print_banner
            create_github_actions
            create_gitlab_ci
            create_jenkins_pipeline
            create_docker_ci
            create_pre_commit_hooks
            create_coverage_config
            success "All CI/CD configurations created!"
            ;;
        "github")
            print_banner
            create_github_actions
            ;;
        "gitlab")
            print_banner
            create_gitlab_ci
            ;;
        "jenkins")
            print_banner
            create_jenkins_pipeline
            ;;
        "docker")
            print_banner
            create_docker_ci
            ;;
        "hooks")
            print_banner
            create_pre_commit_hooks
            ;;
        "coverage")
            print_banner
            create_coverage_config
            ;;
        "help"|"--help"|"-h")
            print_banner
            show_help
            ;;
        *)
            print_banner
            error_exit "Unknown command: $1. Use 'help' to see available commands."
            ;;
    esac
}

# Run main function with all arguments
main "$@"
