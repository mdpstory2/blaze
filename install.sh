#!/usr/bin/env bash
set -euo pipefail

# Blaze VCS Installer Script
# A blazingly fast, chunk-based version control system

BLAZE_VERSION="0.1.0"
BLAZE_REPO="mdpstory2/blaze"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="blaze"
TEMP_DIR=$(mktemp -d)
USER_INSTALL_DIR="$HOME/.local/bin"

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
CHECK="✅"
CROSS="❌"
INFO="ℹ️"
WARN="⚠️"

print_banner() {
    echo -e "${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}"
    echo -e "${FIRE}                                              ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}██${NC}      ${CYAN}█████${NC}  ${CYAN}███████${NC} ${CYAN}███████${NC}  ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}     ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}      ${CYAN}██${NC}       ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}██${NC}     ${CYAN}███████${NC} ${CYAN}███████${NC} ${CYAN}█████${NC}    ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}██${NC}     ${CYAN}██${NC}   ${CYAN}██${NC}      ${CYAN}██${NC} ${CYAN}██${NC}       ${FIRE}"
    echo -e "${FIRE}  ${CYAN}██████${NC}  ${CYAN}███████${NC} ${CYAN}██${NC}   ${CYAN}██${NC} ${CYAN}███████${NC} ${CYAN}███████${NC}  ${FIRE}"
    echo -e "${FIRE}                                              ${FIRE}"
    echo -e "${FIRE}  ${YELLOW}A blazingly fast version control system${NC}     ${FIRE}"
    echo -e "${FIRE}  ${PURPLE}Faster than Git • Easier to use • More reliable${NC} ${FIRE}"
    echo -e "${FIRE}                                              ${FIRE}"
    echo -e "${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}${FIRE}"
    echo ""
    echo -e "${INFO} Blaze VCS Installer v${BLAZE_VERSION}"
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

detect_os() {
    case "$(uname -s)" in
        Linux*)     MACHINE=Linux;;
        Darwin*)    MACHINE=Mac;;
        CYGWIN*)    MACHINE=Cygwin;;
        MINGW*)     MACHINE=MinGw;;
        *)          MACHINE="UNKNOWN:$(uname -s)"
    esac

    case "$(uname -m)" in
        x86_64)     ARCH=x64;;
        arm64)      ARCH=arm64;;
        aarch64)    ARCH=arm64;;
        armv7l)     ARCH=arm;;
        i386)       ARCH=x86;;
        i686)       ARCH=x86;;
        *)          ARCH="UNKNOWN:$(uname -m)"
    esac
}

detect_package_manager() {
    if command -v apt-get >/dev/null 2>&1; then
        PKG_MANAGER="apt"
        PKG_UPDATE="apt-get update"
        PKG_INSTALL="apt-get install -y"
        DEPS="curl tar build-essential pkg-config libsqlite3-dev libssl-dev"
    elif command -v yum >/dev/null 2>&1; then
        PKG_MANAGER="yum"
        PKG_UPDATE="yum update -y"
        PKG_INSTALL="yum install -y"
        DEPS="curl tar gcc gcc-c++ make pkgconfig sqlite-devel openssl-devel"
    elif command -v dnf >/dev/null 2>&1; then
        PKG_MANAGER="dnf"
        PKG_UPDATE="dnf update -y"
        PKG_INSTALL="dnf install -y"
        DEPS="curl tar gcc gcc-c++ make pkgconfig sqlite-devel openssl-devel"
    elif command -v pacman >/dev/null 2>&1; then
        PKG_MANAGER="pacman"
        PKG_UPDATE="pacman -Sy"
        PKG_INSTALL="pacman -S --noconfirm"
        DEPS="curl tar base-devel pkg-config sqlite openssl"
    elif command -v brew >/dev/null 2>&1; then
        PKG_MANAGER="brew"
        PKG_UPDATE="brew update"
        PKG_INSTALL="brew install"
        DEPS="curl gnu-tar pkg-config sqlite openssl"
    elif command -v apk >/dev/null 2>&1; then
        PKG_MANAGER="apk"
        PKG_UPDATE="apk update"
        PKG_INSTALL="apk add"
        DEPS="curl tar build-base pkgconfig sqlite-dev openssl-dev"
    else
        PKG_MANAGER="unknown"
    fi
}

