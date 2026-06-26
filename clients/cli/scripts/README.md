# CLI Scripts

This directory contains utility scripts for the Nexus CLI.

## Build Guest Script

The `build_guest.sh` script automates the process of creating and building guest programs using `cargo nexus host`.

### Usage

```bash
./scripts/build_guest.sh [guest_name]
```

### Parameters

- `guest_name` (optional): Name for the guest program. Defaults to `fib_input_initial`

### Examples

```bash
# Build with default name
./scripts/build_guest.sh

# Build with custom name
./scripts/build_guest.sh my_custom_program
```

### What it does

1. Creates a `programs/` directory (if it doesn't exist)
2. Uses `cargo nexus host` to generate a new guest program (or rebuilds existing one)
3. Builds the guest program for RISC-V target (`riscv32i-unknown-none-elf`)
4. Copies the built ELF to `assets/[guest_name]` in the CLI repo
5. Provides colored output and error handling

### Directory Structure

After running the script, you'll have:

```
nexus-cli/clients/cli/
├── programs/                 # Guest program source code (tracked in git)
│   └── [guest_name]/         # Generated guest program
│       ├── src/
│       │   └── guest/
│       │       └── src/
│       │           └── main.rs  # Edit this file to modify the guest program
│       └── ...
├── assets/
│   └── [guest_name]          # Built ELF file
└── scripts/
    └── build_guest.sh        # This script
```

### Modifying Guest Programs

To modify a guest program:

1. Edit the source code: `programs/[guest_name]/src/guest/src/main.rs`
2. Run the build script again: `./scripts/build_guest.sh [guest_name]`
3. The updated ELF will be copied to `assets/[guest_name]`
4. Commit your source code changes to git

### Git Integration

- **Source code is tracked**: The guest program source code in `programs/` is committed to git
- **Build artifacts are ignored**: `target/` directories and `Cargo.lock` files are excluded
- **Reproducible builds**: Anyone can clone the repo and rebuild the ELF files

### Requirements

- `cargo nexus` must be installed and available in PATH
- The CLI assets directory must exist at `assets/`

### Error Handling

The script will exit with an error if:
- `cargo nexus` is not available
- The build fails
- The ELF file can't be copied

All errors are displayed with red text and include helpful error messages.

## Peak Memory Monitor Script

The `peak_memory.sh` script monitors the peak memory usage of the nexus-compute-cli during operation.

### Usage

```bash
./scripts/peak_memory.sh [DURATION_SECONDS]
```

### Parameters

- `DURATION_SECONDS` (optional): How long to monitor in seconds. Defaults to 60 seconds

### Examples

```bash
# Monitor for default 60 seconds
./scripts/peak_memory.sh

# Monitor for 20 seconds
./scripts/peak_memory.sh 20

# Monitor for 5 minutes
./scripts/peak_memory.sh 300
```

### What it does

1. Starts `nexus-compute-cli start --headless` in the background
2. Monitors memory usage every second for the specified duration
3. Tracks peak memory usage throughout the run
4. Displays real-time current and peak memory usage
5. Terminates the process and reports final statistics

### Example Output

```bash
❯ ./scripts/peak_memory.sh 20
Starting nexus-compute-cli and monitoring memory for 20 seconds...
[1/20s] Current: 4 MB, Peak: 4 MB
[INFO!!!] ✅ Found Node ID from config file      Node ID: 19444429
Refresh [2025-07-31 08:09:24] Task Fetcher: [Task step 1 of 3] Fetching task...
[2/20s] Current: 241 MB, Peak: 241 MB
[3/20s] Current: 795 MB, Peak: 795 MB
[4/20s] Current: 1154 MB, Peak: 1154 MB
[5/20s] Current: 1993 MB, Peak: 1993 MB
[6/20s] Current: 2148 MB, Peak: 2148 MB
[7/20s] Current: 1097 MB, Peak: 2148 MB
...
[13/20s] Current: 2970 MB, Peak: 2970 MB
[14/20s] Current: 1714 MB, Peak: 2970 MB
Success [2025-07-31 08:09:37] Prover 0: [Task step 2 of 3] Proof completed successfully
[15/20s] Current: 642 MB, Peak: 2970 MB
...
[20/20s] Current: 643 MB, Peak: 2970 MB
================================
Peak Memory Usage: 2970 MB
Total Runtime: 21 seconds
================================
```
### Requirements

- `nexus-compute-cli` must be built and available in PATH or current directory
- Valid nexus configuration (Node ID) for proper operation