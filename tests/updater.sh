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
}

trap cleanup EXIT INT TERM

# Create test directory
TEST_DIR=$(mktemp -d)
echo -e "${ORANGE}Setting up test in $TEST_DIR${NC}"

# Create .nexus directory structure
mkdir -p "$TEST_DIR/.nexus/bin"

# Build the source code
echo -e "${ORANGE}Building source code...${NC}"
cd clients/cli  # Change to CLI directory

# Run prover with test environment
echo -e "${ORANGE}Running prover with version check...${NC}"
NEXUS_HOME="$TEST_DIR" cargo run -- $ORCHESTRATOR_HOST &

INITIAL_PID=$!
echo -e "${ORANGE}Initial process PID: $INITIAL_PID${NC}"

# Wait a moment for the process to start
sleep 5

# Wait for update to happen
echo -e "${ORANGE}Waiting for update check (60s)...${NC}"
sleep 60

# Update version check location
if [ -f "$TEST_DIR/.nexus/bin/version" ] && [ "$(cat "$TEST_DIR/.nexus/bin/version")" = "$EXPECTED_NEW_VERSION" ]; then
    echo -e "${ORANGE}✅ Version file updated successfully${NC}"
else
    echo -e "${ORANGE}❌ Version file not updated${NC}"
    exit 1
fi

# Check if binary exists and is executable
if [ -x "$TEST_DIR/.nexus/bin/prover" ]; then
    echo -e "${ORANGE}✅ Binary downloaded and executable${NC}"
else
    echo -e "${ORANGE}❌ Binary not found or not executable${NC}"
    ls -l "$TEST_DIR/.nexus/bin"
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
