# Nexus Network CLI

A high-performance command-line interface for contributing proofs to the Nexus network.

<figure>
    <a href="https://beta.nexus.xyz/">
        <img src="assets/images/nexus-network-image.png" alt="Nexus Network visualization showing a distributed network of interconnected nodes with a 'Launch Network' button in the center">
    </a>
    <figcaption>
        <strong>Verifiable Computation on a Global Scale</strong><br>
        We're building a global distributed prover network to unite the world's computers and power a new and better Internet: the Verifiable Internet. Connect to the beta and give it a try today.
    </figcaption>
</figure>

## Quick Start

```bash
curl https://cli.nexus.xyz/ | sh
```

If you don't have Rust installed, you will be prompted to install it.

## Prerequisites

### Linux
```bash
sudo apt update && sudo apt upgrade
sudo apt install build-essential pkg-config libssl-dev git-all
```

### macOS
```bash
brew install git
```

### Windows
[Install WSL](https://learn.microsoft.com/en-us/windows/wsl/install) first, then follow Linux instructions.

## Terms of Use

Use of the CLI is subject to the [Terms of Use](https://nexus.xyz/terms-of-use). First-time users will be prompted to accept the terms. For non-interactive acceptance (e.g., CI environments), use:

```bash
NONINTERACTIVE=1 sh
```

## Prover Id

The CLI will prompt for your web prover id from [beta.nexus.xyz](https://beta.nexus.xyz/)
on startup. It is ok to skip this prompt and a random id will be generated, but you'll be
prompted again on startup until your web prover id is entered.

The prover id prompt is disabled when NONINTERACTIVE=1 is set. In a server environment,
you can manually overwrite $HOME/.nexus/prover-id with your full prover id.

## Current Limitations

- Only latest CLI version is supported
- No prebuilt binaries yet
- Proof cycle counting coming soon
- Program submission requires API key (contact growth@nexus.xyz)

## Get Help

- [Network FAQ](https://nexus.xyz/network#network-faqs)
- [Discord Community](https://discord.gg/nexus-xyz)
- Technical issues? [Open an issue](https://github.com/nexus-xyz/network-api/issues)

## Repository Structure

```
network-api/
├── assets/       # Media for documentation
├── clients/
│   └── cli/      # Main CLI implementation
├── proto/        # Shared network interface definition
└── public/       # Files hosted at cli.nexus.xyz
```

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development setup and guidelines.
