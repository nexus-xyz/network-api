#!/bin/sh

rustc --version || curl https://sh.rustup.rs -sSf | sh
NEXUS_HOME=$HOME/.nexus
GREEN='\033[1;32m'
ORANGE='\033[1;33m'
NC='\033[0m' # No Color

[ -d $NEXUS_HOME ] || mkdir -p $NEXUS_HOME

while [ -z "$NONINTERACTIVE" ] && [ ! -f "$NEXUS_HOME/prover-id" ]; do
    read -p "Do you agree to the Nexus Beta Terms of Use (https://nexus.xyz/terms-of-use)? (Y/n) " yn </dev/tty
    case $yn in
        [Nn]* ) exit;;
        [Yy]* ) break;;
        "" ) break;;
        * ) echo "Please answer yes or no.";;
    esac
done

git --version 2>&1 >/dev/null
GIT_IS_AVAILABLE=$?
if [ $GIT_IS_AVAILABLE != 0 ]; then
  echo Unable to find git. Please install it and try again.
  exit 1;
fi

PROVER_ID=$(cat $NEXUS_HOME/prover-id 2>/dev/null)
if [ -z "$NONINTERACTIVE" ] && [ "${#PROVER_ID}" -ne "28" ]; then
    echo "\n${ORANGE}The Nexus network is currently in devnet. It is important to note that you cannot earn Nexus points.${NC}"
    echo "\nInstead, devnet allows developers to experiment and build with the network. Stay tuned for updates regarding future testnets.\n"
    read -p "Do you want to continue? (Y/n) " yn </dev/tty
    case $yn in
        [Nn]* ) exit;;
        [Yy]* ) ;;
        "" ) ;;
        * ) echo "Please answer yes or no."; exit;;
    esac
fi

REPO_PATH=$NEXUS_HOME/network-api
if [ -d "$REPO_PATH" ]; then
  echo "$REPO_PATH exists. Updating.";
  (cd $REPO_PATH && git stash save && git fetch --tags)
else
  (cd $NEXUS_HOME && git clone https://github.com/nexus-xyz/network-api)
fi
(cd $REPO_PATH && git -c advice.detachedHead=false checkout $(git rev-list --tags --max-count=1))

(cd $REPO_PATH/clients/cli && cargo run --release --bin prover -- beta.orchestrator.nexus.xyz)
