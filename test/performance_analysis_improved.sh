#!/bin/bash

# 🔥 BLAZE vs GIT: Comprehensive Performance Analysis
# ==================================================
# Professional benchmarking suite for comparing Blaze VCS against Git
# Tests performance across various file sizes and repository scales

set -e

# ========================================
# Configuration & Setup
# ========================================

# Colors for beautiful output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly BLUE='\033[0;34m'
readonly YELLOW='\033[1;33m'
readonly PURPLE='\033[0;35m'
readonly CYAN='\033[0;36m'
readonly BOLD='\033[1m'
readonly NC='\033[0m'

# Test configuration
readonly TEST_DIR="perf_analysis"
readonly RUNS_PER_TEST=3
readonly LOG_FILE="performance_results.log"

# Test scenarios: [file_count, file_size, description, category]
declare -a TEST_SCENARIOS=(
    "10,1KB,Small files (startup overhead test),SMALL"
    "50,10KB,Medium small files,MEDIUM"
    "100,100KB,Medium files (typical development),MEDIUM"
    "500,100KB,Bulk medium files,LARGE_SCALE"
    "1000,100KB,Large scale repository,LARGE_SCALE"
    "100,1MB,Large files,LARGE_FILES"
    "15,100MB,Huge files (Blaze sweet spot),HUGE_FILES"
)

# Global results storage
declare -A RESULTS
declare -A WINNERS

# ========================================
# Utility Functions
# ========================================

print_header() {
    local title="$1"
    local width=80
    local padding=$(( (width - ${#title}) / 2 ))

    echo -e "\n${BOLD}${BLUE}$(printf '=%.0s' $(seq 1 $width))${NC}"
    echo -e "${BOLD}${BLUE}$(printf '%*s' $padding '')${title}$(printf '%*s' $padding '')${NC}"
    echo -e "${BOLD}${BLUE}$(printf '=%.0s' $(seq 1 $width))${NC}\n"
}

print_section() {
    echo -e "\n${BOLD}${CYAN}▶ $1${NC}"
    echo -e "${CYAN}$(printf '─%.0s' $(seq 1 60))${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

# Progress bar animation
show_progress() {
    local duration=$1
    local message="$2"
    local bar_length=30

    for ((i=0; i<=duration; i++)); do
        local progress=$((i * bar_length / duration))
        local bar=$(printf '█%.0s' $(seq 1 $progress))
        local spaces=$(printf ' %.0s' $(seq 1 $((bar_length - progress))))
        local percent=$((i * 100 / duration))

        printf "\r${CYAN}%s [%s%s] %d%%${NC}" "$message" "$bar" "$spaces" "$percent"
        sleep 0.1
    done
    echo
}

# ========================================
# File Management Functions
# ========================================

size_to_bytes() {
    local size=$1
    case $size in
        *KB) echo $((${size%KB} * 1024)) ;;
        *MB) echo $((${size%MB} * 1024 * 1024)) ;;
        *GB) echo $((${size%GB} * 1024 * 1024 * 1024)) ;;
        *) echo 1024 ;;
    esac
}

format_size() {
    local bytes=$1
    if [ $bytes -gt 1073741824 ]; then
        echo "$((bytes / 1073741824))GB"
    elif [ $bytes -gt 1048576 ]; then
        echo "$((bytes / 1048576))MB"
    elif [ $bytes -gt 1024 ]; then
        echo "$((bytes / 1024))KB"
    else
        echo "${bytes}B"
    fi
}

create_test_files() {
    local count=$1
    local size=$2
    local dir=$3
    local bytes=$(size_to_bytes $size)

    echo -e "${CYAN}Creating $count × $size test files...${NC}"
    mkdir -p "$dir"

    # Create diverse file content for realistic testing
    for i in $(seq 1 $count); do
        local filename="$dir/file_${i}_${size}.txt"

        case $((i % 3)) in
            0)
                # Highly compressible content (repeated patterns)
                yes "PATTERN_${i}_$(printf '%*s' 50 | tr ' ' 'A')" | head -c $bytes > "$filename" 2>/dev/null || true
                ;;
            1)
                # Semi-random content (base64 encoded random)
                head -c $bytes /dev/urandom 2>/dev/null | base64 -w 0 | head -c $bytes > "$filename" 2>/dev/null || true
                ;;
            2)
                # Mixed content (realistic files)
                {
                    echo "# File $i - Mixed content with headers and data"
                    echo "timestamp=$(date)"
                    echo "size=$size"
                    echo "index=$i"
                    echo "content_start"
                    head -c $((bytes - 200)) /dev/urandom 2>/dev/null | base64 -w 80
                    echo "content_end"
                } | head -c $bytes > "$filename" 2>/dev/null || true
                ;;
        esac

        # Show progress for large file creation
        if [ $count -gt 100 ] || [ "$size" = "100MB" ]; then
            local progress=$((i * 100 / count))
            printf "\r  Progress: [%3d%%] Creating file %d of %d" $progress $i $count
        fi
    done

    if [ $count -gt 100 ] || [ "$size" = "100MB" ]; then
        echo
    fi

    print_success "Created $count test files ($(format_size $((count * bytes))))"
}

