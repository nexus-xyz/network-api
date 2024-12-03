#!/bin/bash

set -e  # Exit on any error

# Configuration
ORCHESTRATOR_HOST="beta.orchestrator.nexus.xyz"
OLD_VERSION="0.3.6"
EXPECTED_NEW_VERSION="0.3.7"
TEST_DIR=""

# Colors for output
ORANGE='\033[1;33m'
NC='\033[0m' # No Color

cleanup() {
    # Kill any running prover processes started by this test
    if [ -n "$INITIAL_PID" ]; then
        echo -e "${ORANGE}Killing prover process...${NC}"
        pkill -P $INITIAL_PID 2>/dev/null || true
        kill $INITIAL_PID 2>/dev/null || true
    fi

    # Clean up test directory
    if [ -n "$TEST_DIR" ]; then
        echo -e "${ORANGE}Cleaning up test directory...${NC}"
        rm -rf "$TEST_DIR"
    fi

    # Clean up any existing version files in the development directory
    echo -e "${ORANGE}Cleaning up version files...${NC}"
    rm -f "$(pwd)/.nexus/bin/version" 2>/dev/null || true
    rm -f ".nexus/bin/version" 2>/dev/null || true
}

trap cleanup EXIT INT TERM

# Create test directory and setup
TEST_DIR=$(mktemp -d)
echo -e "${ORANGE}Setting up test in $TEST_DIR${NC}"

# Create .nexus directory structure
mkdir -p "$TEST_DIR/.nexus/bin"
echo -e "${ORANGE}Created directory structure: $TEST_DIR/.nexus/bin${NC}"

# Force the initial version
echo -e "${ORANGE}Setting initial version $OLD_VERSION${NC}"
echo "$OLD_VERSION" > "$TEST_DIR/.nexus/bin/version"

# Build the source code
echo -e "${ORANGE}Building source code...${NC}"
cd clients/cli  # Change to CLI directory

# Build and run the binary
if command -v cargo >/dev/null 2>&1; then
    CARGO_PATH=$(which cargo)
    echo -e "${ORANGE}Using cargo at: $CARGO_PATH${NC}"
    
    # Build the binary
    $CARGO_PATH build || {
        echo -e "${ORANGE}❌ Failed to build binary${NC}"
        exit 1
    }
    
    # Get the path to the built binary
    BINARY_PATH="$(pwd)/target/debug/prover"
    echo -e "${ORANGE}Binary built at: $BINARY_PATH${NC}"
    
    # Run the binary directly with modified environment
    echo -e "${ORANGE}Running binary with test environment...${NC}"
    HOME="$TEST_DIR" NEXUS_HOME="$TEST_DIR" \
        $BINARY_PATH $ORCHESTRATOR_HOST &
else
    echo -e "${ORANGE}❌ Cargo not found in PATH${NC}"
    exit 1
fi

INITIAL_PID=$!
echo -e "${ORANGE}Initial process PID: $INITIAL_PID${NC}"

# Wait a moment for the process to start
sleep 5

# Wait for update to happen and verify binary download
echo -e "${ORANGE}Waiting for update check (60s)...${NC}"
sleep 60

# Update version check location
echo -e "${ORANGE}Checking version file at: $TEST_DIR/.nexus/bin/version${NC}"

if [ -f "$TEST_DIR/.nexus/bin/version" ]; then
    ACTUAL_VERSION=$(cat "$TEST_DIR/.nexus/bin/version")
    echo -e "${ORANGE}Found version file. Content: '$ACTUAL_VERSION'${NC}"
    
    if [ "$ACTUAL_VERSION" = "$EXPECTED_NEW_VERSION" ]; then
        echo -e "${ORANGE}✅  successfully${NC}"
    else
        echo -e "${ORANGE}❌ Version mismatch - Expected: '$EXPECTED_NEW_VERSION', Got: '$ACTUAL_VERSION'${NC}"
        exit 1
    fi
else
    echo -e "${ORANGE}❌ Version file not found at: $TEST_DIR/.nexus/bin/version${NC}"
    echo -e "${ORANGE}Directory contents:${NC}"
    ls -la "$TEST_DIR/.nexus/bin"
    exit 1
fi

# Check if binary exists and is executable
echo -e "${ORANGE}Checking binary at: $TEST_DIR/.nexus/bin/prover${NC}"
echo -e "${ORANGE}Directory contents (test dir):${NC}"
ls -la "$TEST_DIR/.nexus/bin"

echo -e "${ORANGE}Checking binary at: $HOME/.nexus/bin/prover${NC}"
echo -e "${ORANGE}Directory contents (home dir):${NC}"
ls -la "$HOME/.nexus/bin" 2>/dev/null || echo "Home .nexus/bin directory not found"

if [ -x "$TEST_DIR/.nexus/bin/prover" ]; then
    echo -e "${ORANGE}✅ Binary downloaded and executable (in test dir)${NC}"
elif [ -x "$HOME/.nexus/bin/prover" ]; then
    echo -e "${ORANGE}✅ Binary downloaded and executable (in home dir)${NC}"
    echo -e "${ORANGE}Moving binary to test directory...${NC}"
    cp "$HOME/.nexus/bin/prover" "$TEST_DIR/.nexus/bin/prover"
    chmod +x "$TEST_DIR/.nexus/bin/prover"
else
    echo -e "${ORANGE}❌ Binary not found or not executable in either location${NC}"
    echo -e "${ORANGE}Full directory listing (test dir):${NC}"
    find "$TEST_DIR/.nexus" -type f -ls
    echo -e "${ORANGE}Full directory listing (home dir):${NC}"
    find "$HOME/.nexus" -type f -ls 2>/dev/null || echo "No files found in home .nexus directory"
    exit 1
fi

# Check if original process was replaced
if ps -p $INITIAL_PID > /dev/null; then
    echo -e "${ORANGE}❌ Original process still running - should have been replaced${NC}"
    exit 1
else
    echo -e "${ORANGE}✅ Original process replaced${NC}"
fi

# Check if new binary is running
NEW_PID=$(pgrep -f "$TEST_DIR/.nexus/bin/prover")
if [ -n "$NEW_PID" ]; then
    echo -e "${ORANGE}✅ New binary is running with PID: $NEW_PID${NC}"
else
    echo -e "${ORANGE}❌ New binary not running${NC}"
    exit 1
fi

echo -e "${ORANGE}All tests passed!${NC}"

# Clean up Nexus installation
echo -e "${ORANGE}Cleaning up Nexus installation...${NC}"
