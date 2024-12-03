#!/bin/bash

# package-releases.sh
#
# This script automates the process of building and packaging the Nexus CLI for distribution
# across multiple platforms. It:
#
# 1. Checks and installs required Rust compilation targets if missing
# 2. Builds optimized release binaries for:
#    - Apple Silicon (M1/M2) Mac     (aarch64-apple-darwin)
#    - Intel Mac                     (x86_64-apple-darwin)
#    - Linux x86_64                  (x86_64-unknown-linux-gnu)
#    - Linux ARM64                   (aarch64-unknown-linux-gnu)
#    - Windows x86_64                (x86_64-pc-windows-msvc)
# 3. Creates compressed tarballs for each platform
# 4. Collects all tarballs into a dist/ directory
#
# Usage:
#   ./scripts/package-releases.sh
#
# Output:
#   Creates a dist/ directory containing platform-specific tarballs
#   Each tarball contains the prover binary for that platform

# Exit on any error
set -e

# Colors for output
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# target architectures 
TARGETS=(
    "aarch64-apple-darwin"
    "x86_64-apple-darwin"
)

# Check for required tools
check_dependencies() {
    echo -e "${GREEN}Checking dependencies...${NC}"
    
    # Check if running on macOS
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # Check for Homebrew
        if ! command -v brew >/dev/null 2>&1; then
            echo "Homebrew is required but not installed. Please install it first:"
            echo "/bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
            exit 1
        fi
        
        # Check for musl-cross
        if ! brew list musl-cross >/dev/null 2>&1; then
            echo "Installing musl-cross for Linux cross-compilation..."
            brew install FiloSottile/musl-cross/musl-cross
        fi
        
        # Check for OpenSSL
        if ! brew list openssl@3 >/dev/null 2>&1; then
            echo "Installing OpenSSL..."
            brew install openssl@3
        fi
    fi
}

# Function to check if target is installed
check_target() {
    local target=$1
    if ! rustup target list | grep -q "$target installed"; then
        echo -e "${GREEN}Installing target $target...${NC}"
        rustup target add "$target"
    fi
}

# Function to build and package for a target
build_target() {
    local target=$1
    echo -e "\n${GREEN}Building for $target...${NC}"
    
    # Use cross for Linux targets, cargo for others
    if [[ "$target" == *"linux"* ]]; then
        # Install cross if not already installed
        if ! command -v cross >/dev/null 2>&1; then
            echo -e "${GREEN}Installing cross...${NC}"
            cargo install cross
        fi
        
        # Build using cross
        cross build --release --target "$target"
    else
        # Build normally for non-Linux targets
        RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target "$target"
    fi
    
    # Package
    local binary_name="prover"
    if [[ "$target" == *"windows"* ]]; then
        binary_name="prover.exe"
    fi
    
    cd "target/$target/release"
    echo -e "${GREEN}Creating tarball for $target...${NC}"
    tar -czf "$target.tar.gz" "./$binary_name"
    cd - > /dev/null
}

# Main execution
echo -e "${GREEN}Starting build process...${NC}"

# Check dependencies
check_dependencies

# Check and install targets
for target in "${TARGETS[@]}"; do
    check_target "$target"
done

# Build and package for each target
for target in "${TARGETS[@]}"; do
    build_target "$target"
done

# Create output directory and collect tarballs
echo -e "\n${GREEN}Collecting tarballs...${NC}"
mkdir -p dist
for target in "${TARGETS[@]}"; do
    cp "target/$target/release/$target.tar.gz" dist/
done

echo -e "\n${GREEN}Build complete! Tarballs are in the dist/ directory${NC}"
ls -lh dist/

# Clean up
rm -rf dist/


