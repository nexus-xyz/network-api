// Copyright (c) 2024 Nexus. All rights reserved.

mod analytics;
mod config;
mod generated;

use crate::analytics::track;

use std::borrow::Cow;

use clap::Parser;
use futures::{SinkExt, StreamExt};
use generated::pb::{
    self, compiled_program::Program, proof, prover_request, vm_program_input::Input, Progress,
    ProverRequest, ProverRequestRegistration, ProverResponse, ProverType,
};
use prost::Message as _;
use random_word::Lang;
use serde_json::json;
use std::{fs, path::Path, time::SystemTime};
use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message};
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
use std::env;
use zstd::stream::Encoder;
use rand::{ RngCore };

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
async fn main() {
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

    // If the prover_id file is found, use the contents, otherwise generate a new random id
    // and store it.
    let mut prover_id = format!(
        "{}-{}-{}",
        random_word::gen(Lang::En),
        random_word::gen(Lang::En),
        rand::thread_rng().next_u32() % 100,
    );
    match home::home_dir() {
        Some(path) if !path.as_os_str().is_empty() => {
            let nexus_dir = Path::new(&path).join(".nexus");
            prover_id = match fs::read(nexus_dir.join("prover-id")) {
                Ok(buf) => String::from_utf8(buf).unwrap(),
                Err(_) => {
                    let _ = fs::create_dir(nexus_dir.clone());
                    fs::write(nexus_dir.join("prover-id"), prover_id.clone()).unwrap();
                    prover_id
                }
            }
        }
        _ => {
            println!("Unable to get home dir.");
        }
    };

    track(
        "connect".into(),
        format!("Connecting to {}...", &ws_addr_string),
        &ws_addr_string,
        json!({"prover_id": prover_id}),
    );

    let (mut client, _) = tokio_tungstenite::connect_async(&ws_addr_string)
        .await
        .unwrap();

    track(
        "connected".into(),
        "Connected.".into(),
        &ws_addr_string,
        json!({"prover_id": prover_id}),
    );

    let registration = ProverRequest {
        contents: Some(prover_request::Contents::Registration(
            ProverRequestRegistration {
                prover_type: ProverType::Volunteer.into(),
                prover_id: prover_id.clone().into(),
                estimated_proof_cycles_hertz: None,
            },
        )),
    };

    client
        .send(Message::Binary(registration.encode_to_vec()))
        .await
        .unwrap();

    track(
        "register".into(),
        format!("Your assigned prover identifier is {}.", prover_id),
        &ws_addr_string,
        json!({"ws_addr_string": ws_addr_string, "prover_id": prover_id}),
    );
    println!(
        "Network stats are available at https://beta.nexus.xyz/."
    );
    loop {
        let program_message = match client.next().await.unwrap().unwrap() {
            Message::Binary(b) => b,
            _ => panic!("Unexpected message type"),
        };
        let program = ProverResponse::decode(program_message.as_slice()).unwrap();

        let Program::Rv32iElfBytes(elf_bytes) = program
            .to_prove
            .clone()
            .unwrap()
            .program
            .unwrap()
            .program
            .unwrap();
        let to_prove = program.to_prove.unwrap();
        let Input::RawBytes(input) = to_prove.input.unwrap().input.unwrap();

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
            parse_elf(&elf_bytes.as_ref()).expect("error loading and parsing RISC-V instruction");
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

        let initial_progress = ProverRequest {
            contents: Some(prover_request::Contents::Progress(Progress {
                completed_fraction: 0.0,
                steps_in_trace: total_steps as i32,
                steps_to_prove: (end - start) as i32,
                steps_proven: 0,
            })),
        };
        client
            .send(Message::Binary(initial_progress.encode_to_vec()))
            .await
            .unwrap();

        let z_st = tr.input(start).expect("error starting circuit trace");
        let mut proof = IVCProof::new(&z_st);

        let mut completed_fraction = 0.0;
        let mut steps_proven = 0;
        track(
            "progress".into(),
            format!(
                "Program trace is {} steps. Proving from {} to {}...",
                total_steps, start, end
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
        let start_time = SystemTime::now();
        let mut progress_time = start_time;
        for step in start..end {
            proof = prove_seq_step(Some(proof), &pp, &tr).expect("error proving step");
            steps_proven += 1;
            completed_fraction = steps_proven as f32 / steps_to_prove as f32;
            let progress = ProverRequest {
                contents: Some(prover_request::Contents::Progress(Progress {
                    completed_fraction: completed_fraction,
                    steps_in_trace: total_steps as i32,
                    steps_to_prove: steps_to_prove as i32,
                    steps_proven: steps_proven as i32,
                })),
            };
            let progress_duration = SystemTime::now().duration_since(progress_time).unwrap();
            let cycles_proven = steps_proven * 4;
            let proof_cycles_hertz = k * 1000 / progress_duration.as_millis();
            let proof_cycles_per_minute = k * 60 * 1000 / progress_duration.as_millis();
            track(
                "progress".into(),
                format!("Proved step {} at {} Hz.", step, proof_cycles_hertz),
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
                    "proof_cycles_per_minute": proof_cycles_per_minute,
                    "prover_id": prover_id,
                }),
            );
            progress_time = SystemTime::now();
            client
                .send(Message::Binary(progress.encode_to_vec()))
                .await
                .unwrap();
            if step == end - 1 {
                let mut buf = Vec::new();
                let mut writer = Box::new(&mut buf);
                let mut encoder = Encoder::new(&mut writer, 0).expect("failed to create encoder");
                proof
                    .serialize_compressed(&mut encoder)
                    .expect("failed to compress proof");
                encoder.finish().expect("failed to finish encoder");

                let response = ProverRequest {
                    contents: Some(prover_request::Contents::Proof(pb::Proof {
                        proof: Some(proof::Proof::NovaBytes(buf)),
                    })),
                };
                let duration = SystemTime::now().duration_since(start_time).unwrap();
                let proof_cycles_hertz = cycles_proven * 1000 / duration.as_millis();
                let proof_cycles_per_minute = cycles_proven * 60 * 1000 / duration.as_millis();
                client
                    .send(Message::Binary(response.encode_to_vec()))
                    .await
                    .unwrap();                                               
                track(
                    "proof".into(),
                    format!("Proof sent! You proved at {} Hz.", proof_cycles_hertz),
                    &ws_addr_string,
                    json!({
                        "proof_duration_sec": duration.as_secs(),
                        "proof_duration_millis": duration.as_millis(),
                        "proof_cycles_hertz": proof_cycles_hertz,
                        "proof_cycles_per_minute": proof_cycles_per_minute,
                        "prover_id": prover_id,
                    }),
                );
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
        .unwrap();
    track(
        "disconnect".into(),
        "Sent proof and closed connection...".into(),
        &ws_addr_string,
        json!({ "prover_id": prover_id }),
    );
}
