[![Release](https://img.shields.io/github/v/release/nexus-xyz/nexus-cli.svg)](https://github.com/nexus-xyz/nexus-cli/releases)
[![CI](https://github.com/nexus-xyz/nexus-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/nexus-xyz/nexus-cli/actions)
[![License](https://img.shields.io/badge/License-Apache_2.0-green.svg)](https://github.com/nexus-xyz/nexus-cli/blob/main/LICENSE-APACHE)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](https://github.com/nexus-xyz/nexus-cli/blob/main/LICENSE-MIT)
[![Twitter](https://img.shields.io/twitter/follow/NexusLabs)](https://x.com/NexusLabs)
[![Discord](https://img.shields.io/badge/Discord-Join-7289da.svg?logo=discord&logoColor=white)](https://discord.com/invite/nexus-xyz)

# Nexus CLI

A high-performance command-line interface for contributing proofs to the Nexus network.

## Nexus Network

The [Nexus network](https://app.nexus.xyz/compute) unifies compute from nodes around the world into one verifiable system state. [Learn more](https://docs.nexus.xyz/network).

There have been several testnets so far:

- Testnet 0: [October 8 – 28, 2024](https://blog.nexus.xyz/nexus-launches-worlds-first-open-prover-network/)
- Testnet I: [December 9 – 13, 2024](https://blog.nexus.xyz/the-new-nexus-testnet-is-live/)
- Testnet II: [February 18 – 22, 2025](https://blog.nexus.xyz/testnet-ii-is-open/)
- Devnet: [February 22 - June 20, 2025](https://docs.nexus.xyz/network/proving-on-the-layer-1/nexus-layer-1-devnet)
- Testnet III: [Ongoing](https://blog.nexus.xyz/live-everywhere/)

---

## Quick Start

### Installation

#### Precompiled Binary (Recommended)

For the simplest and most reliable installation:

```bash
curl https://cli.nexus.xyz/ | sh
```

This downloads the latest binary, prompts for Terms of Use acceptance, and starts interactive mode.

#### Non-Interactive Installation

For automated installations (e.g., in CI):

```bash
curl -sSf https://cli.nexus.xyz/ -o install.sh
chmod +x install.sh
NONINTERACTIVE=1 ./install.sh
```

> **Note on the command name:** The CLI command is now `nexus-compute-cli`. The
> previous name, `nexus-cli`, continues to work as an alias during the transition
> window, so existing scripts won't break — but new setups should use
> `nexus-compute-cli`.

### Proving

Proving with the CLI is documented [here](https://docs.nexus.xyz/network/proving-on-the-layer-1/contribute-via-cli).

To start with an existing node ID, run:

```bash
nexus-compute-cli start --node-id <your-node-id>
```

Alternatively, you can register your wallet address and create a node ID with the CLI, or at [app.nexus.xyz](https://app.nexus.xyz).

```bash
nexus-compute-cli register-user --wallet-address <your-wallet-address>
nexus-compute-cli register-node --node-id <your-cli-node-id>
nexus-compute-cli start
```

To run the CLI noninteractively, you can also opt to start it in headless mode.

```bash
nexus-compute-cli start --headless
```

#### Quick Reference

The `register-user` and `register-node` commands will save your credentials to `~/.nexus/config.json`. To clear credentials, run:

```bash
nexus-compute-cli logout
```

For troubleshooting or to see available command-line options, run:

```bash
nexus-compute-cli --help
```

### Adaptive Task Difficulty

The Nexus CLI features an **adaptive difficulty system** that automatically adjusts task difficulty based on your node's performance. This ensures optimal resource utilization while preventing system overload.

#### How It Works

- **Starts at**: `small` difficulty
- **Auto-promotes**: If tasks complete in < 7 minutes

#### When to Override Difficulty

**Lower Difficulty** (e.g. `Small` or `SmallMedium`):
- Resource-constrained systems
- Background processing alongside other apps
- Testing/development environments
- Battery-powered devices

**Higher Difficulty** (e.g. `Large`, `ExtraLarge`, or `ExtraLarge2`):
- High-performance hardware (8+ cores, 16+ GB RAM)
- Dedicated proving machines
- Maximum reward optimization

#### Using Difficulty Override

```bash
# Lower difficulty for resource-constrained systems
nexus-compute-cli start --max-difficulty small
nexus-compute-cli start --max-difficulty small_medium

# Higher difficulty for powerful hardware
nexus-compute-cli start --max-difficulty medium
nexus-compute-cli start --max-difficulty large
nexus-compute-cli start --max-difficulty extra_large
nexus-compute-cli start --max-difficulty extra_large_2
nexus-compute-cli start --max-difficulty extra_large_3
nexus-compute-cli start --max-difficulty extra_large_4

# Equivalent to extra_large_4 if no extra_large_5 tasks available
nexus-compute-cli start --max-difficulty extra_large_5

# Case-insensitive (all equivalent)
nexus-compute-cli start --max-difficulty MEDIUM
nexus-compute-cli start --max-difficulty medium
nexus-compute-cli start --max-difficulty Medium
```

#### Difficulty Guidelines

| Difficulty | Use Case |
|------------|----------|
| `small` | Default, starting task |
| `small_medium` | Building reputation |
| `medium` and `large` | Standard desktop/laptop |
| `extra_large` and above | High-performance systems, more points |

> **Tip**: Use `nexus-compute-cli start --help` to see the full auto-promotion details in the CLI help text.

#### Troubleshooting Difficulty Issues

**Tasks taking too long:**

Try a lower difficulty.

```bash
nexus-compute-cli start --max-difficulty small_medium
```

**Want more challenging tasks:**

Request a harder difficulty. It will still take time to build up reputation to get the requested difficulty.

```bash
nexus-compute-cli start --max-difficulty extra_large_2
```

**Unsure about system capabilities:**
- Use the default adaptive system (no `--max-difficulty` needed)
- The system will automatically find the optimal difficulty for your hardware
- Only override if you're fine-tuning performance

### Docker Installation

For containerized deployments:

1. Install [Docker](https://docs.docker.com/engine/install/) and [Docker Compose](https://docs.docker.com/compose/install/)
2. Update the node ID in `docker-compose.yaml`
3. Build and run:

```bash
docker compose build --no-cache
docker compose up -d
docker compose logs  # Check logs
docker compose down  # Shutdown
```

---

## Terms of Use

Use of the CLI is subject to the [Terms of Use](https://nexus.xyz/terms-of-use).
First-time users running interactively will be prompted to accept these terms.

---

## Node ID

During the CLI's startup, you'll be asked for your node ID. To skip prompts in a
non-interactive environment, manually create a `~/.nexus/config.json` in the
following format:

```json
{
   "node_id": "<YOUR NODE ID>"
}
```

---

## Get Help

- [Network FAQ](https://docs.nexus.xyz/network/proving-on-the-layer-1/faq)
- [Discord Community](https://discord.gg/nexus-xyz)
- Technical issues? [Open an issue](https://github.com/nexus-xyz/nexus-cli/issues)
- To submit programs to the network for proving, contact
  [growth@nexus.xyz](mailto:growth@nexus.xyz).

---

## Contributing

Interested in contributing to the Nexus Network CLI? Check out our
[Contributor Guide](./CONTRIBUTING.md) for:

- Development setup instructions
- How to report issues and submit pull requests
- Our code of conduct and community guidelines
- Tips for working with the codebase

For most users, we recommend using the precompiled binaries as described above.
The contributor guide is intended for those who want to modify or improve the CLI
itself.

### 🛠  Developer Guide

The following steps may be required in order to set up a development environment for contributing to the project:

#### Linux

```bash
sudo apt update
sudo apt upgrade
sudo apt install build-essential pkg-config libssl-dev git-all
sudo apt install protobuf-compiler
```

#### macOS

```bash
# Install using Homebrew
brew install protobuf

# Verify installation
protoc --version
```

#### Windows

[Install WSL](https://learn.microsoft.com/en-us/windows/wsl/install),
then see Linux instructions above.

```bash
# Install using Chocolatey
choco install protobuf
```

## License

Nexus CLI is distributed under the terms of both the [MIT License](./LICENSE-MIT) and the [Apache License (Version 2.0)](./LICENSE-APACHE).
