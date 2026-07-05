use crate::environment::Environment;
use crate::prover::input::InputParser;
use crate::system::{estimate_peak_gflops, measure_gflops, num_cores};
use crate::task::Task;
use chrono::Datelike;
use chrono::Timelike;
use reqwest::header::ACCEPT;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, thiserror::Error)]
pub enum TrackError {
    #[error("System time error: {0}")]
    SystemTimeError(#[from] std::time::SystemTimeError),

    #[error("event_properties is not a valid JSON object")]
    InvalidEventProperties,

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Non-successful response: {status} - {body}")]
    FailedResponse {
        status: reqwest::StatusCode,
        body: String,
    },
}

pub const PRODUCTION_MEASUREMENT_ID: &str = "G-GLH0GMEEFH";
pub const PRODUCTION_API_SECRET: &str = "3wxu8FjVSPqOlxSsZEnBOw";

// Expected input size for fib_input_initial (3 u32 values = 12 bytes)
const FIB_INPUT_INITIAL_BYTES: usize = (u32::BITS / 8 * 3) as usize;

pub fn analytics_id(environment: &Environment) -> String {
    match environment {
        Environment::Production => PRODUCTION_MEASUREMENT_ID.to_string(),
        Environment::Custom { .. } => String::new(), // Disable analytics for custom environments
    }
}

pub fn analytics_api_key(environment: &Environment) -> String {
    match environment {
        Environment::Production => PRODUCTION_API_SECRET.to_string(),
        Environment::Custom { .. } => String::new(), // Disable analytics for custom environments
    }
}

/// Track an event with the Firebase Measurement Protocol
///
/// # Arguments
/// * `event_name` - The name of the event to track.
/// * `event_properties` - A JSON object containing properties of the event.
/// * `environment` - The environment in which the application is running.
/// * `client_id` - A unique identifier for the client, typically a UUID or similar.
pub async fn track(
    event_names: Vec<String>,
    event_properties: Value,
    environment: &Environment,
    client_id: String,
) -> Result<(), TrackError> {
    let analytics_id = analytics_id(environment);
    let analytics_api_key = analytics_api_key(environment);
    if analytics_id.is_empty() {
        return Ok(());
    }
    let local_now = chrono::offset::Local::now();

    // For tracking events, we use the Firebase Measurement Protocol
    // Firebase is mostly designed for mobile and web apps, but for our use case of a CLI,
    // we can use the Measurement Protocol to track events by POST to a URL.
    // The only thing that may be unexpected is that the URL we use includes a firebase key

    // Firebase format for properties for Measurement protocol:
    // https://developers.google.com/analytics/devguides/collection/protocol/ga4/reference?client_type=firebase#payload
    // https://developers.google.com/analytics/devguides/collection/protocol/ga4/reference?client_type=firebase#payload_query_parameters

    let system_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let timezone = iana_time_zone::get_timezone().ok().map_or_else(
        || String::from("UTC"), // fallback to UTC
        |tz| tz,
    );

    let mut properties = json!({
        "time": system_time,
        "platform": "CLI",
        "os": env::consts::OS,
        "os_version": env::consts::OS,  // We could get more specific version if needed
        "app_version": env!("CARGO_PKG_VERSION"),
        "timezone": timezone,
        "local_hour": local_now.hour(),
        "day_of_week": local_now.weekday().number_from_monday(),
        "event_id": system_time,
        "measured_flops": measure_gflops(),
        "num_cores": num_cores(),
        "peak_flops": estimate_peak_gflops(num_cores()),
    });

    // Add event properties to the properties JSON
    // This is done by iterating over the key-value pairs in the event_properties JSON object
    // but checking that it is a valid JSON object first
    if let Some(obj) = event_properties.as_object() {
        for (k, v) in obj {
            properties[k] = v.clone();
        }
    } else {
        return Err(TrackError::InvalidEventProperties);
    }

    // Format for events
    let body = json!({
        "client_id": client_id,
        "events": event_names.iter().map(|event_name| {
            json!({
                "name": event_name,
                "params": properties
            })
        }).collect::<Vec<_>>(),
    });

    let client = reqwest::Client::new();
    let url = format!(
        "https://www.google-analytics.com/mp/collect?measurement_id={}&api_secret={}",
        analytics_id, analytics_api_key
    );

    let response = client
        .post(&url)
        .json(&body)
        .header(ACCEPT, "application/json")
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await?;
        return Err(TrackError::FailedResponse {
            status,
            body: body_text,
        });
    }

