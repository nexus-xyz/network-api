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
GREEN='\033[0;32m'
BLUE='\033[0;34m'

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
echo -e "${ORANGE}1. Setting up test in $TEST_DIR${NC}\n"

# Create .nexus directory structure
mkdir -p "$TEST_DIR/.nexus/bin"
echo -e "${ORANGE}2. Created directory structure: $TEST_DIR/.nexus/bin${NC}\n"

# Force the initial version
echo -e "${ORANGE}3. Setting initial version $OLD_VERSION${NC}\n"
echo "$OLD_VERSION" > "$TEST_DIR/.nexus/bin/version"

# Build the source code
echo -e "${ORANGE}4. Building source code...${NC}\n"
cd clients/cli  # Change to CLI directory

# Build and run the binary
if command -v cargo >/dev/null 2>&1; then
    CARGO_PATH=$(which cargo)
    echo -e "${ORANGE}\t\t Using cargo at: $CARGO_PATH${NC}\n"
    
    # Build the binary
    $CARGO_PATH build || {
        echo -e "${ORANGE}❌ Failed to build binary${NC}\n"
        exit 1
    }
    
    # Get the path to the built binary
    BINARY_PATH="$(pwd)/target/debug/prover"
    echo -e "${ORANGE}5. Binary built at: $BINARY_PATH${NC}"
    
    # Run the binary directly with modified environment
    echo -e "${ORANGE}6. Running binary with test environment...${NC}\n"
    HOME="$TEST_DIR" NEXUS_HOME="$TEST_DIR" \
        $BINARY_PATH $ORCHESTRATOR_HOST &
else
    echo -e "${ORANGE}❌ Cargo not found in PATH${NC}"
    exit 1
fi

INITIAL_PID=$!
echo -e "${ORANGE}7. Initial process PID: $INITIAL_PID${NC}\n"

echo -e "${ORANGE}Test will pass if you see messages change from ${GREEN}[auto-updater]${ORANGE} to ${BLUE}[auto-updater]${ORANGE} color${NC}"
echo -e "${ORANGE}This color change indicates the binary was successfully replaced${NC}"


# Wait a moment for the process to start
sleep 5

# (During this time the process starts)

# Wait for update to happen and verify binary download
echo -e "${ORANGE}\nWaiting for update check (30s)...${NC}"
sleep 30

# Update version check location
echo -e "${ORANGE}Checking binary version${NC}"

echo -e "${ORANGE}DID YOU SEE THE COLOR CHANGE?${NC}"
