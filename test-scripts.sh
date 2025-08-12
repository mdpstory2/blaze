#!/usr/bin/env bash
set -euo pipefail

# Blaze VCS Shell Scripts Test Suite
# Tests all shell scripts to ensure they work correctly

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="$(mktemp -d)"
FAILURES=0

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
TEST="🧪"
CHECK="✅"
CROSS="❌"
INFO="ℹ️"
WARN="⚠️"
ROCKET="🚀"

print_banner() {
    echo -e "${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}"
    echo -e "${FIRE}                                         ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}██${NC}      ${CYAN}█████${NC}  ${CYAN}███████${NC} ${CYAN}███████${NC}  ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}     ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}      ${CYAN}██${NC}       ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}██${NC}     ${CYAN}███████${NC} ${CYAN}███████${NC} ${CYAN}█████${NC}    ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}     ${CYAN}██${NC}   ${CYAN}██${NC}      ${CYAN}██${NC} ${CYAN}██${NC}       ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}███████${NC} ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}███████${NC} ${CYAN}███████${NC}  ${FIRE}"
    echo -e "${FIRE}                                         ${FIRE}"
    echo -e "${FIRE}  ${YELLOW}Shell Scripts Test Suite${NC}             ${FIRE}"
    echo -e "${FIRE}                                         ${FIRE}"
    echo -e "${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}"
    echo ""
    echo -e "${TEST} Testing all shell scripts for functionality and syntax"
    echo ""
}

log() {
    echo -e "$1"
}