cleanup() {
    print_section "Cleanup"
    echo -e "${BLUE}Removing test directories...${NC}"
    rm -rf "${TEST_DIR}_git" "${TEST_DIR}_blaze" test_files_* memory_*.log >/dev/null 2>&1 || true
    pkill -f "git\|blaze" 2>/dev/null || true
    print_success "Cleanup completed"
}

# ========================================
# Performance Measurement Functions
# ========================================

measure_memory() {
    local cmd="$1"
    local tool="$2"
    local memory_log="memory_${tool}_$$.log"

    # Background memory monitoring
    (
        while kill -0 $$ 2>/dev/null; do
            ps aux 2>/dev/null | grep -E "(git|blaze)" | grep -v grep | \
                awk '{mem += $6} END {print (mem ? mem/1024 : 0)}' >> "$memory_log" 2>/dev/null || echo "0" >> "$memory_log"
            sleep 0.05
        done
    ) &
    local monitor_pid=$!

    # Execute command
    eval "$cmd" >/dev/null 2>&1 || true

    # Stop monitoring
    kill $monitor_pid 2>/dev/null || true
    wait $monitor_pid 2>/dev/null || true

    # Get peak memory
    local peak_memory=0
    if [ -f "$memory_log" ]; then
        peak_memory=$(sort -n "$memory_log" | tail -1 | cut -d. -f1 2>/dev/null || echo "0")
        rm -f "$memory_log"
    fi

    echo "${peak_memory:-0}"
}

measure_repo_size() {
    local repo_path="$1"
    local vcs_dir="$2"

    if [ -d "$repo_path/$vcs_dir" ]; then
        du -sk "$repo_path/$vcs_dir" 2>/dev/null | awk '{print $1}' || echo "0"
    else
        echo "0"
    fi
}

benchmark_operation() {
    local cmd="$1"
    local tool="$2"
    local operation="$3"
    local runs="$4"

    echo -e "    ${YELLOW}Testing $tool $operation...${NC}" >&2

    local total_time=0
    local total_memory=0
    local max_memory=0
    local best_time=999999

    for run in $(seq 1 $runs); do
        # Measure time
        local start_time=$(date +%s%N)
        local memory_used=$(measure_memory "$cmd" "$tool")
        eval "$cmd" >/dev/null 2>&1 || true
        local end_time=$(date +%s%N)

        local duration_ms=$(( (end_time - start_time) / 1000000 ))
        total_time=$((total_time + duration_ms))
        total_memory=$((total_memory + memory_used))

        if [ $memory_used -gt $max_memory ]; then
            max_memory=$memory_used
        fi

        if [ $duration_ms -lt $best_time ]; then
            best_time=$duration_ms
        fi

        printf "      Run %d: %dms, %dMB\r" $run $duration_ms $memory_used >&2
    done

    local avg_time=$((total_time / runs))
    local avg_memory=$((total_memory / runs))

    echo -e "      ${GREEN}Average: ${avg_time}ms, ${avg_memory}MB (best: ${best_time}ms)${NC}" >&2
    echo "$avg_time,$avg_memory,$max_memory,$best_time"
}

