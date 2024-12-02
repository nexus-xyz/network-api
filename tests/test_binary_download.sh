#!/bin/bash

set -e  # Exit on any error

# Configuration
ORANGE='\033[1;33m'
NC='\033[0m' # No Color
NEXUS_HOME="${HOME}/.nexus"
ORCHESTRATOR_HOST="beta.orchestrator.nexus.xyz"

# Change to the clients/cli directory where Cargo.toml is located
cd "$(dirname "$0")/../clients/cli"

echo -e "${ORANGE}(1 of 3). Cleaning existing installation...${NC}"
rm -rf $NEXUS_HOME

echo -e "${ORANGE}(2 of 3). Downloading prover binary from GitHub...${NC}"
# This will trigger the auto-download of the latest version from GitHub
# Force terminal to be interactive and prevent buffering
script -q /dev/null cargo run --release -- $ORCHESTRATOR_HOST

echo -e "${ORANGE}(3 of 3). Verifying installed version:${NC}"
$NEXUS_HOME/bin/prover --version
