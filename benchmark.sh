#!/usr/bin/env bash
set -euo pipefail

# Blaze vs Git Performance Benchmark Script
# Comprehensive performance comparison between Blaze VCS and Git

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCHMARK_DIR="$(mktemp -d)"
BLAZE_BINARY="${SCRIPT_DIR}/target/release/blaze"
RESULTS_FILE="${SCRIPT_DIR}/benchmark_results.md"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Emojis
FIRE="🔥"
ROCKET="🚀"
CHART="📊"
CLOCK="⏱️"
CHECK="✅"
CROSS="❌"

# Test configuration
# Standard tests
SMALL_FILES=100
MEDIUM_FILES=50
LARGE_FILES=10
COMMITS_TO_TEST=20

# Extended tests - many files
TINY_FILES=1000          # Many tiny files
MANY_LARGE_FILES=100     # Many large files
HUGE_FILES=5             # Few huge files

# File sizes for different scenarios
TINY_SIZE=100            # 100 bytes
SMALL_SIZE=1024          # 1KB
MEDIUM_SIZE=10240        # 10KB
LARGE_SIZE=1048576       # 1MB
HUGE_SIZE=10485760       # 10MB

FILE_SIZES=(100 1024 10240 102400 1048576 10485760)  # 100B, 1KB, 10KB, 100KB, 1MB, 10MB

# File formats to test
FILE_FORMATS=("txt" "json" "xml" "csv" "log" "md" "js" "py" "cpp" "rs")

print_header() {
    echo -e "${FIRE} Blaze vs Git Performance Benchmark ${FIRE}"
    echo -e "${CHART} Testing repository operations with various file sizes and counts"
    echo ""
}

log() {
    echo -e "$1"
    echo "$1" | sed 's/\x1b\[[0-9;]*m//g' >> "$RESULTS_FILE"
}

error_exit() {
    log "${CROSS} ${RED}Error: $1${NC}"
    exit 1
}

success() {
    log "${CHECK} ${GREEN}$1${NC}"
}

info() {
    log "${BLUE}$1${NC}"
}

# Check if required tools are available
check_prerequisites() {
    info "Checking prerequisites..."

    if [ ! -f "$BLAZE_BINARY" ]; then
        error_exit "Blaze binary not found at $BLAZE_BINARY. Please build first with 'cargo build --release'"
    fi

    if ! command -v git >/dev/null 2>&1; then
        error_exit "Git is required for benchmarking"
    fi

    if ! command -v hyperfine >/dev/null 2>&1; then
        log "${YELLOW}Warning: hyperfine not found. Using basic timing instead.${NC}"
        log "Install hyperfine for more accurate benchmarks: https://github.com/sharkdp/hyperfine"
        USE_HYPERFINE=false
    else
        USE_HYPERFINE=true
    fi

    success "Prerequisites satisfied"
}

