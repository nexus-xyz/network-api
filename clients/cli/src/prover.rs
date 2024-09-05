// Copyright (c) 2024 Nexus. All rights reserved.

mod generated;

use std::borrow::Cow;

use clap::Parser;
use futures::{SinkExt, StreamExt};
use generated::pb::{
    self, compiled_program::Program, proof, prover_request, vm_program_input::Input, Progress,
    ProverRequest, ProverRequestRegistration, ProverResponse, ProverType,
};
use prost::Message as _;
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
        init_circuit_trace,
        key::{CanonicalSerialize},
        prove_seq_step,
        types::*,
        pp::gen_vm_pp,
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

    /// Whether to connect using secure web sockets
    #[arg(short, long, default_value_t = true)]
    use_https: bool,

    /// Whether to loop and keep the connection open
    #[arg(short, long, default_value_t = true)]
    keep_listening: bool,
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
        if args.use_https { "wss" } else { "ws" },
        args.hostname,
        args.port
    );

    let prover_id = format!("prover_{}", rand::random::<u32>());

    let k = 4;
    // TODO(collinjackson): Get parameters from a file or URL.
    let pp = gen_vm_pp::<C1, seq::SetupParams<(G1, G2, C1, C2, RO, SC)>>(k as usize, &())
        .expect("error generating public parameters");

    println!(
        "{} supplying proofs to Orchestrator at {}",
        prover_id, &ws_addr_string
    );

    let registration = ProverRequest {
        contents: Some(prover_request::Contents::Registration(
            ProverRequestRegistration {
                prover_type: ProverType::Volunteer.into(),
                prover_id: prover_id,
                estimated_proof_cycles_hertz: None,
            },
        )),
    };

    let (mut client, _) = tokio_tungstenite::connect_async(ws_addr_string)
        .await
        .unwrap();

    client
        .send(Message::Binary(registration.encode_to_vec()))
        .await
        .unwrap();
    println!("Sent registration message...");

    loop {
        let program_message = match client.next().await.unwrap().unwrap() {
            Message::Binary(b) => b,
            _ => panic!("Unexpected message type"),
        };
        println!("Received program message...");

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

        println!(
            "Received a {} byte program to prove with {} bytes of input",
            elf_bytes.len(),
            input.len()
        );

        let mut vm: NexusVM<MerkleTrie> =
            parse_elf(&elf_bytes.as_ref()).expect("error loading and parsing RISC-V instruction");
        vm.syscalls.set_input(&input);

        let k = 4;
        // TODO(collinjackson): Get outputs
        let completed_trace = trace(&mut vm, k as usize, false).expect("error generating trace");
        let tr = init_circuit_trace(completed_trace).expect("error initializing circuit trace");

        let total_steps = tr.steps();
        println!("Program trace {} steps", total_steps);
        let start: usize = match to_prove.step_to_start {
            Some(step) => step as usize,
            None => 0,
        };
        let steps_to_prove = to_prove.steps_to_prove;
        let mut end: usize = match steps_to_prove {
            Some(steps) => start + steps as usize,
            None => total_steps,
        };
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
        println!("Proving...");
        let mut proof = IVCProof::new(&z_st);

        println!("Proving from {} to {}", start, end);
        for step in start..end {
            proof = prove_seq_step(Some(proof), &pp, &tr).expect("error proving step");
            println!("Proved step {}", step);
            let steps_proven = step - start + 1;
            let progress = ProverRequest {
                contents: Some(prover_request::Contents::Progress(Progress {
                    completed_fraction: steps_proven as f32 / steps_to_prove.unwrap() as f32,
                    steps_in_trace: total_steps as i32,
                    steps_to_prove: (end - start) as i32,
                    steps_proven: steps_proven as i32,
                })),
            };
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
                client
                    .send(Message::Binary(response.encode_to_vec()))
                    .await
                    .unwrap();
            }
        }
        // TODO(collinjackson): Consider verifying the proof before sending it
        // proof.verify(&public_params, proof.step_num() as _).expect("error verifying execution")

        println!("Proof sent!");

        if args.keep_listening {
            println!("Waiting for another program to prove...");
        } else {
            break;
        }
    }

    client
        .close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: Cow::Borrowed("Finished proving."),
        }))
        .await
        .unwrap();
    println!("Sent proof and closed connection...");
}
