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

# Install old version
echo -e "${ORANGE}Installing version $OLD_VERSION for testing...${NC}"
mkdir -p "$TEST_DIR/.nexus/bin"

# Download old version from GitHub releases
curl -L "https://github.com/nexus-xyz/network-api/releases/download/$OLD_VERSION/prover" \
    -o "$TEST_DIR/.nexus/bin/prover"
chmod +x "$TEST_DIR/.nexus/bin/prover"

# Set initial version
echo "$OLD_VERSION" > "$TEST_DIR/.current_version"

# Run prover with test environment
echo -e "${ORANGE}Running prover with version check...${NC}"
NEXUS_HOME="$TEST_DIR" "$TEST_DIR/.nexus/bin/prover" \
    $ORCHESTRATOR_HOST --updater-mode test > output.txt &

PROVER_PID=$!

# Wait for update to happen
echo -e "${ORANGE}Waiting for update check (30s)...${NC}"
sleep 30

# Check if new version was downloaded
if [ -f "$TEST_DIR/.current_version" ] && [ "$(cat "$TEST_DIR/.current_version")" = "$EXPECTED_NEW_VERSION" ]; then
    echo -e "${ORANGE}✅ Update successful - detected new version${NC}"
else
    echo -e "${ORANGE}❌ Update failed - version not updated${NC}"
    cat output.txt
    exit 1
fi

echo -e "${ORANGE}All tests passed!${NC}" 