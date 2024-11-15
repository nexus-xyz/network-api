// Copyright (c) 2024 Nexus. All rights reserved.

mod analytics;
mod config;
mod generated;
mod connection;
mod prover_id_manager;
mod websocket; 


use crate::analytics::track;
use crate::websocket::receive_program_message;

use std::borrow::Cow;

use crate::connection::{connect_to_orchestrator_with_retry};

use clap::Parser;
use futures::{SinkExt};
use generated::pb::{
    compiled_program::Program,
     ProverResponse, ClientProgramProofRequest, vm_program_input::Input
};
use std::time::Instant;
use prost::Message as _;
use serde_json::json;
// Network connection types for WebSocket communication

// WebSocket protocol types for message handling
use tokio_tungstenite::tungstenite::protocol::{
    frame::coding::CloseCode,  // Status codes for connection closure (e.g., 1000 for normal)
    CloseFrame,               // Frame sent when closing connection (includes code and reason)
    Message,                  // Different types of WebSocket messages (Binary, Text, Ping, etc.)
};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;


use nexus_core::{
    nvm::{
        interactive::{parse_elf, trace},
        memory::MerkleTrie,
        NexusVM,
    },
    prover::nova::{
        init_circuit_trace, key::CanonicalSerialize, pp::gen_vm_pp, prove_seq_step, types::*,
    },
};
use zstd::stream::Encoder;

#[derive(Parser, Debug)]
struct Args {
    /// Hostname at which Orchestrator can be reached
    hostname: String,

    /// Port over which to communicate with Orchestrator
    #[arg(short, long, default_value_t = 443u16)]
    port: u16,