# Time command execution
time_command() {
    local cmd="$1"
    local iterations="${2:-1}"

    if [ "$USE_HYPERFINE" = true ] && [ "$iterations" -gt 1 ]; then
        hyperfine --runs "$iterations" --warmup 1 "$cmd" --export-json /tmp/hyperfine_result.json
        python3 -c "
import json
with open('/tmp/hyperfine_result.json') as f:
    data = json.load(f)
    print(f\"{data['results'][0]['mean']:.3f}\")
" 2>/dev/null || echo "0.000"
    else
        # Fallback timing
        local start_time end_time duration
        start_time=$(date +%s.%N)
        bash -c "$cmd" >/dev/null 2>&1
        end_time=$(date +%s.%N)
        duration=$(echo "$end_time - $start_time" | bc -l 2>/dev/null || echo "0.000")
        # Use awk for number formatting instead of printf for better compatibility
        echo "$duration" | awk '{printf "%.3f", $1}'
    fi
}

# Compare benchmark times and log results
compare_and_log() {
    local test_name="$1"
    local git_time="$2"
    local blaze_time="$3"

    log "${CHART} File Addition - $test_name:"
    log "  Git:   ${git_time}s"
    log "  Blaze: ${blaze_time}s"

    local speedup
    speedup=$(echo "scale=2; $git_time / $blaze_time" | bc -l 2>/dev/null || echo "1.00")

    if (( $(echo "$blaze_time < $git_time" | bc -l 2>/dev/null || echo 0) )); then
        log "  ${ROCKET} Blaze is ${speedup}x faster!"
    else
        local git_speedup
        git_speedup=$(echo "scale=2; $blaze_time / $git_time" | bc -l 2>/dev/null || echo "1.00")
        log "  Git is ${git_speedup}x faster"
    fi
    log ""
}

# Generate test files of various sizes and formats
generate_test_files() {
    local repo_dir="$1"
    local file_count="$2"
    local file_size="$3"
    local prefix="$4"
    local use_formats="${5:-false}"

    mkdir -p "$repo_dir"
    cd "$repo_dir"

    for i in $(seq 1 "$file_count"); do
        if [ "$use_formats" = "true" ]; then
            # Use different file formats
            local format_index=$(( (i - 1) % ${#FILE_FORMATS[@]} ))
            local extension="${FILE_FORMATS[$format_index]}"
            local filename="${prefix}_${file_size}b_${i}.${extension}"

            # Generate format-specific content
            case "$extension" in
                "json")
                    echo "{\"id\": $i, \"data\": \"" > "$filename"
                    head -c $(( file_size - 20 )) /dev/urandom | base64 | tr -d '\n' >> "$filename"
                    echo "\"}" >> "$filename"
                    ;;
                "xml")
                    echo "<?xml version=\"1.0\"?><root><id>$i</id><data>" > "$filename"
                    head -c $(( file_size - 50 )) /dev/urandom | base64 | tr -d '\n' >> "$filename"
                    echo "</data></root>" >> "$filename"
                    ;;
                "csv")
                    echo "id,timestamp,data" > "$filename"
                    for j in $(seq 1 $(( file_size / 50 ))); do
                        echo "$j,$(date +%s),$(head -c 30 /dev/urandom | base64 | tr -d '\n')" >> "$filename"
                    done
                    ;;
                *)
                    # Default text content
                    head -c "$file_size" /dev/urandom | base64 > "$filename"
                    ;;
            esac
        else
            local filename="${prefix}_${file_size}b_${i}.txt"
            if [ "$file_size" -lt 1024 ]; then
                # Small files - generate text content
                head -c "$file_size" /dev/urandom | base64 > "$filename"
            else
                # Larger files - use dd for speed
                dd if=/dev/urandom of="$filename" bs="$file_size" count=1 2>/dev/null
            fi
        fi
    done
}

# Initialize repositories
init_repos() {
    local base_dir="$1"

    # Git repo
    local git_repo="$base_dir/git_repo"
    mkdir -p "$git_repo"
    cd "$git_repo"
    git init >/dev/null 2>&1
    git config user.name "Benchmark Test" >/dev/null 2>&1
    git config user.email "test@benchmark.local" >/dev/null 2>&1

    # Blaze repo
    local blaze_repo="$base_dir/blaze_repo"
    mkdir -p "$blaze_repo"
    cd "$blaze_repo"
    "$BLAZE_BINARY" init >/dev/null 2>&1
}

# Benchmark initialization
benchmark_init() {
    info "Benchmarking repository initialization..."

    local test_dir="$BENCHMARK_DIR/init_test"
    mkdir -p "$test_dir"

    # Git init
    cd "$test_dir"
    local git_time
    git_time=$(time_command "git init git_repo" 5)

    # Blaze init
    local blaze_time
    blaze_time=$(time_command "$BLAZE_BINARY init blaze_repo" 5)

    log "${CHART} Repository Initialization:"
    log "  Git:   ${git_time}s"
    log "  Blaze: ${blaze_time}s"

    local speedup
    speedup=$(echo "scale=2; $git_time / $blaze_time" | bc -l)
    if (( $(echo "$blaze_time < $git_time" | bc -l) )); then
        log "  ${ROCKET} Blaze is ${speedup}x faster!"
    else
        log "  Git is faster by $(echo "scale=2; $blaze_time / $git_time" | bc -l)x"
    fi
    log ""
}

