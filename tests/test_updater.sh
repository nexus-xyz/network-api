#!/bin/bash
set -e  # Exit on any error

# Configuration
ORCHESTRATOR_HOST="beta.orchestrator.nexus.xyz"
TEST_NEW_VERSION="0.9.9"  # Define version once here

# Add at top after shebang
ORANGE='\033[1;33m'
NC='\033[0m' # No Color

# Find project root (assuming script is in tests/)
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Setup cleanup trap
cleanup() {
    echo -e "\nCleaning up..."
    if [ -n "$INSTALL_PATH" ]; then  # Only if INSTALL_PATH is not empty
        # Kill any running prover processes from our test
        pkill -f "$INSTALL_PATH" || true
    fi
    # Remove test directory
    if [ -n "$TEST_DIR" ]; then      # Only if TEST_DIR is not empty
        rm -rf "$TEST_DIR"
    fi
    # Return to original directory
    cd "$PROJECT_ROOT"
    echo "Cleanup complete"
    exit 0
}

# Trap cleanup on script exit, interrupts (Ctrl+C), and termination
trap cleanup EXIT
trap cleanup INT
trap cleanup TERM

# Create clean test directory
TEST_DIR=$(mktemp -d)
echo " "
echo -e "${ORANGE}[test-updater] Starting test... for auto-updater${NC}"
echo -e "${ORANGE}[test-updater] Setting up test directory in $TEST_DIR${NC}"

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

# Build and start CLI
cd clients/cli
cargo build --release
INSTALL_PATH="$TEST_DIR/clients/cli/target/release/prover"

# Start CLI and store its PID
echo -e "${ORANGE}Starting CLI v1.0...${NC}"
STARTING_COMMIT=$(git rev-parse HEAD)
$INSTALL_PATH $ORCHESTRATOR_HOST &
ORIGINAL_PID=$!
echo -e "${ORANGE}[test-updater]Original PID: $ORIGINAL_PID${NC}"

# Give CLI time to start the proving from prover.rs
sleep 30 

# Create new version with higher number than 0.3.5
# This section represents what may happen in the wild: the code is updated on github with a new tag
echo -e "${ORANGE}[test-updater] Adding new code to test auto-update...${NC}"
echo "updated" > test.txt
git add test.txt
git commit -m "Update"
git tag $TEST_NEW_VERSION # Use a version higher than current
echo -e "${ORANGE}[test-updater] new code added and committed. New tag version: $TEST_NEW_VERSION${NC}"

# Wait for auto-update to happen
echo -e "${ORANGE}[test-updater] Waiting for auto-update to catch the new version...${NC}"
echo " "
sleep 60  # Give updater time to detect and apply update (it checks every 20 seconds)
echo -e "${ORANGE}[test-updater] Checking if the updater applied the update...${NC}"


# Verify that the new version is running in a new process (e.g. CLI restarted)
NEW_PID=$(pgrep -f "$INSTALL_PATH" || echo "")
echo -e "${ORANGE}[test-updater] New PID: $NEW_PID${NC}"

# if the new PID is empty, the CLI is not running
if [ -z "$NEW_PID" ]; then
    echo -e "${ORANGE}❌ CLI is not running!${NC}"
    exit 1
fi

# If the new PID is the same as the original PID, the CLI was not restarted (same process)
if [ "$NEW_PID" == "$ORIGINAL_PID" ]; then
    echo -e "${ORANGE}[test-updater] ❌ CLI was not restarted (PID unchanged)${NC}"
    echo -e "${ORANGE}[test-updater] Original version: $(git describe --tags $STARTING_COMMIT)${NC}"  # Check version at start
    echo -e "${ORANGE}[test-updater] Expected version: $TEST_NEW_VERSION${NC}"
    exit 1
fi

echo -e "${ORANGE}[test-updater] ✅ CLI auto-updated and restarted successfully${NC}"