error() {
    log "${CROSS} ${RED}$1${NC}"
    ((FAILURES++))
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

# Test script syntax
test_script_syntax() {
    local script="$1"
    local script_name
    script_name=$(basename "$script")

    info "Testing syntax: $script_name"

    if [ ! -f "$script" ]; then
        error "Script not found: $script"
        return 1
    fi

    if [ ! -x "$script" ]; then
        warn "Script not executable: $script_name"
        chmod +x "$script" || {
            error "Cannot make script executable: $script_name"
            return 1
        }
    fi

    # Test bash syntax
    if bash -n "$script"; then
        success "Syntax OK: $script_name"
        return 0
    else
        error "Syntax error in: $script_name"
        return 1
    fi
}

# Test script help functionality
test_script_help() {
    local script="$1"
    local script_name
    script_name=$(basename "$script")

    info "Testing help functionality: $script_name"

    local help_outputs=()

    # Try different help flags
    for flag in "--help" "-h" "help"; do
        if timeout 10s "$script" "$flag" >/dev/null 2>&1; then
            help_outputs+=("$flag")
        fi
    done

    if [ ${#help_outputs[@]} -gt 0 ]; then
        success "Help works: $script_name (${help_outputs[*]})"
        return 0
    else
        warn "No help functionality found: $script_name"
        return 1
    fi
}

# Test specific script functionality
test_install_script() {
    info "Testing install.sh functionality..."

    # Test help
    if ./install.sh --help >/dev/null 2>&1; then
        success "install.sh: Help works"
    else
        error "install.sh: Help failed"
    fi

    # Test version detection (should not fail)
    local temp_script="$TEST_DIR/install_test.sh"
    cp install.sh "$temp_script"

    # Modify to skip actual installation
    sed -i 's/main "\$@"/echo "Test mode - skipping actual installation"/' "$temp_script"

    if bash "$temp_script" --help >/dev/null 2>&1; then
        success "install.sh: Modified test passed"
    else
        warn "install.sh: Modified test issues"
    fi
}

test_dev_script() {
    info "Testing dev.sh functionality..."

    # Test help
    if ./dev.sh help >/dev/null 2>&1; then
        success "dev.sh: Help works"
    else
        error "dev.sh: Help failed"
    fi

    # Test stats (should work in any Rust project)
    if ./dev.sh stats >/dev/null 2>&1; then
        success "dev.sh: Stats command works"
    else
        warn "dev.sh: Stats command issues"
    fi

    # Test format check (if project exists)
    if [ -f "Cargo.toml" ]; then
        if timeout 30s ./dev.sh format-check >/dev/null 2>&1; then
            success "dev.sh: Format check works"
        else
            warn "dev.sh: Format check issues (may need rustfmt)"
        fi
    fi
}

test_benchmark_script() {
    info "Testing benchmark.sh functionality..."

    # Test help
    if ./benchmark.sh --help >/dev/null 2>&1; then
        success "benchmark.sh: Help works"
    else
        error "benchmark.sh: Help failed"
    fi

    # Test prerequisites check (don't run full benchmark)
    local temp_script="$TEST_DIR/benchmark_test.sh"
    cp benchmark.sh "$temp_script"

    # Modify to only check prerequisites
    sed -i '/^main() {/,/^}$/{
        s/main "$@"/echo "Prerequisites check only"/
        /check_prerequisites/!d
    }' "$temp_script"

    if bash "$temp_script" >/dev/null 2>&1; then
        success "benchmark.sh: Prerequisites check works"
    else
        warn "benchmark.sh: Prerequisites check issues"
    fi
}

test_ci_script() {
    info "Testing ci.sh functionality..."

    # Test help
    if ./ci.sh help >/dev/null 2>&1; then
        success "ci.sh: Help works"
    else
        error "ci.sh: Help failed"
    fi

    # Test GitHub Actions creation in temp dir
    local temp_ci_dir="$TEST_DIR/ci_test"
    mkdir -p "$temp_ci_dir"
    cd "$temp_ci_dir"

    if "$SCRIPT_DIR/ci.sh" github >/dev/null 2>&1; then
        if [ -f ".github/workflows/ci.yml" ]; then
            success "ci.sh: GitHub Actions creation works"
        else
            error "ci.sh: GitHub Actions files not created"
        fi
    else
        error "ci.sh: GitHub Actions creation failed"
    fi

    cd "$SCRIPT_DIR"
}

test_deploy_script() {
    info "Testing deploy.sh functionality..."

    # Test help
    if ./deploy.sh help >/dev/null 2>&1; then
        success "deploy.sh: Help works"
    else
        error "deploy.sh: Help failed"
    fi

    # Test clean operation (safe to run)
    if ./deploy.sh clean >/dev/null 2>&1; then
        success "deploy.sh: Clean command works"
    else
        warn "deploy.sh: Clean command issues"
    fi
}

test_uninstall_script() {
    info "Testing uninstall.sh functionality..."

    # Test help
    if ./uninstall.sh --help >/dev/null 2>&1; then
        success "uninstall.sh: Help works"
    else
        error "uninstall.sh: Help failed"
    fi

    # Test in non-destructive mode (just search)
    local temp_script="$TEST_DIR/uninstall_test.sh"
    cp uninstall.sh "$temp_script"

    # Modify to only search, not remove
    sed -i 's/main "\$@"/echo "Search mode only"; find_installations || true/' "$temp_script"

    if bash "$temp_script" >/dev/null 2>&1; then
        success "uninstall.sh: Search functionality works"
    else
        warn "uninstall.sh: Search functionality issues"
    fi
}

# Test shell script standards
test_shell_standards() {
    local script="$1"
    local script_name
    script_name=$(basename "$script")

    info "Testing shell standards: $script_name"

    local issues=0

    # Check for set -euo pipefail
    if ! grep -q "set -euo pipefail" "$script"; then
        warn "$script_name: Missing 'set -euo pipefail'"
        ((issues++))
    fi

    # Check for proper shebang
    if ! head -1 "$script" | grep -q "#!/usr/bin/env bash"; then
        warn "$script_name: Should use '#!/usr/bin/env bash' shebang"
        ((issues++))
    fi

    # Check for undefined variables (basic check)
    if grep -q '\$[A-Z_][A-Z0-9_]*[^}]' "$script" && ! grep -q "set.*u" "$script"; then
        warn "$script_name: May have undefined variable usage"
        ((issues++))
    fi

    # Check for hardcoded paths that should be dynamic
    if grep -q "/usr/local/bin" "$script" && ! grep -q "INSTALL_DIR" "$script"; then
        warn "$script_name: Contains hardcoded paths"
        ((issues++))
    fi

    if [ $issues -eq 0 ]; then
        success "Shell standards OK: $script_name"
        return 0
    else
        warn "Shell standards issues: $script_name ($issues issues)"
        return 1
    fi
}

# Test shell script security
test_script_security() {
    local script="$1"
    local script_name
    script_name=$(basename "$script")

    info "Testing security: $script_name"

    local issues=0

    # Check for potential command injection
    if grep -q 'eval.*\$' "$script"; then
        warn "$script_name: Uses eval with variables (potential injection)"
        ((issues++))
    fi

    # Check for unquoted variables in dangerous contexts
    if grep -q 'rm.*\$[A-Z_]' "$script" | grep -v '"\$'; then
        warn "$script_name: Unquoted variables in rm commands"
        ((issues++))
    fi

    # Check for curl without SSL verification
    if grep -q 'curl.*-k\|curl.*--insecure' "$script"; then
        warn "$script_name: curl with disabled SSL verification"
        ((issues++))
    fi

    # Check for temporary file creation
    if grep -q 'mktemp' "$script"; then
        if grep -q 'trap.*cleanup\|trap.*rm' "$script"; then
            success "$script_name: Good temporary file handling"
        else
            warn "$script_name: Creates temp files but may not clean up"
            ((issues++))
        fi
    fi

    if [ $issues -eq 0 ]; then
        success "Security check OK: $script_name"
        return 0
    else
        warn "Security issues found: $script_name ($issues issues)"
        return 1
    fi
}

# Test all scripts
test_all_scripts() {
    local scripts=(
        "install.sh"
        "dev.sh"
        "benchmark.sh"
        "ci.sh"
        "deploy.sh"
        "uninstall.sh"
    )

    info "Testing all shell scripts..."
    echo ""

    for script in "${scripts[@]}"; do
        if [ -f "$script" ]; then
            echo -e "${INFO} Testing: $script"
            echo "----------------------------------------"

            test_script_syntax "$script"
            test_script_help "$script"
            test_shell_standards "$script"
            test_script_security "$script"

            echo ""
        else
            error "Script not found: $script"
        fi
    done
}

# Test specific functionality
test_specific_functionality() {
    info "Testing specific script functionality..."
    echo ""

    test_install_script
    test_dev_script
    test_benchmark_script
    test_ci_script
    test_deploy_script
    test_uninstall_script

    echo ""
}

# Test script interactions
test_script_interactions() {
    info "Testing script interactions..."

    # Test if dev.sh can call other scripts
    if [ -f "dev.sh" ] && [ -f "benchmark.sh" ]; then
        # Check if dev.sh references benchmark.sh correctly
        if grep -q "benchmark.sh\|perf-comparison" dev.sh; then
            success "dev.sh correctly references benchmark functionality"
        else
            warn "dev.sh may not integrate with benchmarking"
        fi
    fi

    # Test if install.sh and uninstall.sh are complementary
    if [ -f "install.sh" ] && [ -f "uninstall.sh" ]; then
        # Check if they handle the same directories
        local install_dirs
        local uninstall_dirs

        install_dirs=$(grep -o '"/[^"]*bin"' install.sh | sort | uniq || true)
        uninstall_dirs=$(grep -o '"/[^"]*bin"' uninstall.sh | sort | uniq || true)

        if [ "$install_dirs" = "$uninstall_dirs" ]; then
            success "install.sh and uninstall.sh handle same directories"
        else
            warn "install.sh and uninstall.sh may handle different directories"
        fi
    fi
}

# Generate test report
generate_test_report() {
    local total_tests=$(($(find "$SCRIPT_DIR" -name "*.sh" | wc -l) * 4)) # 4 tests per script roughly
    local success_rate

    if [ $total_tests -gt 0 ]; then
        success_rate=$(( (total_tests - FAILURES) * 100 / total_tests ))
    else
        success_rate=0
    fi

    echo ""
    echo -e "${ROCKET} ${BLUE}Test Summary${NC}"
    echo "=========================="
    echo -e "Total Failures: ${FAILURES}"
    echo -e "Success Rate: ${success_rate}%"
    echo ""

    if [ $FAILURES -eq 0 ]; then
        echo -e "${CHECK} ${GREEN}All tests passed! Scripts are ready for use.${NC}"
    elif [ $FAILURES -lt 5 ]; then
        echo -e "${WARN} ${YELLOW}Minor issues found. Scripts should work but may need refinement.${NC}"
    else
        echo -e "${CROSS} ${RED}Significant issues found. Scripts need attention before use.${NC}"
    fi

    echo ""
    echo -e "${INFO} Test artifacts stored in: $TEST_DIR"
    echo ""
}

# Cleanup function
cleanup() {
    if [ -d "$TEST_DIR" ]; then
        rm -rf "$TEST_DIR"
    fi
}

# Main function
main() {
    print_banner

    # Check prerequisites
    info "Checking test prerequisites..."

    if ! command -v bash >/dev/null 2>&1; then
        error "bash not found"
        exit 1
    fi

    if ! command -v timeout >/dev/null 2>&1; then
        warn "timeout command not found - some tests may hang"
    fi

    success "Prerequisites OK"
    echo ""

    # Run all tests
    test_all_scripts
    test_specific_functionality
    test_script_interactions

    # Generate report
    generate_test_report

    # Exit with error code if there were failures
    if [ $FAILURES -gt 0 ]; then
        exit 1
    fi
}

# Trap to cleanup on exit
trap cleanup EXIT

# Show help if requested
if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    echo "Blaze VCS Shell Scripts Test Suite"
    echo ""
    echo "Usage: $0 [options]"
    echo ""
    echo "This script tests all shell scripts in the project for:"
    echo "  - Syntax errors"
    echo "  - Help functionality"
    echo "  - Shell script standards"
    echo "  - Security issues"
    echo "  - Basic functionality"
    echo "  - Script interactions"
    echo ""
    echo "Options:"
    echo "  --help, -h    Show this help message"
    echo ""
    echo "The script will create temporary files in a temp directory"
    echo "and clean them up automatically on exit."
    echo ""
    exit 0
fi

# Run main function
main "$@"
