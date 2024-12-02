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
    if [ -n "$TEST_DIR" ]; then
        echo -e "${ORANGE}Cleaning up test directory...${NC}"
        rm -rf "$TEST_DIR"
    fi
}

trap cleanup EXIT

# Create test directory
TEST_DIR=$(mktemp -d)
echo -e "${ORANGE}Setting up test in $TEST_DIR${NC}"

# Create .nexus directory structure
mkdir -p "$TEST_DIR/.nexus/bin"

# Write the old version to .current_version
echo -e "${ORANGE}Setting initial version to $OLD_VERSION${NC}"
echo "$OLD_VERSION" > "$TEST_DIR/.current_version"

# Run prover with test environment
echo -e "${ORANGE}Running prover with version check...${NC}"
cd clients/cli  # Change to CLI directory
NEXUS_HOME="$TEST_DIR" cargo run -- \
    $ORCHESTRATOR_HOST --updater-mode test > /dev/null 2>&1 &

INITIAL_PID=$!
echo -e "${ORANGE}Initial process PID: $INITIAL_PID${NC}"

# Wait for update to happen
echo -e "${ORANGE}Waiting for update check (30s)...${NC}"
sleep 30

# Check if new version was downloaded
if [ -f "$TEST_DIR/.current_version" ] && [ "$(cat "$TEST_DIR/.current_version")" = "$EXPECTED_NEW_VERSION" ]; then
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