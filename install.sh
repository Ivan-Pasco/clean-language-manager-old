#!/bin/bash

# Clean Language Manager Installer
# This script downloads and installs the latest version of cleen

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO="Ivan-Pasco/clean-language-manager-old"
BINARY_NAME="cleen"
INSTALL_DIR="$HOME/.local/bin"

# Detect platform
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)
    
    case "$os" in
        linux*)
            case "$arch" in
                x86_64|amd64) echo "linux-x86_64" ;;
                *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        darwin*)
            case "$arch" in
                x86_64) echo "macos-x86_64" ;;
                arm64) echo "macos-aarch64" ;;
                *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        mingw*|cygwin*|msys*)
            case "$arch" in
                x86_64|amd64) echo "windows-x86_64" ;;
                *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        *)
            echo "Unsupported operating system: $os" >&2
            exit 1
            ;;
    esac
}

# Get latest release version
get_latest_version() {
    curl -s "https://api.github.com/repos/$REPO/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"([^"]+)".*/\1/'
}

# Detect user's shell and config file
detect_shell_config() {
    local shell_name=$(basename "$SHELL")
    
    case "$shell_name" in
        zsh)
            echo "$HOME/.zshrc"
            ;;
        bash)
            # Check which bash config file exists or should be used
            if [[ "$OSTYPE" == "darwin"* ]]; then
                # macOS: prefer .bash_profile
                if [[ -f "$HOME/.bash_profile" ]]; then
                    echo "$HOME/.bash_profile"
                else
                    echo "$HOME/.bash_profile"  # Create it if it doesn't exist
                fi
            else
                # Linux: prefer .bashrc
                if [[ -f "$HOME/.bashrc" ]]; then
                    echo "$HOME/.bashrc"
                else
                    echo "$HOME/.bashrc"  # Create it if it doesn't exist
                fi
            fi
            ;;
        fish)
            mkdir -p "$HOME/.config/fish"
            echo "$HOME/.config/fish/config.fish"
            ;;
        *)
            # Default to bashrc for unknown shells
            echo "$HOME/.bashrc"
            ;;
    esac
}

# Add directory to PATH in shell config
add_to_path() {
    local config_file="$1"
    local path_export="export PATH=\"$INSTALL_DIR:\$PATH\""
    
    # Check if the PATH export already exists
    if [[ -f "$config_file" ]] && grep -q "PATH.*$INSTALL_DIR" "$config_file"; then
        echo -e "${YELLOW}⚠️  PATH already configured in $config_file${NC}"
        return 0
    fi
    
    echo -e "${YELLOW}Adding $INSTALL_DIR to PATH in $config_file${NC}"
    
    # Create config file if it doesn't exist
    touch "$config_file"
    
    # Add a comment and the PATH export
    {
        echo ""
        echo "# Added by Clean Language Manager installer"
        echo "$path_export"
    } >> "$config_file"
    
    echo -e "${GREEN}✅ PATH updated in $config_file${NC}"
    return 0
}