install_system_dependencies() {
    info "Checking system build dependencies..."

    detect_package_manager

    if [ "$PKG_MANAGER" = "unknown" ]; then
        warn "Unknown package manager. Please ensure you have the following installed:"
        warn "  - curl, tar (basic tools)"
        warn "  - build-essential/gcc/clang (compiler)"
        warn "  - pkg-config (build configuration)"
        warn "  - sqlite3 development libraries"
        warn "  - openssl development libraries"
        return 0
    fi

    # Check if we need to install any dependencies
    local missing_deps=""

    case "$PKG_MANAGER" in
        "apt")
            for dep in curl tar build-essential pkg-config libsqlite3-dev libssl-dev; do
                if ! dpkg -l "$dep" >/dev/null 2>&1; then
                    missing_deps="$missing_deps $dep"
                fi
            done
            ;;
        "brew")
            for dep in curl gnu-tar pkg-config sqlite openssl; do
                if ! brew list "$dep" >/dev/null 2>&1; then
                    missing_deps="$missing_deps $dep"
                fi
            done
            ;;
        *)
            # For other package managers, we'll try to install anyway
            missing_deps="$DEPS"
            ;;
    esac

    if [ -n "$missing_deps" ]; then
        warn "Missing system dependencies:$missing_deps"
        read -p "Would you like to install them automatically? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            info "Installing system dependencies..."
            if [ "$PKG_MANAGER" = "brew" ]; then
                $PKG_UPDATE || warn "Failed to update package manager"
                for dep in $missing_deps; do
                    $PKG_INSTALL "$dep" || warn "Failed to install $dep"
                done
            else
                # For system package managers, try with sudo
                if [ "$EUID" -ne 0 ]; then
                    info "Installing with sudo..."
                    sudo $PKG_UPDATE || warn "Failed to update package manager"
                    sudo $PKG_INSTALL $missing_deps || warn "Failed to install some dependencies"
                else
                    $PKG_UPDATE || warn "Failed to update package manager"
                    $PKG_INSTALL $missing_deps || warn "Failed to install some dependencies"
                fi
            fi
            success "System dependencies installation completed"
        else
            warn "Skipping system dependency installation"
            warn "Note: Build from source may fail without these dependencies"
        fi
    else
        success "All system dependencies satisfied"
    fi
}

check_dependencies() {
    info "Checking basic dependencies..."

    if ! command -v curl >/dev/null 2>&1; then
        error_exit "curl is required but not installed. Please install curl first."
    fi

    if ! command -v tar >/dev/null 2>&1; then
        error_exit "tar is required but not installed. Please install tar first."
    fi

    success "Basic dependencies satisfied"
}

check_rust() {
    if command -v rustc >/dev/null 2>&1; then
        RUST_VERSION=$(rustc --version | awk '{print $2}')
        info "Found Rust $RUST_VERSION"
        return 0
    else
        return 1
    fi
}

install_rust() {
    warn "Rust not found. Blaze can be installed from source with Rust."
    read -p "Would you like to install Rust? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
        success "Rust installed successfully!"
        return 0
    else
        warn "Skipping Rust installation"
        return 1
    fi
}

get_latest_release() {
    if command -v curl >/dev/null 2>&1; then
        LATEST_RELEASE=$(curl -s "https://api.github.com/repos/$BLAZE_REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
        if [ -n "$LATEST_RELEASE" ]; then
            BLAZE_VERSION="$LATEST_RELEASE"
            info "Latest release: $BLAZE_VERSION"
        fi
    fi
}

install_from_binary() {
    info "Attempting to install from pre-built binary..."

    detect_os

    if [ "$MACHINE" = "UNKNOWN" ] || [ "$ARCH" = "UNKNOWN" ]; then
        warn "Unknown platform: $MACHINE-$ARCH"
        return 1
    fi

    # Construct download URL (adjust based on your release naming convention)
    BINARY_URL="https://github.com/$BLAZE_REPO/releases/download/$BLAZE_VERSION/blaze-$MACHINE-$ARCH"

    info "Downloading from: $BINARY_URL"

    # Try to download the binary
    if curl -L "$BINARY_URL" -o "$TEMP_DIR/$BINARY_NAME" 2>/dev/null; then
        chmod +x "$TEMP_DIR/$BINARY_NAME"
        success "Downloaded pre-built binary"
        return 0
    else
        warn "Pre-built binary not available for $MACHINE-$ARCH"
        return 1
    fi
}

install_from_source() {
    info "Installing from source..."

    if ! check_rust; then
        if ! install_rust; then
            error_exit "Rust is required to build from source"
        fi
    fi

    info "Cloning repository..."
    cd "$TEMP_DIR"

    if command -v git >/dev/null 2>&1; then
        git clone "https://github.com/$BLAZE_REPO.git" blaze
        cd blaze
    else
        # Fallback to downloading source archive
        curl -L "https://github.com/$BLAZE_REPO/archive/main.tar.gz" | tar -xz
        cd blaze-main
    fi

    info "Building Blaze (this may take a few minutes)..."
    if cargo build --release; then
        cp target/release/blaze "$TEMP_DIR/$BINARY_NAME"
        chmod +x "$TEMP_DIR/$BINARY_NAME"
        success "Built from source successfully"
        return 0
    else
        error_exit "Failed to build from source"
    fi
}

choose_install_dir() {
    info "Choosing installation directory..."

    # Check if we have write access to /usr/local/bin
    if [ -w "$INSTALL_DIR" ] || [ -w "$(dirname "$INSTALL_DIR")" ]; then
        CHOSEN_INSTALL_DIR="$INSTALL_DIR"
        info "Will install to system directory: $INSTALL_DIR"
    else
        # Try to create user local bin directory
        mkdir -p "$USER_INSTALL_DIR"
        CHOSEN_INSTALL_DIR="$USER_INSTALL_DIR"
        warn "No write access to $INSTALL_DIR"
        info "Will install to user directory: $USER_INSTALL_DIR"
    fi
}

install_binary() {
    choose_install_dir

    info "Installing blaze to $CHOSEN_INSTALL_DIR..."

    if cp "$TEMP_DIR/$BINARY_NAME" "$CHOSEN_INSTALL_DIR/$BINARY_NAME"; then
        success "Blaze installed successfully!"

        # Check if the installation directory is in PATH
        case ":$PATH:" in
            *":$CHOSEN_INSTALL_DIR:"*)
                success "Installation directory is already in PATH"
                ;;
            *)
                warn "Installation directory $CHOSEN_INSTALL_DIR is not in your PATH"
                info "Add this line to your shell configuration file (~/.bashrc, ~/.zshrc, etc.):"
                echo "    export PATH=\"$CHOSEN_INSTALL_DIR:\$PATH\""
                ;;
        esac

        return 0
    else
        if [ "$CHOSEN_INSTALL_DIR" = "$INSTALL_DIR" ]; then
            warn "Failed to install to $INSTALL_DIR, trying with sudo..."
            if sudo cp "$TEMP_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"; then
                success "Blaze installed successfully with sudo!"
                return 0
            fi
        fi
        error_exit "Failed to install binary"
    fi
}