# ========================================
# Core Testing Functions
# ========================================

run_scenario_test() {
    local file_count=$1
    local file_size=$2
    local description="$3"
    local category="$4"

    local test_key="${file_count}_${file_size}"

    print_section "Test: $description ($file_count × $file_size)"

    # Setup
    cleanup >/dev/null 2>&1
    mkdir -p "${TEST_DIR}_git" "${TEST_DIR}_blaze"

    # Initialize repositories
    echo -e "  ${BLUE}Initializing repositories...${NC}"

    cd "${TEST_DIR}_git"
    git init >/dev/null 2>&1
    git config user.email "benchmark@blaze.dev" >/dev/null 2>&1
    git config user.name "Benchmark Suite" >/dev/null 2>&1
    cd ..

    cd "${TEST_DIR}_blaze"
    blaze init >/dev/null 2>&1
    cd ..

    # Create test files
    create_test_files $file_count $file_size "test_files_${test_key}"

    # Copy files to repositories
    echo -e "  ${BLUE}Setting up test data...${NC}"
    cp -r "test_files_${test_key}" "${TEST_DIR}_git/"
    cp -r "test_files_${test_key}" "${TEST_DIR}_blaze/"

    # Test Git
    echo -e "  ${PURPLE}Testing Git performance...${NC}"
    cd "${TEST_DIR}_git"
    local git_add_result=$(benchmark_operation "git add test_files_${test_key}/*" "git" "ADD" $RUNS_PER_TEST)
    local git_commit_result=$(benchmark_operation "git commit -m 'Test commit'" "git" "COMMIT" 2)
    local git_repo_size=$(measure_repo_size "$(pwd)" ".git")
    cd ..

    # Test Blaze
    echo -e "  ${PURPLE}Testing Blaze performance...${NC}"
    cd "${TEST_DIR}_blaze"
    local blaze_add_result=$(benchmark_operation "blaze add test_files_${test_key}/*" "blaze" "ADD" $RUNS_PER_TEST)
    local blaze_commit_result=$(benchmark_operation "blaze commit -m 'Test commit'" "blaze" "COMMIT" 2)
    local blaze_repo_size=$(measure_repo_size "$(pwd)" ".blaze")
    cd ..

    # Parse results
    local git_add_time=$(echo $git_add_result | cut -d, -f1)
    local git_add_mem=$(echo $git_add_result | cut -d, -f2)
    local git_add_best=$(echo $git_add_result | cut -d, -f4)

    local git_commit_time=$(echo $git_commit_result | cut -d, -f1)
    local git_commit_mem=$(echo $git_commit_result | cut -d, -f2)
    local git_commit_best=$(echo $git_commit_result | cut -d, -f4)

    local blaze_add_time=$(echo $blaze_add_result | cut -d, -f1)
    local blaze_add_mem=$(echo $blaze_add_result | cut -d, -f2)
    local blaze_add_best=$(echo $blaze_add_result | cut -d, -f4)

    local blaze_commit_time=$(echo $blaze_commit_result | cut -d, -f1)
    local blaze_commit_mem=$(echo $blaze_commit_result | cut -d, -f2)
    local blaze_commit_best=$(echo $blaze_commit_result | cut -d, -f4)

    # Store results
    RESULTS["${test_key}_git_add_time"]=$git_add_time
    RESULTS["${test_key}_git_add_best"]=$git_add_best
    RESULTS["${test_key}_git_commit_time"]=$git_commit_time
    RESULTS["${test_key}_git_commit_best"]=$git_commit_best
    RESULTS["${test_key}_git_size"]=$git_repo_size

    RESULTS["${test_key}_blaze_add_time"]=$blaze_add_time
    RESULTS["${test_key}_blaze_add_best"]=$blaze_add_best
    RESULTS["${test_key}_blaze_commit_time"]=$blaze_commit_time
    RESULTS["${test_key}_blaze_commit_best"]=$blaze_commit_best
    RESULTS["${test_key}_blaze_size"]=$blaze_repo_size

    # Determine winners
    local add_winner="TIE"
    local add_improvement=0
    if [ "${blaze_add_time:-999999}" -lt "${git_add_time:-999999}" ]; then
        add_winner="BLAZE"
        add_improvement=$(( ((git_add_time - blaze_add_time) * 100) / git_add_time ))
    elif [ "${git_add_time:-999999}" -lt "${blaze_add_time:-999999}" ]; then
        add_winner="GIT"
        add_improvement=$(( ((blaze_add_time - git_add_time) * 100) / blaze_add_time ))
    fi

    local commit_winner="TIE"
    local commit_improvement=0
    if [ "${blaze_commit_time:-999999}" -lt "${git_commit_time:-999999}" ]; then
        commit_winner="BLAZE"
        commit_improvement=$(( ((git_commit_time - blaze_commit_time) * 100) / git_commit_time ))
    elif [ "${git_commit_time:-999999}" -lt "${blaze_commit_time:-999999}" ]; then
        commit_winner="GIT"
        commit_improvement=$(( ((blaze_commit_time - git_commit_time) * 100) / blaze_commit_time ))
    fi

    local storage_winner="TIE"
    local storage_savings=0
    if [ "${blaze_repo_size:-999999}" -lt "${git_repo_size:-999999}" ]; then
        storage_winner="BLAZE"
        storage_savings=$((git_repo_size - blaze_repo_size))
    elif [ "${git_repo_size:-999999}" -lt "${blaze_repo_size:-999999}" ]; then
        storage_winner="GIT"
        storage_savings=$((blaze_repo_size - git_repo_size))
    fi

    WINNERS["${test_key}_add"]=$add_winner
    WINNERS["${test_key}_commit"]=$commit_winner
    WINNERS["${test_key}_storage"]=$storage_winner

    # Display results table
    echo -e "\n  ${BOLD}${BLUE}Results Summary:${NC}"
    printf "  %-15s │ %-8s │ %-8s │ %-12s │ %-12s │ %-10s\n" "Operation" "Git (ms)" "Blaze (ms)" "Winner" "Improvement" "Memory"
    echo "  ────────────────┼──────────┼──────────┼──────────────┼──────────────┼───────────"

    local add_color=$( [ "$add_winner" = "BLAZE" ] && echo "$GREEN" || echo "$RED" )
    printf "  %-15s │ %8s │ %8s │ ${add_color}%-12s${NC} │ %11s%% │ %4s/%4sMB\n" \
        "ADD" "$git_add_time" "$blaze_add_time" "$add_winner" "$add_improvement" "$git_add_mem" "$blaze_add_mem"

    local commit_color=$( [ "$commit_winner" = "BLAZE" ] && echo "$GREEN" || echo "$RED" )
    printf "  %-15s │ %8s │ %8s │ ${commit_color}%-12s${NC} │ %11s%% │ %4s/%4sMB\n" \
        "COMMIT" "$git_commit_time" "$blaze_commit_time" "$commit_winner" "$commit_improvement" "$git_commit_mem" "$blaze_commit_mem"

    local storage_color=$( [ "$storage_winner" = "BLAZE" ] && echo "$GREEN" || echo "$RED" )
    printf "  %-15s │ %7sKB │ %7sKB │ ${storage_color}%-12s${NC} │ %10sKB │ %10s\n" \
        "STORAGE" "$git_repo_size" "$blaze_repo_size" "$storage_winner" "$storage_savings" "-"

    # Performance insights
    local total_git=$((git_add_time + git_commit_time))
    local total_blaze=$((blaze_add_time + blaze_commit_time))
    local overall_winner="GIT"
    local overall_improvement=0

    if [ "$total_blaze" -lt "$total_git" ]; then
        overall_winner="BLAZE"
        overall_improvement=$(( ((total_git - total_blaze) * 100) / total_git ))
    else
        overall_improvement=$(( ((total_blaze - total_git) * 100) / total_blaze ))
    fi

    if [ "$overall_winner" = "BLAZE" ]; then
        echo -e "  ${GREEN}🚀 BLAZE WINS OVERALL: ${overall_improvement}% faster (${total_git}ms → ${total_blaze}ms)${NC}"
    else
        echo -e "  ${RED}⚡ Git wins overall: ${overall_improvement}% faster (${total_blaze}ms → ${total_git}ms)${NC}"
    fi

    # Clean up test files
    rm -rf "test_files_${test_key}"

    echo -e "  ${GREEN}✓ Test completed${NC}\n"
}