    /// Whether to hang up after the first proof
    #[arg(short, long, default_value_t = false)]
    just_once: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{
    // Configure the tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let args = Args::parse();

    let ws_addr_string = format!(
        "{}://{}:{}/prove",
        if args.port == 443 { "wss" } else { "ws" },
        args.hostname,
        args.port
    );

    let k = 4;
    // TODO(collinjackson): Get parameters from a file or URL.
    let pp = gen_vm_pp::<C1, seq::SetupParams<(G1, G2, C1, C2, RO, SC)>>(k as usize, &())
        .expect("error generating public parameters");

    // get or generate the prover id
    let prover_id = prover_id_manager::get_or_generate_prover_id();

    track(
        "connect".into(),
        format!("Connecting to {}...", &ws_addr_string),
        &ws_addr_string,
        json!({"prover_id": prover_id}),
    );

    // Connect to the Orchestrator with exponential backoff
    let mut client = connect_to_orchestrator_with_retry(&ws_addr_string, &prover_id).await;    

    track(
        "register".into(),
        format!("Your assigned prover identifier is {}.", prover_id),
        &ws_addr_string,
        json!({"ws_addr_string": ws_addr_string, "prover_id": prover_id}),
    );
    loop {

        



        let program_message = match receive_program_message(&mut client, &ws_addr_string, &prover_id).await {
            Ok(message) => message,
            Err(e) => {
                eprintln!("Failed to receive program message: {}", e);
                continue;  // Skip rest of this iteration, try to receive next message
            }
        };

        let program = match ProverResponse::decode(program_message.as_slice()) {
            Ok(program) => program,
            Err(e) => {
                track(
                    "decode_error".into(),
                    format!("Failed to decode prover response: {}", e),
                    &ws_addr_string,
                    json!({
                        "prover_id": &prover_id,
                        "error": e.to_string(),
                        "message_size": program_message.len(),
                    }),
                );
                eprintln!("Failed to decode program message: {}", e);
                return Err("Decode error".into());     // Exit with error

                // return Err(e.into());
            }
        };

        let program_enum = program
            .to_prove
            .as_ref()                                   // Borrow instead of move
            .ok_or("No program to prove")?              // handle first Option
            .program
            .as_ref()                                   // Borrow instead of move
            .ok_or("Program field is None")?            // handle second Option
            .program
            .as_ref()                                   // Borrow instead of move
            .ok_or("Program inner field is None")?;      // handle third Option
        
        // Then extract the ELF bytes with proper error handling
        // let elf_bytes = match program_enum {
        //     Program::Rv32iElfBytes(bytes) => bytes
        // };
        let Program::Rv32iElfBytes(elf_bytes) = program_enum;


        let to_prove = match program.to_prove.clone() {
            Some(to_prove) => to_prove,
            None => {
                // Log the error
                track(
                    "program_error".into(),
                    "No program to prove".into(),
                    &ws_addr_string,
                    json!({
                        "prover_id": &prover_id,
                        "error": "to_prove is None"
                    }),
                );
                // Return error instead of panicking
                return Err("No program to prove".into());
            }
        };

        // First handle the nested Options with proper error messages
        let input_enum = to_prove
            .input
            .as_ref()
            .ok_or("No input provided")?
            .input
            .as_ref()
            .ok_or("Input field is None")?;

        // Then match on the Input enum variant
        let Input::RawBytes(bytes) = input_enum;
        let input = bytes.clone();

        track(
            "program".into(),
            format!(
                "Received a {} byte program to prove with {} bytes of input",
                elf_bytes.len(),
                input.len()
            ),
            &ws_addr_string,
            json!({"prover_id": prover_id}),
        );

        let mut vm: NexusVM<MerkleTrie> =
            parse_elf(elf_bytes.as_ref()).expect("error loading and parsing RISC-V instruction");
        vm.syscalls.set_input(&input);

        // TODO(collinjackson): Get outputs
        let completed_trace = trace(&mut vm, k as usize, false).expect("error generating trace");
        let tr = init_circuit_trace(completed_trace).expect("error initializing circuit trace");

        let total_steps = tr.steps();
        let start: usize = match to_prove.step_to_start {
            Some(step) => step as usize,
            None => 0,
        };
        let steps_to_prove = match to_prove.steps_to_prove {
            Some(steps) => steps as usize,
            None => total_steps,
        };
        let mut end: usize = start + steps_to_prove;
        if end > total_steps {
            end = total_steps
        }

        let initial_progress = ClientProgramProofRequest {
            steps_in_trace: total_steps as i32,
            steps_proven: 0,
            step_to_start: start as i32,
            program_id: String::new(),  // TODO: pass program id
            client_id_token: String::new(),  // TODO: pass client id token
            proof_duration_millis: 0,
            proof_speed_hz: 0.0,
        };

        // Send with error handling
        if let Err(e) = client.send(Message::Binary(initial_progress.encode_to_vec())).await {
            eprintln!("Failed to send progress update: {}", e);
            track(
                "send_error".into(),
                format!("Failed to send progress message: {}", e),
                &ws_addr_string,
                json!({
                    "prover_id": prover_id,
                    "error": e.to_string(),
                }),
            );
            // TODO: Depending on error type, might want to:
            // 1. Try to reconnect
            // 2. Return error
            // 3. Continue with next operation
        }

        let z_st = tr.input(start).expect("error starting circuit trace");
        let mut proof = IVCProof::new(&z_st);

        let mut completed_fraction = 0.0;
        let mut steps_proven = 0;
        track(
            "progress".into(),
            format!(
                "Program trace is {} steps. Proving {} steps starting at {}...",
                total_steps, steps_to_prove, start
            ),
            &ws_addr_string,
            json!({
                "completed_fraction": completed_fraction,
                "steps_in_trace": total_steps,
                "steps_to_prove": steps_to_prove,
                "steps_proven": steps_proven,
                "cycles_proven": steps_proven * k,
                "k": k,
                "prover_id": prover_id,
            }),
        );
        let start_time = Instant::now();
        let mut progress_time = start_time;
        for step in start..end {
            proof = prove_seq_step(Some(proof), &pp, &tr).expect("error proving step");
            steps_proven += 1;
            completed_fraction = steps_proven as f32 / steps_to_prove as f32;
  
            

            let progress_duration = progress_time.elapsed();
            let proof_cycles_hertz = k as f64 * 1000.0 / progress_duration.as_millis() as f64;
            
            let progress = ClientProgramProofRequest {
                steps_in_trace: total_steps as i32,
                steps_proven,
                step_to_start: start as i32,
                program_id: String::new(),  // TODO: pass program id
                client_id_token: String::new(),  // TODO: pass client id token
                proof_duration_millis: progress_duration.as_millis() as i32, // TODO: find proof_duration_millis
                proof_speed_hz: proof_cycles_hertz as f32, //TODO: find proof_cycles_hertz
            };
            
            track(
                "progress".into(),
                format!(
                    "Proved step {} at {:.2} proof cycles/sec.",
                    step, proof_cycles_hertz
                ),
                &ws_addr_string,
                json!({
                    "completed_fraction": completed_fraction,
                    "steps_in_trace": total_steps,
                    "steps_to_prove": steps_to_prove,
                    "steps_proven": steps_proven,
                    "cycles_proven": steps_proven * 4,
                    "k": k,
                    "progress_duration_millis": progress_duration.as_millis(),
                    "proof_cycles_hertz": proof_cycles_hertz,
                    "prover_id": prover_id,
                }),
            );
            progress_time = Instant::now();

            let mut retries = 0;
            let max_retries = 5;
            while let Err(e) = client.send(Message::Binary(progress.encode_to_vec())).await {
                eprintln!(
                    "Failed to send message: {:?}, attempt {}/{}",
                    e,
                    retries + 1,
                    max_retries
                );

                retries += 1;
                if retries >= max_retries {
                    eprintln!("Max retries reached, exiting...");
                    break;
                }

                // Add a delay before retrying
                tokio::time::sleep(tokio::time::Duration::from_secs(u64::pow(2, retries))).await;
            }

            if step == end - 1 {
                let mut buf = Vec::new();
                let mut writer = Box::new(&mut buf);
                let mut encoder = Encoder::new(&mut writer, 0).expect("failed to create encoder");
                proof
                    .serialize_compressed(&mut encoder)
                    .expect("failed to compress proof");
                encoder.finish().expect("failed to finish encoder");        
   
            }
        }
        // TODO(collinjackson): Consider verifying the proof before sending it
        // proof.verify(&public_params, proof.step_num() as _).expect("error verifying execution")

        if args.just_once {
            break;
        } else {
            println!("Waiting for another program to prove...");
        }
    }

    client
        .close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: Cow::Borrowed("Finished proving."),
        }))
        .await
        .map_err(|e| {
            track(
                "close_error".into(),
                "Failed to close WebSocket connection".into(),
                &ws_addr_string,
                json!({
                    "prover_id": &prover_id,
                    "error": e.to_string(),
                }),
            );
            format!("Failed to close WebSocket connection: {}", e)
        })?;
    track(
        "disconnect".into(),
        "Sent proof and closed connection...".into(),
        &ws_addr_string,
        json!({ "prover_id": prover_id }),
    );
    Ok(())
}
