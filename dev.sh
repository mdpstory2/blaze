#!/usr/bin/env bash
set -euo pipefail

# Blaze VCS Development Script
# Comprehensive development helper for building, testing, and development tasks

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
CLEAN="🧹"
CHECK="✅"
CROSS="❌"
INFO="ℹ️"
WARN="⚠️"
PACKAGE="📦"
DOCS="📚"

print_banner() {
    echo -e "${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}"
    echo -e "${FIRE}                                         ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}██${NC}      ${CYAN}█████${NC}  ${CYAN}███████${NC} ${CYAN}███████${NC}  ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}     ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}      ${CYAN}██${NC}       ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}██${NC}     ${CYAN}███████${NC} ${CYAN}███████${NC} ${CYAN}█████${NC}    ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}     ${CYAN}██${NC}   ${CYAN}██${NC}      ${CYAN}██${NC} ${CYAN}██${NC}       ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}███████${NC} ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}███████${NC} ${CYAN}███████${NC}  ${FIRE}"
    echo -e "${FIRE}                                         ${FIRE}"
    echo -e "${FIRE}  ${YELLOW}Development & Build Script${NC}            ${FIRE}"
    echo -e "${FIRE}                                         ${FIRE}"
    echo -e "${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}"
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

# Check if we're in a Rust project
check_rust_project() {
    if [ ! -f "Cargo.toml" ]; then
        error_exit "Not in a Rust project directory (Cargo.toml not found)"
    fi
}

# Install development dependencies
setup() {
    info "Setting up development environment..."

    # Check for Rust
    if ! command -v cargo >/dev/null 2>&1; then
        error_exit "Rust/Cargo not found. Install from https://rustup.rs/"
    fi

    # Check for useful tools and install if missing
    local tools_to_check=(
        "cargo-watch"
        "cargo-expand"
        "cargo-audit"
        "cargo-outdated"
        "hyperfine"
    )

    for tool_name in "${tools_to_check[@]}"; do
        if ! command -v "$tool_name" >/dev/null 2>&1; then
            warn "$tool_name not found, installing..."
            cargo install "$tool_name"
        else
            success "$tool_name is available"
        fi
    done

    success "Development environment setup complete!"
}

# Build in debug mode
build_debug() {
    info "Building in debug mode..."
    cargo build
    success "Debug build completed!"
}

# Build in release mode
build_release() {
    info "Building in release mode with optimizations..."
    cargo build --release
    success "Release build completed!"

    local binary_path="target/release/$PROJECT_NAME"
    if [ -f "$binary_path" ]; then
        local binary_size
        binary_size=$(du -h "$binary_path" | cut -f1)
        info "Binary size: $binary_size"
        info "Binary location: $binary_path"
    fi
}

# Run all tests
test_all() {
    info "Running all tests..."
    cargo test --all-features
    success "All tests passed!"
}

# Run tests with coverage
test_coverage() {
    info "Running tests with coverage..."

    if ! command -v cargo-tarpaulin >/dev/null 2>&1; then
        warn "cargo-tarpaulin not found, installing..."
        cargo install cargo-tarpaulin
    fi

    cargo tarpaulin --all-features --workspace --timeout 120 --out Html
    success "Coverage report generated in tarpaulin-report.html"
}

# Run benchmarks
benchmark() {
    info "Running benchmarks..."
    cargo bench
    success "Benchmarks completed!"
}

# Run clippy (linting)
lint() {
    info "Running Clippy linter..."
    cargo clippy --all-targets --all-features -- -D warnings
    success "No linting issues found!"
}

# Format code
format() {
    info "Formatting code..."
    cargo fmt
    success "Code formatted!"
}

# Check formatting
format_check() {
    info "Checking code formatting..."
    cargo fmt --check
    success "Code formatting is correct!"
}

# Run security audit
audit() {
    info "Running security audit..."
    cargo audit
    success "No security vulnerabilities found!"
}

# Check for outdated dependencies
outdated() {
    info "Checking for outdated dependencies..."
    cargo outdated
}

# Clean build artifacts
clean() {
    info "Cleaning build artifacts..."
    cargo clean
    success "Build artifacts cleaned!"
}

# Watch for changes and rebuild
watch() {
    info "Watching for changes (Ctrl+C to stop)..."
    cargo watch -x "build" -x "test" -x "clippy"
}

# Watch and run specific command
watch_cmd() {
    local cmd="$1"
    info "Watching for changes and running: $cmd"
    cargo watch -x "$cmd"
}

# Generate documentation
docs() {
    info "Generating documentation..."
    cargo doc --all-features --no-deps --open
    success "Documentation generated and opened in browser!"
}

# Run integration tests
integration_tests() {
    info "Running integration tests..."

    # Build first
    cargo build --release

    # Run integration test example
    if [ -f "examples/integration_test.rs" ]; then
        cargo run --example integration_test
    else
        warn "Integration test example not found"
    fi

    success "Integration tests completed!"
}

# Performance comparison with Git
perf_comparison() {
    info "Running performance comparison with Git..."

    if [ ! -f "benchmark.sh" ]; then
        error_exit "benchmark.sh script not found"
    fi

    # Build release version first
    cargo build --release

    # Run benchmark
    ./benchmark.sh
    success "Performance comparison completed! Check benchmark_results.md"
}

# Package for distribution
package() {
    info "Packaging for distribution..."

    # Clean first
    cargo clean

    # Build release
    cargo build --release

    # Run tests
    cargo test --release

    # Package
    cargo package --allow-dirty

    success "Package created successfully!"
}

# Publish to crates.io (dry run by default)
publish() {
    local dry_run="${1:-true}"

    if [ "$dry_run" = "true" ]; then
        info "Performing dry run publish to crates.io..."
        cargo publish --dry-run
        success "Dry run publish completed! Use 'publish false' to actually publish"
    else
        warn "Publishing to crates.io..."
        read -p "Are you sure you want to publish? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            cargo publish
            success "Published to crates.io!"
        else
            info "Publish cancelled"
        fi
    fi
}

# Quick development cycle
dev() {
    info "Running quick development cycle..."
    format
    lint
    test_all
    build_debug
    success "Development cycle completed!"
}

# Full CI pipeline
ci() {
    info "Running full CI pipeline..."
    format_check
    lint
    test_all
    build_release
    audit
    success "CI pipeline completed successfully!"
}

# Create release
release() {
    local version="$1"
    if [ -z "$version" ]; then
        error_exit "Version number required. Usage: ./dev.sh release 1.2.3"
    fi

    info "Creating release $version..."

    # Update Cargo.toml version
    sed -i.bak "s/^version = \".*\"/version = \"$version\"/" Cargo.toml

    # Run full CI
    ci

    # Create git tag
    if command -v git >/dev/null 2>&1; then
        git add Cargo.toml
        git commit -m "Bump version to $version"
        git tag -a "v$version" -m "Release version $version"
        info "Git tag v$version created"
    fi

    success "Release $version prepared!"
    info "Don't forget to push with: git push && git push --tags"
}

# Show project statistics
stats() {
    info "Project Statistics:"
    echo ""

    # Lines of code
    echo -e "${GEAR} Lines of code:"
    find src -name "*.rs" -exec wc -l {} + | tail -1

    # File count
    echo -e "${GEAR} Rust files:"
    find src -name "*.rs" | wc -l

    # Dependencies
    echo -e "${GEAR} Dependencies:"
    cargo tree --depth 1 | grep -v "├──\|└──" | wc -l

    # Binary size (if exists)
    if [ -f "target/release/$PROJECT_NAME" ]; then
        echo -e "${GEAR} Release binary size:"
        du -h "target/release/$PROJECT_NAME"
    fi

    # Test count
    echo -e "${GEAR} Test count:"
    grep -r "#\[test\]" src/ | wc -l

    echo ""
}

# Show help
show_help() {
    echo "Blaze VCS Development Script"
    echo ""
    echo "Usage: $0 <command> [args...]"
    echo ""
    echo "Setup & Environment:"
    echo "  setup              Install development dependencies"
    echo "  stats              Show project statistics"
    echo ""
    echo "Building:"
    echo "  build              Build in debug mode"
    echo "  build-release      Build in release mode"
    echo "  clean              Clean build artifacts"
    echo ""
    echo "Testing:"
    echo "  test               Run all tests"
    echo "  test-coverage      Run tests with coverage report"
    echo "  integration        Run integration tests"
    echo "  benchmark          Run performance benchmarks"
    echo "  perf-comparison    Compare performance with Git"
    echo ""
    echo "Code Quality:"
    echo "  lint               Run Clippy linter"
    echo "  format             Format code with rustfmt"
    echo "  format-check       Check code formatting"
    echo "  audit              Run security audit"
    echo "  outdated           Check for outdated dependencies"
    echo ""
    echo "Development:"
    echo "  watch              Watch for changes and rebuild"
    echo "  watch-cmd <cmd>    Watch and run specific cargo command"
    echo "  dev                Quick development cycle (format+lint+test+build)"
    echo "  docs               Generate and open documentation"
    echo ""
    echo "CI/CD:"
    echo "  ci                 Run full CI pipeline"
    echo "  package            Package for distribution"
    echo "  publish [false]    Publish to crates.io (dry-run by default)"
    echo "  release <version>  Create a new release"
    echo ""
    echo "Examples:"
    echo "  $0 dev                    # Quick development cycle"
    echo "  $0 watch                  # Watch for changes"
    echo "  $0 perf-comparison        # Compare with Git"
    echo "  $0 release 1.2.3          # Create release 1.2.3"
    echo "  $0 publish false          # Actually publish to crates.io"
    echo ""
}

# Main function
main() {
    check_rust_project

    case "${1:-help}" in
        "setup")
            print_banner
            setup
            ;;
        "build")
            print_banner
            build_debug
            ;;
        "build-release"|"release-build")
            print_banner
            build_release
            ;;
        "test")
            print_banner
            test_all
            ;;
        "test-coverage"|"coverage")
            print_banner
            test_coverage
            ;;
        "benchmark"|"bench")
            print_banner
            benchmark
            ;;
        "integration"|"integration-tests")
            print_banner
            integration_tests
            ;;
        "perf-comparison"|"perf"|"compare")
            print_banner
            perf_comparison
            ;;
        "lint"|"clippy")
            print_banner
            lint
            ;;
        "format"|"fmt")
            print_banner
            format
            ;;
        "format-check"|"fmt-check")
            print_banner
            format_check
            ;;
        "audit")
            print_banner
            audit
            ;;
        "outdated")
            print_banner
            outdated
            ;;
        "clean")
            print_banner
            clean
            ;;
        "watch")
            print_banner
            if [ -n "${2:-}" ]; then
                watch_cmd "$2"
            else
                watch
            fi
            ;;
        "watch-cmd")
            if [ -z "${2:-}" ]; then
                error_exit "Command required for watch-cmd"
            fi
            print_banner
            watch_cmd "$2"
            ;;
        "docs"|"doc")
            print_banner
            docs
            ;;
        "dev")
            print_banner
            dev
            ;;
        "ci")
            print_banner
            ci
            ;;
        "package")
            print_banner
            package
            ;;
        "publish")
            print_banner
            publish "${2:-true}"
            ;;
        "release")
            if [ -z "${2:-}" ]; then
                error_exit "Version number required for release"
            fi
            print_banner
            release "$2"
            ;;
        "stats")
            print_banner
            stats
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