verify_installation() {
    info "Verifying installation..."

    if command -v blaze >/dev/null 2>&1; then
        VERSION_OUTPUT=$(blaze --version 2>/dev/null || echo "version check failed")
        success "Blaze is installed and working!"
        info "Version: $VERSION_OUTPUT"

        echo ""
        echo -e "${ROCKET} ${GREEN}Installation complete!${NC}"
        echo ""
        echo -e "${INFO} Try these commands to get started:"
        echo -e "  ${CYAN}blaze init${NC}          # Initialize a new repository"
        echo -e "  ${CYAN}blaze add .${NC}         # Add files to staging"
        echo -e "  ${CYAN}blaze commit -m \"Initial commit\"${NC}  # Create your first commit"
        echo -e "  ${CYAN}blaze status${NC}        # Check repository status"
        echo -e "  ${CYAN}blaze --help${NC}        # Show all available commands"
        echo ""
        echo -e "${INFO} Documentation: https://github.com/$BLAZE_REPO"
        return 0
    else
        error_exit "Installation verification failed. Blaze command not found."
    fi
}

cleanup() {
    info "Cleaning up temporary files..."
    rm -rf "$TEMP_DIR"
}

show_help() {
    echo "Blaze VCS Installer"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --help              Show this help message"
    echo "  --source            Force installation from source"
    echo "  --version VERSION   Install specific version"
    echo "  --dir DIR           Install to specific directory"
    echo ""
    echo "Examples:"
    echo "  $0                  # Install latest version automatically"
    echo "  $0 --source         # Force build from source"
    echo "  $0 --dir ~/.local/bin  # Install to specific directory"
    echo ""
}

main() {
    local FORCE_SOURCE=false
    local CUSTOM_INSTALL_DIR=""

    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --help|-h)
                show_help
                exit 0
                ;;
            --source)
                FORCE_SOURCE=true
                shift
                ;;
            --version)
                BLAZE_VERSION="$2"
                shift 2
                ;;
            --dir)
                CUSTOM_INSTALL_DIR="$2"
                shift 2
                ;;
            *)
                error_exit "Unknown option: $1"
                ;;
        esac
    done

    print_banner

    # Set custom install directory if provided
    if [ -n "$CUSTOM_INSTALL_DIR" ]; then
        INSTALL_DIR="$CUSTOM_INSTALL_DIR"
        USER_INSTALL_DIR="$CUSTOM_INSTALL_DIR"
    fi

    check_dependencies

    # Try to get latest release version
    if [ "$BLAZE_VERSION" = "0.1.0" ]; then
        get_latest_release
    fi

    # Installation strategy
    if [ "$FORCE_SOURCE" = true ]; then
        info "Forcing installation from source..."
        install_system_dependencies
        install_from_source || error_exit "Source installation failed"
    else
        # Try binary first, fallback to source
        if ! install_from_binary; then
            warn "Binary installation failed, trying source installation..."
            install_system_dependencies
            install_from_source || error_exit "All installation methods failed"
        fi
    fi

    install_binary
    verify_installation
    cleanup

    echo ""
    echo -e "${FIRE} ${GREEN}Welcome to Blaze - the future of version control!${NC} ${FIRE}"
    echo ""
}

# Trap to ensure cleanup on exit
trap cleanup EXIT

# Run main function with all arguments
main "$@"
