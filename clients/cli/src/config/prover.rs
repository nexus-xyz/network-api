use crate::utils::analytics::track;
use crate::utils::prover_id::get_or_generate_prover_id;

use nexus_core::prover::nova::{
    pp::gen_vm_pp,
    types::{seq, PublicParams, C1, C2, G1, G2, RO, SC},
};
use serde_json::json;
// use serial_test::serial;
// use std::sync::Once;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

/// Configuration for the RISC-V zero-knowledge prover
///
/// This struct holds essential parameters used throughout the proving process:
/// - `prover_id`: Unique identifier for this prover instance
/// - `k`: Step size for the proving system (number of cycles per step)
/// - `ws_addr_string`: WebSocket address for connecting to the orchestrator
/// - `public_parameters`: Nova-specific parameters for generating zero-knowledge proofs
///
/// Used in main.rs to initialize the prover and maintain connection settings
/// throughout the proving lifecycle.
pub struct ProverConfig {
    pub prover_id: String,
    pub k: i32,
    pub ws_addr_string: String,
    #[allow(clippy::type_complexity)]
    pub public_parameters:
        PublicParams<G1, G2, C1, C2, RO, SC, seq::SetupParams<(G1, G2, C1, C2, RO, SC)>>,
}

pub async fn initialize(
    hostname: String,
    port: u16,
) -> Result<ProverConfig, Box<dyn std::error::Error>> {
    // Configure the tracing subscriber
    // This is a global value so we do not need to pass it around
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .init();

    // Construct the WebSocket URL based on the port number
    // Uses secure WebSocket (wss) for port 443, regular WebSocket (ws) otherwise
    let ws_addr_string = format!(
        "{}://{}:{}/prove",
        if port == 443 { "wss" } else { "ws" },
        hostname,
        port
    );

    // Set the constant k value used for proof generation
    // This determines the size/complexity of the proving system
    // Higher values increase proof generation speed but require more memory
    let k = 4;

    // Retrieve an existing prover ID from storage or generate a new one
    // This ID uniquely identifies this prover instance
    let prover_id = get_or_generate_prover_id();

    // Track the registration event
    track(
        "register".into(),
        format!("Your assigned prover identifier is {}.", prover_id),
        &ws_addr_string,
        json!({"ws_addr_string": ws_addr_string, "prover_id": prover_id}),
    );

    // Generate the public parameters for the proving system
    #[allow(clippy::type_complexity)]
    let public_parameters: PublicParams<
        G1,
        G2,
        C1,
        C2,
        RO,
        SC,
        seq::SetupParams<(G1, G2, C1, C2, RO, SC)>,
    > = match gen_vm_pp::<C1, seq::SetupParams<(G1, G2, C1, C2, RO, SC)>>(k as usize, &()) {
        Ok(params) => params,
        Err(e) => return Err(format!("Failed to generate public parameters: {}", e).into()),
    };

    Ok(ProverConfig {
        ws_addr_string,
        k,
        prover_id,
        public_parameters,
    })
}
