# Network CLI

The command line interface (CLI) lets you run a prover node and contribute proofs to the Nexus network.
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
* Linking email to prover id is currently available on the web version only.
* Counting cycles proved is not yet available in the CLI.
* Only proving is supported. Submitting programs to the network is in private beta.
To request an API key, contact us at growth@nexus.xyz.

## Modifying source

The curl command in the quick start section downloads this repo to $HOME/.nexus/network-api
and automatically runs it. If you want to modify the CLI, it's better to clone the GitHub
repo somewhere else.

To run an optimized build using Nexus servers, run the following command in clients/cli:

```
cargo run --release -- beta.orchestrator.nexus.xyz
```

To run the CLI with tracing enabled, run:

```sh
cargo run -- beta.orchestrator.nexus.xyz
```

### Building a release binary

```sh
# Build a release binary for Apple Silicon (M1/M2) Mac
cargo build --release --target aarch64-apple-darwin

# Build a release binary for Intel Mac
cargo build --release --target x86_64-apple-darwin

# Create a tarball for Apple Silicon (M1/M2) Mac binary and return to original directory
cd target/aarch64-apple-darwin/release && tar -czf aarch64-apple-darwin.tar.gz ./prover && cd -

# Create a tarball for Intel Mac binary and return to original directory
cd target/x86_64-apple-darwin/release && tar -czf x86_64-apple-darwin.tar.gz ./prover && cd -

# Test the binary (use appropriate path based on target)
./target/aarch64-apple-darwin/release/prover --version 

# Run the binary (use appropriate path based on target)
./target/aarch64-apple-darwin/release/prover beta.orchestrator.nexus.xyz
```

Note: Make sure you have the appropriate target installed via rustup:
```sh
# Add required target(s)
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu
```

## Resources

* [Network FAQ](https://nexus.xyz/network#network-faqs)
* [Discord server](https://discord.gg/nexus-xyz)
