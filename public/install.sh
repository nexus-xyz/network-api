#!/bin/sh

rustc --version || curl https://sh.rustup.rs -sSf | sh
NEXUS_HOME=$HOME/.nexus

while [ -z "$NONINTERACTIVE" ]; do
    read -p "Do you agree to the Nexus Beta Terms of Use (https://nexus.xyz/terms-of-use)? (Y/n) " yn </dev/tty
    case $yn in
        [Yy]* ) break;;
        [Nn]* ) exit;;
        * ) echo "Please answer yes or no.";;
    esac
done

if [ -d "$NEXUS_HOME/network-api" ]; then
  echo "$NEXUS_HOME/network-api exists. Updating.";
  (cd $NEXUS_HOME/network-api && git pull)
else
  mkdir -p $NEXUS_HOME
  (cd $NEXUS_HOME && git clone git@github.com:nexus-xyz/network-cli)
fi

(cd $NEXUS_HOME/network-api/clients/cli && cargo run --release --bin prover -- beta.orchestrator.nexus.xyz)