# Benchmark file addition
benchmark_add() {
    local file_count="$1"
    local file_size="$2"
    local test_name="$3"

    info "Benchmarking file addition: $test_name ($file_count files of ${file_size}B each)..."

    local test_dir="$BENCHMARK_DIR/add_test_${test_name}"
    init_repos "$test_dir"

    # Generate files for Git
    generate_test_files "$test_dir/git_repo" "$file_count" "$file_size" "file"

    # Copy files to Blaze repo
    cp -r "$test_dir/git_repo"/* "$test_dir/blaze_repo/"

    # Benchmark Git add
    cd "$test_dir/git_repo"
    local git_time
    git_time=$(time_command "git add ." 3)

    # Benchmark Blaze add
    cd "$test_dir/blaze_repo"
    local blaze_time
    blaze_time=$(time_command "$BLAZE_BINARY add ." 3)

    log "${CHART} File Addition - $test_name:"
    log "  Git:   ${git_time}s"
    log "  Blaze: ${blaze_time}s"

    local speedup
    speedup=$(echo "scale=2; $git_time / $blaze_time" | bc -l 2>/dev/null || echo "1.00")
    if (( $(echo "$blaze_time < $git_time" | bc -l 2>/dev/null || echo 0) )); then
        log "  ${ROCKET} Blaze is ${speedup}x faster!"
    else
        local git_speedup
        git_speedup=$(echo "scale=2; $blaze_time / $git_time" | bc -l 2>/dev/null || echo "1.00")
        log "  Git is ${git_speedup}x faster"
    fi
    log ""
}

# Benchmark commits
benchmark_commit() {
    info "Benchmarking commit creation..."

    local test_dir="$BENCHMARK_DIR/commit_test"
    init_repos "$test_dir"

    # Prepare files
    generate_test_files "$test_dir/git_repo" 20 10240 "commit_file"
    cp -r "$test_dir/git_repo"/* "$test_dir/blaze_repo/"

    # Add files first
    cd "$test_dir/git_repo"
    git add . >/dev/null 2>&1

    cd "$test_dir/blaze_repo"
    "$BLAZE_BINARY" add . >/dev/null 2>&1

    # Benchmark Git commit
    cd "$test_dir/git_repo"
    local git_time
    git_time=$(time_command 'git commit -m "Benchmark commit"' 3)

    # Benchmark Blaze commit
    cd "$test_dir/blaze_repo"
    local blaze_time
    blaze_time=$(time_command '"$BLAZE_BINARY" commit -m "Benchmark commit"' 3)

    log "${CHART} Commit Creation:"
    log "  Git:   ${git_time}s"
    log "  Blaze: ${blaze_time}s"

    local speedup
    speedup=$(echo "scale=2; $git_time / $blaze_time" | bc -l 2>/dev/null || echo "1.00")
    if (( $(echo "$blaze_time < $git_time" | bc -l 2>/dev/null || echo 0) )); then
        log "  ${ROCKET} Blaze is ${speedup}x faster!"
    else
        local git_speedup
        git_speedup=$(echo "scale=2; $blaze_time / $git_time" | bc -l 2>/dev/null || echo "1.00")
        log "  Git is ${git_speedup}x faster"
    fi
    log ""
}

# Benchmark status check
benchmark_status() {
    info "Benchmarking status checks..."

    local test_dir="$BENCHMARK_DIR/status_test"
    init_repos "$test_dir"

    # Create initial files and commit
    generate_test_files "$test_dir/git_repo" 50 5120 "status_file"
    cp -r "$test_dir/git_repo"/* "$test_dir/blaze_repo/"

    # Initial commit
    cd "$test_dir/git_repo"
    git add . >/dev/null 2>&1
    git commit -m "Initial commit" >/dev/null 2>&1

    cd "$test_dir/blaze_repo"
    "$BLAZE_BINARY" add . >/dev/null 2>&1
    "$BLAZE_BINARY" commit -m "Initial commit" >/dev/null 2>&1

    # Modify some files
    echo "modified" >> "$test_dir/git_repo/status_file_5120b_1.txt"
    echo "modified" >> "$test_dir/blaze_repo/status_file_5120b_1.txt"

    # Add new files
    generate_test_files "$test_dir/git_repo" 10 2048 "new_file"
    cp "$test_dir/git_repo"/new_file* "$test_dir/blaze_repo/"

    # Benchmark Git status
    cd "$test_dir/git_repo"
    local git_time
    git_time=$(time_command "git status" 5)

    # Benchmark Blaze status
    cd "$test_dir/blaze_repo"
    local blaze_time
    blaze_time=$(time_command "$BLAZE_BINARY status" 5)

    log "${CHART} Status Check:"
    log "  Git:   ${git_time}s"
    log "  Blaze: ${blaze_time}s"

    local speedup
    speedup=$(echo "scale=2; $git_time / $blaze_time" | bc -l 2>/dev/null || echo "1.00")
    if (( $(echo "$blaze_time < $git_time" | bc -l 2>/dev/null || echo 0) )); then
        log "  ${ROCKET} Blaze is ${speedup}x faster!"
    else
        local git_speedup
        git_speedup=$(echo "scale=2; $blaze_time / $git_time" | bc -l 2>/dev/null || echo "1.00")
        log "  Git is ${git_speedup}x faster"
    fi
    log ""
}

# Benchmark log viewing
benchmark_log() {
    info "Benchmarking log operations..."

    local test_dir="$BENCHMARK_DIR/log_test"
    init_repos "$test_dir"

    # Create multiple commits
    for i in $(seq 1 10); do
        cd "$test_dir/git_repo"
        echo "commit $i content" > "file_$i.txt"
        git add "file_$i.txt" >/dev/null 2>&1
        git commit -m "Commit $i" >/dev/null 2>&1

        cd "$test_dir/blaze_repo"
        echo "commit $i content" > "file_$i.txt"
        "$BLAZE_BINARY" add "file_$i.txt" >/dev/null 2>&1
        "$BLAZE_BINARY" commit -m "Commit $i" >/dev/null 2>&1
    done

    # Benchmark Git log
    cd "$test_dir/git_repo"
    local git_time
    git_time=$(time_command "git log --oneline" 5)

    # Benchmark Blaze log
    cd "$test_dir/blaze_repo"
    local blaze_time
    blaze_time=$(time_command "$BLAZE_BINARY log --oneline" 5)

    log "${CHART} Log Operations:"
    log "  Git:   ${git_time}s"
    log "  Blaze: ${blaze_time}s"

    local speedup
    speedup=$(echo "scale=2; $git_time / $blaze_time" | bc -l 2>/dev/null || echo "1.00")
    if (( $(echo "$blaze_time < $git_time" | bc -l 2>/dev/null || echo 0) )); then
        log "  ${ROCKET} Blaze is ${speedup}x faster!"
    else
        local git_speedup
        git_speedup=$(echo "scale=2; $blaze_time / $git_time" | bc -l 2>/dev/null || echo "1.00")
        log "  Git is ${git_speedup}x faster"
    fi
    log ""
}

# Check repository sizes
benchmark_storage() {
    info "Comparing repository storage efficiency..."

    local test_dir="$BENCHMARK_DIR/storage_test"
    init_repos "$test_dir"

    # Create files with some duplication
    generate_test_files "$test_dir/git_repo" 30 8192 "storage_file"

    # Create some duplicate content
    for i in {1..10}; do
        cp "$test_dir/git_repo/storage_file_8192b_1.txt" "$test_dir/git_repo/duplicate_$i.txt"
    done

    cp -r "$test_dir/git_repo"/* "$test_dir/blaze_repo/"

    # Add and commit all files
    cd "$test_dir/git_repo"
    git add . >/dev/null 2>&1
    git commit -m "Storage test commit" >/dev/null 2>&1

    cd "$test_dir/blaze_repo"
    "$BLAZE_BINARY" add . >/dev/null 2>&1
    "$BLAZE_BINARY" commit -m "Storage test commit" >/dev/null 2>&1

    # Calculate repository sizes
    local git_size blaze_size
    git_size=$(du -sb "$test_dir/git_repo/.git" | cut -f1)
    blaze_size=$(du -sb "$test_dir/blaze_repo/.blaze" | cut -f1)

    local git_size_mb blaze_size_mb
    git_size_mb=$(echo "scale=2; $git_size / 1024 / 1024" | bc -l)
    blaze_size_mb=$(echo "scale=2; $blaze_size / 1024 / 1024" | bc -l)

    log "${CHART} Repository Storage (with duplicates):"
    log "  Git:   ${git_size_mb} MB"
    log "  Blaze: ${blaze_size_mb} MB"

    if (( $(echo "$blaze_size < $git_size" | bc -l) )); then
        local savings
        savings=$(echo "scale=1; ($git_size - $blaze_size) * 100 / $git_size" | bc -l)
        log "  ${ROCKET} Blaze saves ${savings}% storage space!"
    else
        local overhead
        overhead=$(echo "scale=1; ($blaze_size - $git_size) * 100 / $git_size" | bc -l)
        log "  Git uses ${overhead}% less storage"
    fi
    log ""
}

# Generate comprehensive report
generate_report() {
    log ""
    log "${FIRE} BENCHMARK SUMMARY ${FIRE}"
    log "========================================"
    log ""
    log "Test Environment:"
    log "  OS: $(uname -s) $(uname -r)"
    log "  CPU: $(nproc) cores"
    log "  Memory: $(free -h | awk '/^Mem:/ {print $2}') total"
    log "  Disk: $(df -h . | awk 'NR==2 {print $4}') available"
    log "  Git Version: $(git --version)"
    log "  Blaze Version: $($BLAZE_BINARY --version 2>/dev/null || echo 'Unknown')"
    log ""
    log "Benchmark Configuration:"
    log "  Small files test: $SMALL_FILES files"
    log "  Medium files test: $MEDIUM_FILES files"
    log "  Large files test: $LARGE_FILES files"
    log "  File sizes tested: ${FILE_SIZES[*]} bytes"
    log ""
    log "Results saved to: $RESULTS_FILE"
    log ""

    success "Benchmark completed! Check $RESULTS_FILE for detailed results."
}

# Cleanup function
cleanup() {
    if [ -d "$BENCHMARK_DIR" ]; then
        rm -rf "$BENCHMARK_DIR"
    fi
}

# Main benchmark function
# Benchmark file operations with different formats
benchmark_file_operations_formats() {
    local test_name="$1"
    local file_count="$2"
    local file_size="$3"

    info "Benchmarking file addition: $test_name ($file_count files of ${file_size}B each with mixed formats)..."

    local git_repo="$BENCHMARK_DIR/git_repo"
    local blaze_repo="$BENCHMARK_DIR/blaze_repo"

    # Generate test files with different formats
    generate_test_files "$git_repo" "$file_count" "$file_size" "$test_name" "true"
    generate_test_files "$blaze_repo" "$file_count" "$file_size" "$test_name" "true"

    # Benchmark Git
    cd "$git_repo"
    local git_time
    git_time=$(time_command "git add ${test_name}_*")

    # Benchmark Blaze
    cd "$blaze_repo"
    local blaze_time
    blaze_time=$(time_command "$BLAZE_BINARY add ${test_name}_*")

    # Compare and log results
    compare_and_log "$test_name" "$git_time" "$blaze_time"

    # Clean up
    cd "$git_repo" && rm -f ${test_name}_*
    cd "$blaze_repo" && rm -f ${test_name}_*
}

main() {
    print_header

    # Initialize results file
    echo "# Blaze vs Git Performance Benchmark Results" > "$RESULTS_FILE"
    echo "Generated on: $(date)" >> "$RESULTS_FILE"
    echo "" >> "$RESULTS_FILE"

    check_prerequisites

    log "Starting comprehensive performance benchmark..."
    log "Benchmark directory: $BENCHMARK_DIR"
    log ""

    # Run all benchmarks
    benchmark_init

    # Standard tests
    benchmark_add "$SMALL_FILES" 1024 "small_files"
    benchmark_add "$MEDIUM_FILES" 10240 "medium_files"
    benchmark_add "$LARGE_FILES" 1048576 "large_files"

    # Extended tests - extreme scenarios
    benchmark_add "$TINY_FILES" "$TINY_SIZE" "tiny_files"
    benchmark_add "$MANY_LARGE_FILES" "$LARGE_SIZE" "many_large_files"
    benchmark_add "$HUGE_FILES" "$HUGE_SIZE" "huge_files"

    # Mixed format tests
    benchmark_file_operations_formats "mixed_small_formats" 200 "$SMALL_SIZE"
    benchmark_file_operations_formats "mixed_medium_formats" 100 "$MEDIUM_SIZE"
    benchmark_file_operations_formats "mixed_large_formats" 50 "$LARGE_SIZE"

    benchmark_commit
    benchmark_status
    benchmark_log
    benchmark_storage

    generate_report
}

# Trap to cleanup on exit
trap cleanup EXIT

# Check for help flag
if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    echo "Blaze vs Git Performance Benchmark"
    echo ""
    echo "Usage: $0 [options]"
    echo ""
    echo "This script compares Blaze VCS performance against Git across various operations:"
    echo "  - Repository initialization"
    echo "  - File addition (various sizes)"
    echo "  - Commit creation"
    echo "  - Status checking"
    echo "  - Log operations"
    echo "  - Storage efficiency"
    echo ""
    echo "Requirements:"
    echo "  - Git installed"
    echo "  - Blaze binary built (cargo build --release)"
    echo "  - hyperfine (optional, for better timing)"
    echo "  - bc calculator"
    echo ""
    echo "Results are saved to benchmark_results.md"
    exit 0
fi

# Run main function
main "$@"
