use nexus_vm::elf::ElfFile;
use nexus_vm::trace::k_trace;
use nexus_vm_prover::prove;

pub async fn generate_and_send_proof(client: &mut impl ProverClient) -> Result<(), Box<dyn std::error::Error>> {
    
    // 1. Ask the orchestrator for a program name and its inputs
    let (program_name, inputs) = client.get_program_and_inputs().await;

    // 2. Use the program name to get the elf file from the files in the CLI
    let program_elf: ElfFile = get_program_elf(&program_name);

    // 3. Serialize inputs sent from the orchestrator
    let public_input_bytes: Vec<u8> = if let Some(input) = &io_args.public_input {
        to_allocvec(input).expect("Failed to serialize public input")
    } else {
        Vec::new()
    };

    // 4. Generate the trace using the elf file and the inputs
    let associated_data: Vec<u8> = Vec::new();
    let private_input_bytes: Vec<u8> = if let Some(input) = &io_args.private_input {
        to_allocvec(input).expect("Failed to serialize private input")
    } else {
        Vec::new()
    };
    let K = 1;

    let (view, execution_trace) = k_trace(
        program_elf,
        &associated_data,
        &public_input_bytes,
        &private_input_bytes,
        K,
    )
    .expect("error generating trace");

    // 5. Prove the program
    let proof = prove(&execution_trace, &view).unwrap();

    // 6. Serialize the proof
    let proof_bytes = postcard::to_allocvec(&proof).expect("Failed to serialize proof");

    // 7. Send the serialized proof to the orchestrator
    client.send_proof(proof_bytes).await;
    println!("Proof sent to the orchestrator");

    Ok(())
}

pub trait ProverClient {
    async fn get_program_and_inputs(&mut self) -> (String, Vec<u8>);
    async fn send_proof(&mut self, proof: Vec<u8>);
}