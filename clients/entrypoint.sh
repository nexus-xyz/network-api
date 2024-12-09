#!/bin/sh

if [ -z "${PROVER_ID}" ]; then
    echo "Error: PROVER_ID environment variable is not set or is empty"
    exit 1
fi

NEXUS_HOME=$HOME/.nexus
echo "$PROVER_ID" > $NEXUS_HOME/prover-id

exec "$@"
