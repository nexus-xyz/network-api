#!/bin/bash

# Test Auto-updater Functionality
# 
# This script tests the CLI's auto-update mechanism by:
# 1. Setting up a clean test environment
# 2. Starting CLI with initial version
# 3. Simulating a new version release
# 4. Verifying update and process handoff
# 5. Cleaning up processes and test files
#
# Usage: ./test_updater.sh
# Requires: git, cargo
# Cleanup: Automatically kills processes and removes test directory on exit
#
# Note: Uses trap to ensure cleanup even if script is interrupted

set -e  # Exit on any error

# Configuration
ORCHESTRATOR_HOST="beta.orchestrator.nexus.xyz"
# The new version number used to test the updater
TEST_NEW_VERSION="0.9.9" 

# Variables used for pretty printing in colors
ORANGE='\033[1;33m'
NC='\033[0m' # No Color

# Find project root (assuming script is in tests/)
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Setup cleanup trap
cleanup() {
    echo -e "${ORANGE}[test-updater script] (14 / 18) Cleaning up...${NC}"

    if [ -n "$NEW_PID" ]; then
        # Kill the new prover process if it exists
        echo -e "${ORANGE}[test-updater script] (15 / 18) Killing newest prover process...${NC}"
        kill $NEW_PID 2>/dev/null || true
    fi
    if [ -n "$ORIGINAL_PID" ]; then
        # Kill the original prover process if it exists
        echo -e "${ORANGE}[test-updater script] (16 / 18) Killing original prover process...${NC}"
        kill $ORIGINAL_PID 2>/dev/null || true
    fi
    # Wait a moment for processes to terminate
    echo -e "${ORANGE}[test-updater script] (17 / 18) Waiting for processes to terminate...${NC}"
    sleep 2
    # Remove test directory
    if [ -n "$TEST_DIR" ]; then
        echo -e "${ORANGE}[test-updater script] (18 / 18) Removing test directory...${NC}"
        rm -rf "$TEST_DIR"
    fi
    # Return to original directory
    cd "$PROJECT_ROOT"
    echo "Cleanup complete"
    exit 0
}

# Trap cleanup on script exit, interrupts (Ctrl+C), and termination
# This ensures that the cleanup is called even if the script is interrupted or terminated
trap cleanup EXIT
trap cleanup INT
trap cleanup TERM

# Create clean test directory
TEST_DIR=$(mktemp -d)
echo " "
echo -e "${ORANGE}[test-updater script] (1 / 18) Starting test for auto-updater...${NC}"
echo -e "${ORANGE}[test-updater script] (2 / 18) Setting up test directory in $TEST_DIR${NC}"
echo -e "${ORANGE}[test-updater script] (3 / 18) Setting up git and files...${NC}"

echo " "


# Copy your local files to test directory
cd $PROJECT_ROOT
cp -r . $TEST_DIR/
cd $TEST_DIR

# Remove existing .git and start fresh
rm -rf .git
git init
git add .
git commit -m "Initial commit"
git tag 0.3.5  # Start with old version

# Build and start the CLI
cd clients/cli
echo " "
echo -e "${ORANGE}[test-updater script] (4 / 18) Building project from local source code (no git pull) with cargo...${NC}"
CARGO_CMD="cargo build --release"
$CARGO_CMD || exit 1

INSTALL_PATH="$TEST_DIR/clients/cli/target/release/prover"
echo -e "${ORANGE}[test-updater script] (5 / 18) Binary path: $INSTALL_PATH ${NC}"

# Start CLI and store its PID in the memory of this bash script 
# note: the PID is ALSO stored in the .prover.pid file by the updater.rs, but this one is just for in-memory testing/validating
echo " "
echo -e "${ORANGE}[test-updater script] (6 / 18) Starting CLI v1.0...${NC}"
echo " "
STARTING_COMMIT=$(git rev-parse HEAD)
$INSTALL_PATH --updater-mode test $ORCHESTRATOR_HOST & # Start CLI with updater mode test
ORIGINAL_PID=$!
echo -e "${ORANGE}[test-updater script] (7 / 18) Original PID for the CLI main process: $ORIGINAL_PID${NC}"
echo " "

# Give CLI some time to start the proving by starting the main thread at prover.rs
sleep 30 

# Create new version with higher number than 0.3.5
# This section represents what may happen in the wild: the code is updated on github with a new tag
echo " "
echo -e "${ORANGE}[test-updater script] (8 / 18) Adding new code to test auto-update...${NC}"
echo "updated" > test.txt
git add test.txt
git commit -m "Update"
git tag $TEST_NEW_VERSION # Use a version higher than current
echo -e "${ORANGE}[test-updater script] new code added and committed. New tag version: $TEST_NEW_VERSION${NC}"

# Wait for auto-update to happen
echo -e "${ORANGE}[test-updater script] (9 / 18) Waiting 60 seconds for auto-update to catch the new version...${NC}"
echo " "
sleep 60  # Give the updater time to detect and apply update (it checks every 20 seconds)
echo -e "${ORANGE}[test-updater script] (10 / 18) Checking if the updater applied the update...${NC}"
echo " "

# { During this time, the updater should have updated the code and restarted with a new process }

# The updater should have written the new version to the file
NEW_VERSION=$(cat .current_version)
echo -e "${ORANGE}[test-updater script] New version: $NEW_VERSION${NC}"


# Verify that the new version is running in a new process (e.g. CLI restarted)
NEW_PID="$(cat .prover.pid 2>/dev/null || echo "")"  # Read PID from file
echo -e "${ORANGE}[test-updater script] New PID: $NEW_PID${NC}"

# if the new PID is empty, the CLI is not running
if [ -z "$NEW_PID" ] || ! ps -p "$NEW_PID" > /dev/null; then
    echo -e "${ORANGE}❌ CLI is not running!${NC}"
    exit 1
fi

# If the new PID is the same as the original PID, the CLI was not restarted (same process)
if [ "$NEW_PID" == "$ORIGINAL_PID" ]; then
    echo -e "${ORANGE}[test-updater script] (12 / 18) ❌ CLI was not restarted \(PID unchanged\)${NC}"
    echo -e "${ORANGE}[test-updater script] (13 / 18) Original version: $(git describe --tags $STARTING_COMMIT)${NC}. Expected version: $TEST_NEW_VERSION${NC}"
    exit 1
fi

echo -e "${ORANGE}[test-updater script] (14 / 18) ✅ CLI auto-updated and restarted successfully${NC}"
