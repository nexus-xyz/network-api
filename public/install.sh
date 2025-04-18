#!/bin/sh

# -----------------------------------------------------------------------------
# 1) Define environment variables and colors for terminal output.
# -----------------------------------------------------------------------------
NEXUS_HOME="$HOME/.nexus"
BIN_DIR="$NEXUS_HOME/bin"
GREEN='\033[1;32m'
ORANGE='\033[1;33m'
RED='\033[1;31m'
NC='\033[0m'  # No Color

# Ensure the $NEXUS_HOME and $BIN_DIR directories exist.
[ -d "$NEXUS_HOME" ] || mkdir -p "$NEXUS_HOME"
[ -d "$BIN_DIR" ] || mkdir -p "$BIN_DIR"

# -----------------------------------------------------------------------------
# 2) Display a message if we're interactive (NONINTERACTIVE is not set) and the
#    $NODE_ID is not a 28-character ID. This is for Testnet II info.
# -----------------------------------------------------------------------------
if [ -z "$NONINTERACTIVE" ] && [ "${#NODE_ID}" -ne "28" ]; then
    echo ""
    echo "${ORANGE}Testnet II is over. The Nexus network is currently in Devnet.${NC}"
    echo ""
fi

# -----------------------------------------------------------------------------
# 3) Prompt the user to agree to the Nexus Beta Terms of Use if we're in an
#    interactive mode (i.e., NONINTERACTIVE is not set) and no config.json file exists.
#    We explicitly read from /dev/tty to ensure user input is requested from the
#    terminal rather than the script's standard input.
# -----------------------------------------------------------------------------
while [ -z "$NONINTERACTIVE" ] && [ ! -f "$NEXUS_HOME/config.json" ]; do
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
# 4) Determine the platform and architecture
# -----------------------------------------------------------------------------
case "$(uname -s)" in
    Linux*)
        PLATFORM="linux"
        case "$(uname -m)" in
            x86_64)
                ARCH="x86_64"
                BINARY_NAME="nexus-network-linux-x86_64"
                ;;
            aarch64|arm64)
                ARCH="arm64"
                BINARY_NAME="nexus-network-linux-arm64"
                ;;
            *)
                echo "${RED}Unsupported architecture: $(uname -m)${NC}"
                echo "Please build from source:"
                echo "  git clone https://github.com/nexus-xyz/network-api.git"
                echo "  cd network-api/clients/cli"
                echo "  cargo build --release"
                exit 1
                ;;
        esac
        ;;
    Darwin*)
        PLATFORM="macos"
        case "$(uname -m)" in
            x86_64)
                ARCH="x86_64"
                BINARY_NAME="nexus-network-macos-x86_64"
                echo "${ORANGE}Note: You are running on an Intel Mac.${NC}"
                ;;
            arm64)
                ARCH="arm64"
                BINARY_NAME="nexus-network-macos-arm64"
                echo "${ORANGE}Note: You are running on an Apple Silicon Mac (M1/M2/M3).${NC}"
                ;;
            *)
                echo "${RED}Unsupported architecture: $(uname -m)${NC}"
                echo "Please build from source:"
                echo "  git clone https://github.com/nexus-xyz/network-api.git"
                echo "  cd network-api/clients/cli"
                echo "  cargo build --release"
                exit 1
                ;;
        esac
        ;;
    MINGW*|MSYS*|CYGWIN*)
        PLATFORM="windows"
        case "$(uname -m)" in
            x86_64)
                ARCH="x86_64"
                BINARY_NAME="nexus-network-windows-x86_64.exe"
                ;;
            *)
                echo "${RED}Unsupported architecture: $(uname -m)${NC}"
                echo "Please build from source:"
                echo "  git clone https://github.com/nexus-xyz/network-api.git"
                echo "  cd network-api/clients/cli"
                echo "  cargo build --release"
                exit 1
                ;;
        esac
        ;;
    *)
        echo "${RED}Unsupported platform: $(uname -s)${NC}"
        echo "Please build from source:"
        echo "  git clone https://github.com/nexus-xyz/network-api.git"
        echo "  cd network-api/clients/cli"
        echo "  cargo build --release"
        exit 1
        ;;
esac

# Get the latest release URL
LATEST_RELEASE_URL=$(curl -s https://api.github.com/repos/nexus-xyz/network-api/releases/latest | 
    grep "browser_download_url.*$BINARY_NAME" | 
    cut -d '"' -f 4)

if [ -z "$LATEST_RELEASE_URL" ]; then
    echo "${RED}Could not find a precompiled binary for $PLATFORM-$ARCH${NC}"
    echo "Please build from source:"
    echo "  git clone https://github.com/nexus-xyz/network-api.git"
    echo "  cd network-api/clients/cli"
    echo "  cargo build --release"
    exit 1
fi

# Download the binary
echo "Downloading latest release for $PLATFORM-$ARCH..."
curl -L -o "$BIN_DIR/nexus-network" "$LATEST_RELEASE_URL"

# Make it executable
chmod +x "$BIN_DIR/nexus-network"

# Create a symlink in a directory that's likely in the user's PATH
if [ -d "$HOME/.local/bin" ]; then
    ln -sf "$BIN_DIR/nexus-network" "$HOME/.local/bin/nexus-network"
elif [ -d "/usr/local/bin" ] && [ -w "/usr/local/bin" ]; then
    ln -sf "$BIN_DIR/nexus-network" "/usr/local/bin/nexus-network"
fi

echo "${GREEN}Installation complete!${NC}"
echo "The nexus-network binary has been installed to $BIN_DIR"
echo "You can run it with: nexus-network start --env beta"

# -----------------------------------------------------------------------------
# 5) Run the CLI in interactive mode
# -----------------------------------------------------------------------------
"$BIN_DIR/nexus-network" start --env beta < /dev/tty