    Ok(())
}

/// Cloud Function endpoint for reporting proving activity
const REPORT_PROVING_URL: &str = "https://us-central1-nexus-prove.cloudfunctions.net/reportProving";
/// User-Agent for nexus-cli requests (used by Cloud Function for special handling)
const CLI_USER_AGENT: &str = concat!("nexus-cli/", env!("CARGO_PKG_VERSION"));

/// Global, per-address, in-process rate limiter for reportProving calls
static LAST_REPORT_BY_ADDRESS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
/// Global wallet address for reporting; set once during session setup
static REPORT_WALLET_ADDRESS: OnceLock<String> = OnceLock::new();

/// Set the wallet address used for reporting proving activity
pub fn set_wallet_address_for_reporting(address: String) {
    let _ = REPORT_WALLET_ADDRESS.set(address);
}

/// Report proving activity to our Cloud Function at most once per hour per wallet address.
/// When the response contains `{"result":{"status":"ok","message":"rewards_processed"}}`,
/// sends a rewards event to the TUI for display (if event sender is configured).
pub async fn report_proving_if_needed() {
    let Some(wallet_address) = REPORT_WALLET_ADDRESS.get() else {
        return;
    };
    // Initialize map
    let map = LAST_REPORT_BY_ADDRESS.get_or_init(|| Mutex::new(HashMap::new()));

    let now = Instant::now();

    // Check (but do NOT update yet) the rate-limit window.
    // The timestamp is only advanced after a successful request so that a
    // transient network failure does not suppress retries for a full hour.
    let should_send = {
        let guard = match map.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        !matches!(guard.get(wallet_address), Some(&last) if now.duration_since(last) < Duration::from_secs(3600))
    };

    if !should_send {
        return;
    }

    let client = reqwest::Client::new();
    let body = json!({
        "data": { "address": wallet_address }
    });

    let result = client
        .post(REPORT_PROVING_URL)
        .header(reqwest::header::USER_AGENT, CLI_USER_AGENT)
        .json(&body)
        .send()
        .await;

    // Only advance the rate-limit timestamp when the request succeeds.
    // On failure, log the error and leave the timestamp unchanged so the
    // next invocation can retry rather than silently waiting a full hour.
    match result {
        Ok(resp) if resp.status().is_success() => {
            let mut guard = match map.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.insert(wallet_address.to_string(), now);
        }
        Ok(resp) => {
            eprintln!(
                "[nexus-cli] reportProving returned non-success status: {}",
                resp.status()
            );
        }
        Err(e) => {
            eprintln!("[nexus-cli] reportProving request failed: {}", e);
        }
    }
}

/// Track analytics for getting a task from orchestrator (non-blocking)
pub async fn track_got_task(task: crate::task::Task, environment: Environment, client_id: String) {
    let analytics_data = json!({
        "program_name": task.program_id,
        "task_id": task.task_id,
    });

    let _ = track(
        vec!["cli_got_task".to_string(), "got_task".to_string()],
        analytics_data,
        &environment,
        client_id,
    )
    .await;
    // TODO: Catch errors and log them
}

/// Track analytics for proof verification failure (non-blocking)
pub async fn track_verification_failed(
    task: crate::task::Task,
    error: String,
    environment: Environment,
    client_id: String,
) {
    let analytics_data = json!({
        "program_name": task.program_id,
        "task_id": task.task_id,
        "error": error,
    });

    let _ = track(
        vec![
            "cli_local_verification_failed".to_string(),
            "local_verification_failed".to_string(),
        ],
        analytics_data,
        &environment,
        client_id,
    )
    .await;
    // TODO: Catch errors and log them
}

