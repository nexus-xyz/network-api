# network-cli

Command line interface (CLI) for accessing the Nexus Network. Highest-performance option for proving.

## Quick start

Ensure that you have Rust. 

```
curl https://sh.rustup.rs -sSf | sh
```

Get the CLI and run it:

```
git clone https://github.com/nexus-xyz/network-cli
cd network-cli
cargo run --bin prover -- dev.orchestrator.nexus.xyz
```

Note: Hostname will default to `orchestrator.nexus.xyz` in the public beta release.

## Known issues

Currently only proving is supported. Submitting programs to the network is in private beta.
To request an API key, [contact us](https://forms.gle/183D9bcDHUdbxCV5A).
