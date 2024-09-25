#!/bin/sh

rustc --version || curl https://sh.rustup.rs -sSf | sh
NEXUS_HOME=$HOME/.nexus
CLI_ZIP=/tmp/nexus-network-api.zip
curl -L --verbose "https://docs.google.com/uc?export=download&id=1kcbEeKpVEyvIqL-_cgR5sYdZe_fOEPs6" > $CLI_ZIP
if [ -d "$NEXUS_HOME" ]; then
  echo "$NEXUS_HOME exists. Updating.";
  (cd $NEXUS_HOME && rm -rf network-api && unzip $CLI_ZIP)
  # TODO: Once GitHub repo is public, do this instead
  # (cd $NEXUS_HOME && git pull)
else
  # TODO: Once GitHub repo is public, do this instead
  # git clone git@github.com:nexus-xyz/network-cli $NEXUS_HOME
  mkdir -p $NEXUS_HOME
  (cd $NEXUS_HOME && unzip $CLI_ZIP)
fi

(cd $NEXUS_HOME/network-api/clients/cli && cargo run --release --bin prover -- beta.orchestrator.nexus.xyz)