/// Track analytics for proof submission error (non-blocking)
pub async fn track_proof_submission_error(
    task: crate::task::Task,
    error: String,
    status_code: Option<u16>,
    environment: Environment,
    client_id: String,
) {
    let mut analytics_data = json!({
        "program_name": task.program_id,
        "task_id": task.task_id,
        "error": error,
    });

    if let Some(status) = status_code {
        analytics_data["status_code"] = json!(status);
    }

    let _ = track(
        vec![
            "cli_proof_submission_error".to_string(),
            "proof_submission_error".to_string(),
        ],
        analytics_data,
        &environment,
        client_id,
    )
    .await;
    // TODO: Catch errors and log them
}

/// Track analytics for proof acceptance (non-blocking)
pub async fn track_proof_accepted(
    task: crate::task::Task,
    environment: Environment,
    client_id: String,
) {
    let analytics_data = json!({
        "program_name": task.program_id,
        "task_id": task.task_id,
    });

    let _ = track(
        vec![
            "cli_proof_accepted".to_string(),
            "proof_accepted".to_string(),
        ],
        analytics_data,
        &environment,
        client_id,
    )
    .await;
    // TODO: Catch errors and log them

    // Rate-limited cloud ping
    tokio::spawn(async move {
        report_proving_if_needed().await;
    });
}

/// Track analytics for proof submission success (non-blocking)
pub async fn track_proof_submission_success(
    task: crate::task::Task,
    environment: Environment,
    client_id: String,
) {
    let analytics_data = json!({
        "program_name": task.program_id,
        "task_id": task.task_id,
    });

    let _ = track(
        vec![
            "cli_proof_submission_success".to_string(),
            "proof_submission_success".to_string(),
        ],
        analytics_data,
        &environment,
        client_id,
    )
    .await;
    // TODO: Catch errors and log them

    // Rate-limited cloud ping
    tokio::spawn(async move {
        report_proving_if_needed().await;
    });
}

/// Track analytics for authenticated proof (non-blocking)
pub async fn track_authenticated_proof_analytics(
    task: Task,
    environment: Environment,
    client_id: String,
) {
    let analytics_data = match task.program_id.as_str() {
        "fib_input_initial" => {
            // For fib_input_initial, extract the triple inputs from the first input
            let all_inputs = task.all_inputs();
            let input_data = if all_inputs.is_empty() {
                &vec![]
            } else {
                &all_inputs[0]
            };

            // Check if we have the expected number of bytes for fib_input_initial
            if input_data.len() >= FIB_INPUT_INITIAL_BYTES && FIB_INPUT_INITIAL_BYTES >= 12 {
                // Use safe slicing that won't panic

                InputParser::parse_triple_input(input_data)
                    .map(|inputs| {
                        json!({
                            "program_name": "fib_input_initial",
                            "public_input": inputs.0,
                            "public_input_2": inputs.1,
                            "public_input_3": inputs.2,
                            "task_id": task.task_id,
                        })
                    })
                    .unwrap_or_else(|_| {
                        // Fallback for slicing error - just log the program and task
                        json!({
                            "program_name": "fib_input_initial",
                            "task_id": task.task_id,
                            "input_size": input_data.len(),
                            "expected_size": FIB_INPUT_INITIAL_BYTES,
                            "error": "safe_slicing_failed",
                        })
                    })
            } else {
                json!({
                    "program_name": "fib_input_initial",
                    "task_id": task.task_id,
                    "input_size": input_data.len(),
                    "expected_size": FIB_INPUT_INITIAL_BYTES,
                })
            }
        }
        _ => {
            json!({
                "program_name": task.program_id,
                "task_id": task.task_id,
            })
        }
    };

    let _ = track(
        vec!["cli_proof_node_v4".to_string(), "proof_node".to_string()],
        analytics_data,
        &environment,
        client_id,
    )
    .await;
    // TODO: Catch errors and log them
}

/// Track analytics for likely OOM error in proof subprocess (non-blocking)
pub async fn track_likely_oom_error(task: Task, environment: Environment, client_id: String) {
    let analytics_data = json!({
        "program_name": task.program_id,
        "task_id": task.task_id,
    });

    let _ = track(
        vec![
            "cli_likely_oom_error".to_string(),
            "likely_oom_error".to_string(),
        ],
        analytics_data,
        &environment,
        client_id,
    )
    .await;
    // TODO: Catch errors and log them
}