# Download and extract binary
install_cleen() {
    local platform=$(detect_platform)
    local version=$(get_latest_version)
    
    if [ -z "$version" ]; then
        echo -e "${RED}Error: Could not fetch latest version${NC}" >&2
        exit 1
    fi
    
    echo -e "${BLUE}Clean Language Manager Installer${NC}"
    echo -e "${BLUE}=================================${NC}"
    echo
    echo -e "Platform: ${GREEN}$platform${NC}"
    echo -e "Version:  ${GREEN}$version${NC}"
    echo -e "Install:  ${GREEN}$INSTALL_DIR${NC}"
    echo
    
    # Determine archive format and binary extension
    local archive_ext="tar.gz"
    local binary_ext=""
    if [[ "$platform" == *"windows"* ]]; then
        archive_ext="zip"
        binary_ext=".exe"
    fi
    
    local archive_name="${BINARY_NAME}-${platform}.${archive_ext}"
    local download_url="https://github.com/$REPO/releases/download/$version/$archive_name"
    
    echo -e "${YELLOW}Downloading cleen...${NC}"
    echo "URL: $download_url"
    
    # Create temporary directory
    local temp_dir=$(mktemp -d)
    cd "$temp_dir"
    
    # Download the archive
    if ! curl -L -o "$archive_name" "$download_url"; then
        echo -e "${RED}Error: Failed to download cleen${NC}" >&2
        rm -rf "$temp_dir"
        exit 1
    fi
    
    echo -e "${YELLOW}Extracting archive...${NC}"
    
    # Extract based on archive type
    if [[ "$archive_ext" == "tar.gz" ]]; then
        tar -xzf "$archive_name"
    else
        unzip -q "$archive_name"
    fi
    
    # Create install directory
    mkdir -p "$INSTALL_DIR"
    
    # Move binary to install directory
    local binary_path="${BINARY_NAME}${binary_ext}"
    if [ ! -f "$binary_path" ]; then
        echo -e "${RED}Error: Binary not found in archive${NC}" >&2
        rm -rf "$temp_dir"
        exit 1
    fi
    
    echo -e "${YELLOW}Installing to $INSTALL_DIR...${NC}"
    cp "$binary_path" "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
    
    # Cleanup
    rm -rf "$temp_dir"
    
    echo -e "${GREEN}✅ Clean Language Manager installed successfully!${NC}"
    echo
    
    # Check if install directory is in PATH
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo -e "${YELLOW}⚠️  Setting up PATH configuration...${NC}"
        echo
        
        # Detect shell config file
        local config_file=$(detect_shell_config)
        local shell_name=$(basename "$SHELL")
        
        echo -e "Detected shell: ${BLUE}$shell_name${NC}"
        echo -e "Config file: ${BLUE}$config_file${NC}"
        echo
        
        # Add to PATH
        if add_to_path "$config_file"; then
            echo
            echo -e "${GREEN}✅ PATH configured successfully!${NC}"
            echo
            echo -e "${YELLOW}To use cleen immediately, run:${NC}"
            echo -e "  ${BLUE}source $config_file${NC}"
            echo -e "${YELLOW}Or restart your terminal${NC}"
            echo
            echo -e "${YELLOW}You can also run cleen directly:${NC}"
            echo -e "  ${BLUE}$INSTALL_DIR/cleen --help${NC}"
        else
            echo -e "${RED}Failed to configure PATH automatically${NC}"
            echo
            echo "Please add the following line to your shell configuration file:"
            echo -e "  ${BLUE}export PATH=\"$INSTALL_DIR:\$PATH\"${NC}"
            echo
            echo "Shell configuration files:"
            echo "  - Bash: ~/.bashrc or ~/.bash_profile"
            echo "  - Zsh: ~/.zshrc" 
            echo "  - Fish: ~/.config/fish/config.fish"
        fi
    else
        echo -e "${GREEN}✅ Installation directory is already in your PATH${NC}"
        echo
        echo "You can now run:"
        echo -e "  ${BLUE}cleen --help${NC}"
    fi
    
    echo
    echo -e "${BLUE}Next steps:${NC}"
    echo "1. Run: ${BLUE}cleen init${NC}"
    echo "2. Run: ${BLUE}cleen doctor${NC}"
    echo "3. Install a Clean Language version: ${BLUE}cleen install <version>${NC}"
    echo
    echo "For more information: ${BLUE}https://github.com/$REPO${NC}"
}

# Main execution
main() {
    # Check dependencies
    for cmd in curl tar; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            echo -e "${RED}Error: $cmd is required but not installed${NC}" >&2
            exit 1
        fi
    done
    
    # Check for unzip on systems that might need it
    if [[ "$(uname -s)" =~ ^(MINGW|CYGWIN|MSYS) ]] && ! command -v unzip >/dev/null 2>&1; then
        echo -e "${RED}Error: unzip is required but not installed${NC}" >&2
        exit 1
    fi
    
    install_cleen
}

# Run the installer
main "$@"