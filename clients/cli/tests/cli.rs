use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use std::path::PathBuf;

/// Helper to get a temporary config directory
fn temp_config_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create temp dir")
}

/// Helper to get config file path in the temp dir
fn config_file_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join(".nexus").join("config.json")
}

const BINARY_NAME: &str = "nexus-network";

/// Run the binary with given args and return stdout + stderr combined.
fn run_bin(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin(BINARY_NAME).unwrap().args(args).assert()
}

#[test]
/// Help command should display usage information.
fn cli_help_displays_usage() {
    let mut cmd = Command::cargo_bin(BINARY_NAME).unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(contains("Command-line arguments"));
}

#[test]
#[ignore] // This currently involves network calls and creating a config file.
fn register_user_command_creates_config_file() {
    let tmp = temp_config_dir();
    let config_path = config_file_path(&tmp);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();

    // Ensure the file does not exist initially
    assert!(!config_path.exists());

    // Run the command
    let mut cmd = Command::cargo_bin(BINARY_NAME).unwrap();
    cmd.arg("register-user")
        .arg("--wallet-address")
        .arg("0x1234567890abcdef1234567890abcdef12345600")
        .env("HOME", tmp.path()) // simulate different $HOME
        .assert()
        .success()
        .stdout(contains("User registered successfully"));

    // Confirm the file was created
    assert!(config_path.exists());
}

#[test]
/// --max-threads flag appears in `start --help`.
fn start_help_shows_max_threads() {
    run_bin(&["start", "--help"])
        .success()
        .stdout(contains("--max-threads"));
}

#[test]
/// --max-threads is accepted by clap; an unrecognized value would produce "error: invalid value".
/// Here we confirm that passing a numeric value doesn't produce a clap argument error.
/// (The command will still fail at runtime due to missing config, which is expected.)
fn start_max_threads_flag_is_parsed() {
    // We expect a runtime failure (missing config / version check), NOT a clap "unrecognized
    // argument" or "invalid value" error. The absence of those clap-level errors confirms the
    // flag was wired up correctly.
    let output = Command::cargo_bin(BINARY_NAME)
        .unwrap()
        .args(["start", "--max-threads", "4"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unrecognized argument") && !stderr.contains("invalid value for '--max-threads'"),
        "clap rejected --max-threads: {stderr}"
    );
}

#[test]
/// The hidden `prove-fib-subprocess` subcommand accepts --num-threads.
/// Passing an invalid --inputs value triggers a JSON parse error (not an "unrecognized argument"
/// error), which confirms the flag was wired up correctly.
fn subprocess_num_threads_flag_is_parsed() {
    let output = Command::cargo_bin(BINARY_NAME)
        .unwrap()
        .args([
            "prove-fib-subprocess",
            "--inputs",
            "not-valid-json",
            "--num-threads",
            "4",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unrecognized argument") && !stderr.contains("invalid value for '--num-threads'"),
        "clap rejected --num-threads: {stderr}"
    );
    // The process must have exited non-zero (invalid inputs)
    assert!(!output.status.success(), "expected non-zero exit for invalid inputs");
}

#[test]
/// Logout command should delete an existing config file.
fn logout_deletes_config_file() {
    let tmp = temp_config_dir();
    let config_path = config_file_path(&tmp);
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    fs::write(&config_path, "{}").unwrap();

    // Ensure the file exists
    assert!(config_path.exists());

    // Run the command
    let mut cmd = Command::cargo_bin(BINARY_NAME).unwrap();
    cmd.arg("logout")
        .env("HOME", tmp.path()) // simulate different $HOME
        .assert()
        .success()
        .stdout(contains("Logging out"));

    // Confirm the file was deleted
    assert!(!config_path.exists());
}
