# Network CLI

The command line interface (CLI) lets you run a Nexus prover node.
It is the highest-performance option for proving.

## Prerequisites

If you don't have these dependencies already, install them first.

### Linux

```
sudo apt update
sudo apt upgrade
sudo apt install build-essential pkg-config libssl-dev git-all
```

### macOS

If you have [installed Homebrew](https://brew.sh/) to manage packages on OS X,
run this command to install Git.

```
brew install git
```

### Windows

[Install WSL](https://learn.microsoft.com/en-us/windows/wsl/install),
then see Linux instructions above.

## Quick start

```
curl https://cli.nexus.xyz/ | sh
```

If you do not already have Rust, you will be prompted to install it.

## Terms of Use

Use of the CLI is subject to the [Terms of Use](https://nexus.xyz/terms-of-use).
The first time you run it, it prompts you to accept the terms. To accept the terms
noninteractively (for example, in a continuous integration environment),
add `NONINTERACTIVE=1` before `sh`.

## Known issues

* Only the latest version of the CLI is currently supported.
* Prebuilt binaries are not yet available.
* Counting cycles proved is not yet available in the CLI.
* Only proving is supported. Submitting programs to the network is in private beta.
To request an API key, contact us at growth@nexus.xyz.

## Resources

* [Network FAQ](https://nexus.xyz/network#network-faqs)
* [Discord channel](https://discord.gg/nexus-xyz)
