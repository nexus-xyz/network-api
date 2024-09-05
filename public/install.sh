#!/bin/sh

rustc --version || curl https://sh.rustup.rs -sSf | sh
NEXUS_HOME=$HOME/.nexus
if [ -d "$NEXUS_HOME" ]; then
  echo "$NEXUS_HOME exists. Updating.";
  (cd $NEXUS_HOME && git pull)
else
  git clone git@github.com:nexus-xyz/network-cli --branch @collinjackson/proving $NEXUS_HOME
fi

# Note: Hostname will default to `orchestrator.nexus.xyz` in the public beta release.

(cd $NEXUS_HOME/clients/cli && cargo run --release --bin prover -- dev.orchestrator.nexus.xyz)