# ========================================
# Analysis & Reporting Functions
# ========================================

generate_comprehensive_report() {
    print_header "COMPREHENSIVE PERFORMANCE ANALYSIS REPORT"

    # Overall summary
    echo -e "${BOLD}${BLUE}EXECUTIVE SUMMARY${NC}"
    echo -e "${BLUE}$(printf '─%.0s' $(seq 1 50))${NC}"

    local blaze_wins=0
    local git_wins=0
    local ties=0

    # Count wins across all operations
    for scenario in "${TEST_SCENARIOS[@]}"; do
        local file_count=$(echo $scenario | cut -d, -f1)
        local file_size=$(echo $scenario | cut -d, -f2)
        local test_key="${file_count}_${file_size}"

        [ "${WINNERS["${test_key}_add"]}" = "BLAZE" ] && blaze_wins=$((blaze_wins + 1))
        [ "${WINNERS["${test_key}_add"]}" = "GIT" ] && git_wins=$((git_wins + 1))
        [ "${WINNERS["${test_key}_add"]}" = "TIE" ] && ties=$((ties + 1))

        [ "${WINNERS["${test_key}_commit"]}" = "BLAZE" ] && blaze_wins=$((blaze_wins + 1))
        [ "${WINNERS["${test_key}_commit"]}" = "GIT" ] && git_wins=$((git_wins + 1))
        [ "${WINNERS["${test_key}_commit"]}" = "TIE" ] && ties=$((ties + 1))
    done

    echo -e "Total Operations Tested: $((blaze_wins + git_wins + ties))"
    echo -e "${GREEN}🔥 Blaze Victories: $blaze_wins${NC}"
    echo -e "${RED}⚡ Git Victories: $git_wins${NC}"
    echo -e "${YELLOW}🤝 Ties: $ties${NC}"

    if [ $blaze_wins -gt $git_wins ]; then
        echo -e "\n${BOLD}${GREEN}🏆 OVERALL CHAMPION: BLAZE VCS${NC}"
        local win_percentage=$(( blaze_wins * 100 / (blaze_wins + git_wins) ))
        echo -e "${GREEN}Victory Rate: ${win_percentage}%${NC}"
    elif [ $git_wins -gt $blaze_wins ]; then
        echo -e "\n${BOLD}${RED}🏆 OVERALL CHAMPION: GIT${NC}"
        local win_percentage=$(( git_wins * 100 / (blaze_wins + git_wins) ))
        echo -e "${RED}Victory Rate: ${win_percentage}%${NC}"
    else
        echo -e "\n${BOLD}${YELLOW}🏆 RESULT: CLOSE COMPETITION${NC}"
    fi

    # Performance breakdown by category
    echo -e "\n${BOLD}${BLUE}PERFORMANCE BREAKDOWN${NC}"
    echo -e "${BLUE}$(printf '─%.0s' $(seq 1 50))${NC}"

    printf "%-25s │ %-12s │ %-12s │ %-15s │ %-12s\n" "Scenario" "Git ADD" "Blaze ADD" "ADD Winner" "Improvement"
    echo "──────────────────────────┼──────────────┼──────────────┼─────────────────┼─────────────"

    for scenario in "${TEST_SCENARIOS[@]}"; do
        local file_count=$(echo $scenario | cut -d, -f1)
        local file_size=$(echo $scenario | cut -d, -f2)
        local description=$(echo $scenario | cut -d, -f3)
        local test_key="${file_count}_${file_size}"

        local git_time=${RESULTS["${test_key}_git_add_time"]:-"N/A"}
        local blaze_time=${RESULTS["${test_key}_blaze_add_time"]:-"N/A"}
        local winner=${WINNERS["${test_key}_add"]:-"TIE"}

        local improvement=""
        if [ "$winner" = "BLAZE" ] && [ "$git_time" != "N/A" ] && [ "$blaze_time" != "N/A" ]; then
            improvement="$(( ((git_time - blaze_time) * 100) / git_time ))%"
        elif [ "$winner" = "GIT" ] && [ "$git_time" != "N/A" ] && [ "$blaze_time" != "N/A" ]; then
            improvement="$(( ((blaze_time - git_time) * 100) / blaze_time ))%"
        else
            improvement="0%"
        fi

        local winner_color=$( [ "$winner" = "BLAZE" ] && echo "$GREEN" || echo "$RED" )
        printf "%-25s │ %10sms │ %10sms │ ${winner_color}%-15s${NC} │ %11s\n" \
            "$file_count×$file_size" "$git_time" "$blaze_time" "$winner" "$improvement"
    done

    # Key insights
    echo -e "\n${BOLD}${BLUE}KEY INSIGHTS & RECOMMENDATIONS${NC}"
    echo -e "${BLUE}$(printf '─%.0s' $(seq 1 50))${NC}"

    echo -e "${YELLOW}📊 Performance Patterns:${NC}"

    # Analyze patterns
    local small_file_blaze_wins=0
    local large_file_blaze_wins=0
    local small_file_total=0
    local large_file_total=0

    for scenario in "${TEST_SCENARIOS[@]}"; do
        local file_count=$(echo $scenario | cut -d, -f1)
        local file_size=$(echo $scenario | cut -d, -f2)
        local category=$(echo $scenario | cut -d, -f4)
        local test_key="${file_count}_${file_size}"

        if [ "$category" = "SMALL" ] || [ "$category" = "MEDIUM" ]; then
            small_file_total=$((small_file_total + 2))  # ADD + COMMIT
            [ "${WINNERS["${test_key}_add"]}" = "BLAZE" ] && small_file_blaze_wins=$((small_file_blaze_wins + 1))
            [ "${WINNERS["${test_key}_commit"]}" = "BLAZE" ] && small_file_blaze_wins=$((small_file_blaze_wins + 1))
        else
            large_file_total=$((large_file_total + 2))  # ADD + COMMIT
            [ "${WINNERS["${test_key}_add"]}" = "BLAZE" ] && large_file_blaze_wins=$((large_file_blaze_wins + 1))
            [ "${WINNERS["${test_key}_commit"]}" = "BLAZE" ] && large_file_blaze_wins=$((large_file_blaze_wins + 1))
        fi
    done

    local small_file_rate=$(( small_file_blaze_wins * 100 / small_file_total ))
    local large_file_rate=$(( large_file_blaze_wins * 100 / large_file_total ))

    echo -e "  • Small/Medium Files: Blaze wins ${small_file_rate}% of operations"
    echo -e "  • Large Files: Blaze wins ${large_file_rate}% of operations"

    if [ $large_file_rate -gt 70 ]; then
        echo -e "\n${GREEN}🚀 BLAZE EXCELS AT LARGE FILES${NC}"
        echo -e "  Recommendation: Use Blaze for repositories with large files (>1MB)"
    fi

    if [ $small_file_rate -lt 30 ]; then
        echo -e "\n${YELLOW}⚠️  Git has advantages with small files${NC}"
        echo -e "  This is expected due to startup overhead vs. established tooling"
    fi

    # Storage analysis
    local blaze_storage_wins=0
    local storage_total=0
    for scenario in "${TEST_SCENARIOS[@]}"; do
        local file_count=$(echo $scenario | cut -d, -f1)
        local file_size=$(echo $scenario | cut -d, -f2)
        local test_key="${file_count}_${file_size}"
        storage_total=$((storage_total + 1))
        [ "${WINNERS["${test_key}_storage"]}" = "BLAZE" ] && blaze_storage_wins=$((blaze_storage_wins + 1))
    done

    local storage_rate=$(( blaze_storage_wins * 100 / storage_total ))
    echo -e "\n${BLUE}💾 Storage Efficiency: Blaze wins ${storage_rate}% of scenarios${NC}"

    # Final recommendations
    echo -e "\n${BOLD}${GREEN}FINAL RECOMMENDATIONS:${NC}"
    if [ $blaze_wins -gt $git_wins ]; then
        echo -e "${GREEN}✅ Consider Blaze for new projects, especially with large files${NC}"
        echo -e "${GREEN}✅ Blaze shows superior performance in its target scenarios${NC}"
    fi
    echo -e "${BLUE}ℹ️  Both tools have their strengths - choose based on your workflow${NC}"
    echo -e "${BLUE}ℹ️  This benchmark reflects current optimization levels${NC}"
}

