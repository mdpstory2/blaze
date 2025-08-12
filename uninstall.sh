#!/usr/bin/env bash
set -euo pipefail

# Blaze VCS Uninstaller Script
# A blazingly fast, chunk-based version control system

BLAZE_VERSION="0.1.0"
BINARY_NAME="blaze"
SYSTEM_INSTALL_DIR="/usr/local/bin"
USER_INSTALL_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/blaze"
CACHE_DIR="$HOME/.cache/blaze"

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
TRASH="🗑️"
CHECK="✅"
CROSS="❌"
INFO="ℹ️"
WARN="⚠️"
QUESTION="❓"

print_banner() {
    echo -e "${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}"
    echo -e "${FIRE}                                              ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}██${NC}      ${CYAN}█████${NC}  ${CYAN}███████${NC} ${CYAN}███████${NC}  ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}     ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}      ${CYAN}██${NC}       ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}██${NC}     ${CYAN}███████${NC} ${CYAN}███████${NC} ${CYAN}█████${NC}    ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}     ${CYAN}██${NC}   ${CYAN}██${NC}      ${CYAN}██${NC} ${CYAN}██${NC}       ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}███████${NC} ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}███████${NC} ${CYAN}███████${NC}  ${FIRE}"
    echo -e "${FIRE}                                              ${FIRE}"
    echo -e "${FIRE}  ${RED}UNINSTALLER${NC} ${YELLOW}- Removing Blaze VCS${NC}           ${FIRE}"
    echo -e "${FIRE}  ${PURPLE}Clean removal • Safe • Thorough${NC}             ${FIRE}"
    echo -e "${FIRE}                                              ${FIRE}"
    echo -e "${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}"
    echo ""
    echo -e "${TRASH} Blaze VCS Uninstaller v${BLAZE_VERSION}"
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

question() {
    log "${QUESTION} ${PURPLE}$1${NC}"
}

confirm() {
    local message="$1"
    local default="${2:-N}"

    if [ "$default" = "Y" ]; then
        read -p "$(echo -e "${QUESTION} ${PURPLE}${message} (Y/n): ${NC}")" -n 1 -r
        echo
        [[ $REPLY =~ ^[Nn]$ ]] && return 1
    else
        read -p "$(echo -e "${QUESTION} ${PURPLE}${message} (y/N): ${NC}")" -n 1 -r
        echo
        [[ ! $REPLY =~ ^[Yy]$ ]] && return 1
    fi
    return 0
}

find_blaze_installations() {
    local found_installations=()

    # Check common installation locations
    local locations=(
        "$SYSTEM_INSTALL_DIR/$BINARY_NAME"
        "$USER_INSTALL_DIR/$BINARY_NAME"
        "$HOME/bin/$BINARY_NAME"
        "/opt/blaze/$BINARY_NAME"
        "/usr/bin/$BINARY_NAME"
        "/bin/$BINARY_NAME"
    )

    info "Scanning for Blaze installations..." >&2

    for location in "${locations[@]}"; do
        if [ -f "$location" ]; then
            found_installations+=("$location")
            info "Found: $location" >&2
        fi
    done

    # Also check PATH for any blaze binaries
    if command -v blaze >/dev/null 2>&1; then
        local path_location=$(which blaze)
        # Check if it's not already in our list
        local already_found=false
        for found in "${found_installations[@]}"; do
            if [ "$found" = "$path_location" ]; then
                already_found=true
                break
            fi
        done

        if [ "$already_found" = false ]; then
            found_installations+=("$path_location")
            info "Found in PATH: $path_location" >&2
        fi
    fi

    # Return the array (bash way to return arrays)
    printf '%s\n' "${found_installations[@]}"
}

remove_binary() {
    local binary_path="$1"
    local dry_run="$2"

    if [ "$dry_run" = true ]; then
        info "[DRY RUN] Would remove: $binary_path"
        return 0
    fi

    info "Removing binary: $binary_path"

    # Check if we need sudo
    if [ -w "$binary_path" ] || [ -w "$(dirname "$binary_path")" ]; then
        if rm -f "$binary_path"; then
            success "Removed: $binary_path"
            return 0
        else
            warn "Failed to remove: $binary_path"
            return 1
        fi
    else
        info "Removing with sudo: $binary_path"
        if sudo rm -f "$binary_path"; then
            success "Removed: $binary_path"
            return 0
        else
            warn "Failed to remove with sudo: $binary_path"
            return 1
        fi
    fi
}

remove_config_files() {
    local dry_run="$1"
    local removed_any=false

    info "Checking for configuration and cache files..."

    # List of potential config/cache locations
    local config_locations=(
        "$CONFIG_DIR"
        "$CACHE_DIR"
        "$HOME/.blaze"
    )

    for location in "${config_locations[@]}"; do
        if [ -e "$location" ]; then
            if [ "$dry_run" = true ]; then
                info "[DRY RUN] Would remove: $location"
                removed_any=true
            else
                if confirm "Remove configuration/cache directory: $location?"; then
                    if rm -rf "$location"; then
                        success "Removed: $location"
                        removed_any=true
                    else
                        warn "Failed to remove: $location"
                    fi
                else
                    info "Skipped: $location"
                fi
            fi
        fi
    done

    if [ "$removed_any" = false ]; then
        info "No configuration or cache files found"
    fi
}

remove_shell_completions() {
    local dry_run="$1"
    local removed_any=false

    info "Checking for shell completions..."

    # Common completion locations
    local completion_locations=(
        "/etc/bash_completion.d/blaze"
        "/usr/share/bash-completion/completions/blaze"
        "$HOME/.local/share/bash-completion/completions/blaze"
        "/usr/share/zsh/site-functions/_blaze"
        "$HOME/.local/share/zsh/site-functions/_blaze"
        "/usr/share/fish/completions/blaze.fish"
        "$HOME/.local/share/fish/completions/blaze.fish"
    )

    for location in "${completion_locations[@]}"; do
        if [ -f "$location" ]; then
            if [ "$dry_run" = true ]; then
                info "[DRY RUN] Would remove completion: $location"
                removed_any=true
            else
                if [ -w "$location" ] || [ -w "$(dirname "$location")" ]; then
                    rm -f "$location" && success "Removed completion: $location"
                    removed_any=true
                else
                    sudo rm -f "$location" && success "Removed completion: $location"
                    removed_any=true
                fi
            fi
        fi
    done

    if [ "$removed_any" = false ]; then
        info "No shell completions found"
    fi
}

check_active_repositories() {
    local found_repos=0

    info "Scanning for active Blaze repositories..."

    # Look for .blaze directories in common places
    local search_dirs=(
        "$HOME"
        "$HOME/Projects"
        "$HOME/Code"
        "$HOME/src"
        "$HOME/Documents"
        "/tmp"
    )

    for dir in "${search_dirs[@]}"; do
        if [ -d "$dir" ]; then
            while IFS= read -r -d '' blaze_dir; do
                local repo_dir=$(dirname "$blaze_dir")
                warn "Found Blaze repository: $repo_dir"
                found_repos=$((found_repos + 1))
            done < <(find "$dir" -type d -name ".blaze" -print0 2>/dev/null | head -20)
        fi
    done

    if [ $found_repos -gt 0 ]; then
        warn "Found $found_repos Blaze repositories"
        warn "These repositories will become inaccessible without Blaze installed"
        warn "Consider backing up or converting them before uninstalling"
        echo
        if ! confirm "Continue with uninstallation anyway?"; then
            info "Uninstallation cancelled"
            exit 0
        fi
    else
        info "No active Blaze repositories found"
    fi
}

verify_removal() {
    info "Verifying removal..."

    if command -v blaze >/dev/null 2>&1; then
        warn "Blaze command still found in PATH: $(which blaze)"
        warn "You may need to restart your shell or check your PATH"
        return 1
    else
        success "Blaze command no longer found in PATH"
        return 0
    fi
}

show_help() {
    echo "Blaze VCS Uninstaller"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --help              Show this help message"
    echo "  --dry-run           Show what would be removed without actually removing"
    echo "  --force             Skip confirmation prompts"
    echo "  --keep-config       Don't remove configuration and cache files"
    echo "  --system-only       Only remove from system locations (/usr/local/bin)"
    echo "  --user-only         Only remove from user locations (~/.local/bin)"
    echo ""
    echo "Examples:"
    echo "  $0                  # Interactive uninstallation"
    echo "  $0 --dry-run        # See what would be removed"
    echo "  $0 --force          # Remove without confirmations"
    echo "  $0 --keep-config    # Remove binary but keep config files"
    echo ""
}

main() {
    local DRY_RUN=false
    local FORCE=false
    local KEEP_CONFIG=false
    local SYSTEM_ONLY=false
    local USER_ONLY=false

    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --help|-h)
                show_help
                exit 0
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --force)
                FORCE=true
                shift
                ;;
            --keep-config)
                KEEP_CONFIG=true
                shift
                ;;
            --system-only)
                SYSTEM_ONLY=true
                shift
                ;;
            --user-only)
                USER_ONLY=true
                shift
                ;;
            *)
                error_exit "Unknown option: $1"
                ;;
        esac
    done

    print_banner

    if [ "$DRY_RUN" = true ]; then
        warn "DRY RUN MODE - No files will actually be removed"
        echo
    fi

    # Find all Blaze installations
    mapfile -t installations < <(find_blaze_installations)

    if [ ${#installations[@]} -eq 0 ]; then
        info "No Blaze installations found"
        exit 0
    fi

    echo
    question "Found ${#installations[@]} Blaze installation(s):"
    for installation in "${installations[@]}"; do
        echo "  • $installation"
    done
    echo

    # Check for active repositories unless forced
    if [ "$FORCE" = false ] && [ "$DRY_RUN" = false ]; then
        check_active_repositories
    fi

    # Confirm uninstallation unless forced
    if [ "$FORCE" = false ] && [ "$DRY_RUN" = false ]; then
        if ! confirm "Proceed with uninstalling Blaze VCS?"; then
            info "Uninstallation cancelled"
            exit 0
        fi
        echo
    fi

    # Remove binaries
    local removed_count=0
    local failed_count=0

    for installation in "${installations[@]}"; do
        # Apply filters
        if [ "$SYSTEM_ONLY" = true ] && [[ ! "$installation" =~ ^/usr/ ]]; then
            continue
        fi
        if [ "$USER_ONLY" = true ] && [[ "$installation" =~ ^/usr/ ]]; then
            continue
        fi

        if remove_binary "$installation" "$DRY_RUN"; then
            removed_count=$((removed_count + 1))
        else
            failed_count=$((failed_count + 1))
        fi
    done

    echo

    # Remove configuration files
    if [ "$KEEP_CONFIG" = false ]; then
        remove_config_files "$DRY_RUN"
        echo
    fi

    # Remove shell completions
    remove_shell_completions "$DRY_RUN"
    echo

    # Summary
    if [ "$DRY_RUN" = true ]; then
        info "Dry run completed - no files were actually removed"
        info "Run without --dry-run to perform the actual uninstallation"
    else
        if [ $removed_count -gt 0 ]; then
            success "Successfully removed $removed_count Blaze installation(s)"
        fi

        if [ $failed_count -gt 0 ]; then
            warn "Failed to remove $failed_count installation(s)"
        fi

        # Verify removal
        if verify_removal; then
            echo
            echo -e "${TRASH} ${GREEN}Blaze VCS has been successfully uninstalled!${NC}"
            echo
            echo -e "${INFO} ${BLUE}Thank you for trying Blaze VCS!${NC}"
            echo -e "${INFO} ${BLUE}Feedback and bug reports: https://github.com/blazevcs/blaze/issues${NC}"
        else
            echo
            warn "Uninstallation may be incomplete"
            warn "You may need to manually remove remaining files or restart your shell"
        fi
    fi
}

# Run main function with all arguments
main "$@"
