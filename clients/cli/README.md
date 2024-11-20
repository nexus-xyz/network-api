# Nexus Network CLI Implementation

This directory contains the implementation of the Nexus Network CLI prover node. For installation and usage instructions, see the [main README](../../README.md).

## Development Setup

1. Ensure you have Rust installed:
```bash
rustup update stable
```

2. Build the CLI:
```bash
cargo build
```

3. Run tests:
```bash
cargo test
```

## Project Structure

```
cli/
├── src/
│   ├── main.rs          # Entry point
│   ├── prover.rs        # Prover implementation
│   ├── config.rs        # Configuration handling
│   └── network.rs       # Network communication
├── tests/               # Integration tests
└── examples/            # Usage examples
```

## Architecture

The CLI is built around these core components:
- Prover: Handles proof generation and validation
- Network Interface: Manages communication with the Nexus network
- Configuration: Handles user settings and environment setup

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Run tests (`cargo test`)
4. Commit your changes (`git commit -m 'Add amazing feature'`)
5. Push to the branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

## Local Development

To run the CLI locally:
```bash
cargo run -- --help
```

For development with local network:
```bash
cargo run -- --dev-mode
```

## Testing

```bash
# Run all tests
cargo test

# Run specific test suite
cargo test prover

# Run with logging
RUST_LOG=debug cargo test
```

## License

See [LICENSE](../../LICENSE) file in the root directory.
