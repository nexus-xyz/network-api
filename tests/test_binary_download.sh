#!/bin/bash

set -e  # Exit on any error

# Configuration
ORANGE='\033[1;33m'
NC='\033[0m' # No Color
NEXUS_HOME="${HOME}/.nexus"
TEST_RELEASE_DIR="/tmp/nexus-test-release"
ORCHESTRATOR_HOST="beta.orchestrator.nexus.xyz"

echo -e "${ORANGE}Setting up test release directory...${NC}"
mkdir -p $TEST_RELEASE_DIR

echo -e "${ORANGE}Building release binary...${NC}"
cd clients/cli
cargo build --release
cp target/release/prover .

echo -e "${ORANGE}Creating test release archive...${NC}"
tar -czf "${TEST_RELEASE_DIR}/aarch64-apple-darwin.tar.gz" prover
rm prover

echo -e "${ORANGE}Archive contents:${NC}"
tar tvf "${TEST_RELEASE_DIR}/aarch64-apple-darwin.tar.gz"

echo -e "${ORANGE}Running prover...${NC}"
TEST_RELEASE_DIR=$TEST_RELEASE_DIR cargo run --release -- $ORCHESTRATOR_HOST
