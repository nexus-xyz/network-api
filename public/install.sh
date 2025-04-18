#!/bin/sh

# -----------------------------------------------------------------------------
# 1) Ensure Rust is installed.
#    - First, check if rustc is available. If not, install Rust non-interactively
#      using the official rustup script.
# -----------------------------------------------------------------------------
rustc --version || curl https://sh.rustup.rs -sSf | sh

# -----------------------------------------------------------------------------
# 2) Define environment variables and colors for terminal output.
# -----------------------------------------------------------------------------
NEXUS_HOME="$HOME/.nexus"
GREEN='\033[1;32m'
ORANGE='\033[1;33m'
NC='\033[0m'  # No Color

# Ensure the $NEXUS_HOME directory exists.
[ -d "$NEXUS_HOME" ] || mkdir -p "$NEXUS_HOME"

# -----------------------------------------------------------------------------
# 3) Display a message if we're interactive (NONINTERACTIVE is not set) and the
#    $NODE_ID is not a 28-character ID. This is for Testnet II info.
# -----------------------------------------------------------------------------
if [ -z "$NONINTERACTIVE" ] && [ "${#NODE_ID}" -ne "28" ]; then
    echo ""
    echo "${ORANGE}Testnet II is over. The Nexus network is currently in Devnet.${NC}"
    echo ""
fi

# -----------------------------------------------------------------------------
# 4) Prompt the user to agree to the Nexus Beta Terms of Use if we're in an
#    interactive mode (i.e., NONINTERACTIVE is not set) and no node-id file exists.
#    We explicitly read from /dev/tty to ensure user input is requested from the
#    terminal rather than the script's standard input.
# -----------------------------------------------------------------------------
while [ -z "$NONINTERACTIVE" ] && [ ! -f "$NEXUS_HOME/node-id" ]; do
    read -p "Do you agree to the Nexus Beta Terms of Use (https://nexus.xyz/terms-of-use)? (Y/n) " yn </dev/tty
    echo ""
    
    case $yn in
        [Nn]* ) 
            echo ""
            exit;;
        [Yy]* ) 
            echo ""
            break;;
        "" ) 
            echo ""
            break;;
        * ) 
            echo "Please answer yes or no."
            echo "";;
    esac
done

# -----------------------------------------------------------------------------
# 5) Check for 'git' availability. If not found, prompt the user to install it.
# -----------------------------------------------------------------------------
git --version 2>&1 >/dev/null
GIT_IS_AVAILABLE=$?
if [ "$GIT_IS_AVAILABLE" != 0 ]; then
  echo "Unable to find git. Please install it and try again."
  exit 1
fi

# -----------------------------------------------------------------------------
# 6) Clone or update the network-api repository in $NEXUS_HOME.
# -----------------------------------------------------------------------------
REPO_PATH="$NEXUS_HOME/network-api"
if [ -d "$REPO_PATH" ]; then
  echo "$REPO_PATH exists. Updating."
  (
    cd "$REPO_PATH" || exit
    git stash
    git fetch --tags
  )
else
  (
    cd "$NEXUS_HOME" || exit
    git clone https://github.com/nexus-xyz/network-api
  )
fi

# -----------------------------------------------------------------------------
# 7) Check out the latest tagged commit in the repository.
# -----------------------------------------------------------------------------
(
  cd "$REPO_PATH" || exit
  git -c advice.detachedHead=false checkout "$(git rev-list --tags --max-count=1)"
)

# -----------------------------------------------------------------------------
# 8) Finally, run the Rust CLI in interactive mode. We explicitly attach
#    /dev/tty to cargo's stdin so it can prompt the user, even if the script
#    itself was piped in or otherwise redirected.
# -----------------------------------------------------------------------------
(
  cd "$REPO_PATH/clients/cli" || exit
  cargo run -r -- start --env beta
) < /dev/tty
# -----------------------------------------------------------------------------
# For local testing (e.g., staging mode), comment out the above cargo run line
# and uncomment the line below.
#
# echo "Current location: $(pwd)"
# (cd clients/cli &&   cargo run -r -- start --env beta
# )
# -----------------------------------------------------------------------------