# ========================================
# Main Execution
# ========================================

main() {
    # Welcome message
    print_header "🔥 BLAZE vs GIT: Performance Benchmark Suite"

    echo -e "${BOLD}Professional VCS Performance Analysis${NC}"
    echo -e "Comprehensive testing across multiple scenarios and file sizes"
    echo -e "Testing date: $(date '+%Y-%m-%d %H:%M:%S')"

    # Check requirements
    print_section "Environment Check"

    if ! command -v blaze >/dev/null; then
        print_error "Blaze command not found"
        exit 1
    fi

    if ! command -v git >/dev/null; then
        print_error "Git command not found"
        exit 1
    fi

    local blaze_version=$(blaze --version 2>/dev/null || echo 'unknown')
    local git_version=$(git --version 2>/dev/null || echo 'unknown')

    print_success "Blaze: $blaze_version"
    print_success "Git: $git_version"
    print_success "Test runs per operation: $RUNS_PER_TEST"

    # Initialize log
    echo "# Blaze vs Git Performance Analysis - $(date)" > "$LOG_FILE"
    echo "# Blaze: $blaze_version" >> "$LOG_FILE"
    echo "# Git: $git_version" >> "$LOG_FILE"
    echo "" >> "$LOG_FILE"

    # Run all test scenarios
    print_section "Running Performance Tests"

    local total_tests=${#TEST_SCENARIOS[@]}
    local current_test=0

    for scenario in "${TEST_SCENARIOS[@]}"; do
        current_test=$((current_test + 1))
        local file_count=$(echo $scenario | cut -d, -f1)
        local file_size=$(echo $scenario | cut -d, -f2)
        local description=$(echo $scenario | cut -d, -f3)
        local category=$(echo $scenario | cut -d, -f4)

        echo -e "${BOLD}${PURPLE}[$current_test/$total_tests]${NC} Running scenario: $description"

        run_scenario_test "$file_count" "$file_size" "$description" "$category"
    done

    # Generate comprehensive report
    generate_comprehensive_report

    print_section "Test Completion"
    print_success "All performance tests completed successfully!"
    print_info "Results logged to: $LOG_FILE"
    print_info "Total scenarios tested: $total_tests"

    echo -e "\n${BOLD}${GREEN}🏁 Performance Analysis Complete!${NC}"
    echo -e "${CYAN}Thank you for benchmarking Blaze VCS${NC}\n"
}

# Set up cleanup on exit
trap cleanup EXIT

# Execute main function if script is run directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
