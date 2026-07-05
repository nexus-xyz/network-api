//! Proving pipeline that orchestrates the full proving process

use std::sync::Arc;

use super::engine::ProvingEngine;
use super::input::InputParser;
use super::types::ProverError;
use crate::analytics::track_verification_failed;
use crate::environment::Environment;
use crate::task::Task;
use futures::future::join_all;
use nexus_sdk::stwo::seq::Proof;
use sha3::{Digest, Keccak256};
use tokio_util::sync::CancellationToken;

/// Orchestrates the complete proving pipeline
pub struct ProvingPipeline;

impl ProvingPipeline {
    /// Execute authenticated proving for a task
    pub async fn prove_authenticated(
        task: &Task,
        environment: &Environment,
        client_id: &str,
        num_workers: usize,
    ) -> Result<(Vec<Proof>, String, Vec<String>), ProverError> {
        match task.program_id.as_str() {
            "fib_input_initial" => {
                Self::prove_fib_task(task, environment, client_id, num_workers).await
            }
            _ => Err(ProverError::MalformedTask(format!(
                "Unsupported program ID: {}",
                task.program_id
            ))),
        }
    }

    /// Process fibonacci proving task with multiple inputs
    async fn prove_fib_task(
        task: &Task,
        environment: &Environment,
        client_id: &str,
        num_workers: usize,
    ) -> Result<(Vec<Proof>, String, Vec<String>), ProverError> {
        let all_inputs = task.all_inputs();

        if all_inputs.is_empty() {
            return Err(ProverError::MalformedTask(
                "No inputs provided for task".to_string(),
            ));
        }

        // Create shared references to avoid unnecessary cloning
        let task_shared = Arc::new(task.clone());
        let environment_shared = Arc::new(environment.clone());
        let client_id_shared = Arc::new(client_id.to_string());

        // Create a semaphore with a specific number of permits
        let semaphore = Arc::new(tokio::sync::Semaphore::new(num_workers));

        // Create cancellation token for graceful shutdown
        let cancellation_token = CancellationToken::new();

        // Spawn all tasks in parallel
        let handles: Vec<_> = all_inputs
            .iter()
            .enumerate()
            .map(|(input_index, input_data)| {
                let task_ref = Arc::clone(&task_shared);
                let environment_ref = Arc::clone(&environment_shared);
                let client_id_ref = Arc::clone(&client_id_shared);
                let input_data = input_data.clone();
                let semaphore_ref = Arc::clone(&semaphore);
                let cancellation_ref = cancellation_token.clone();

                tokio::spawn(async move {
                    // Check for cancellation before starting
                    if cancellation_ref.is_cancelled() {
                        return Err(ProverError::MalformedTask("Task cancelled".to_string()));
                    }

                    // Acquire a permit from the semaphore. This waits if the limit is reached.
                    let _permit = semaphore_ref.acquire_owned().await;

                    // Check for cancellation after acquiring permit
                    if cancellation_ref.is_cancelled() {
                        return Err(ProverError::MalformedTask("Task cancelled".to_string()));
                    }

                    // Step 1: Parse and validate input
                    let inputs = InputParser::parse_triple_input(&input_data)?;

                    // Step 2: Generate and verify proof
                    let proof = ProvingEngine::prove_and_validate(
                        &inputs,
                        &task_ref,
                        &environment_ref,
                        &client_id_ref,
                        num_workers,
                    )
                    .await?;

                    // Step 3: Generate proof hash
                    let proof_hash = Self::generate_proof_hash(&proof);

                    Ok((proof, proof_hash, input_index))
                })
            })
            .collect();

        // Use join_all for better parallelization
        let results = join_all(handles).await;

        // Process results and collect verification failures for batch handling
        let mut all_proofs = Vec::new();
        let mut proof_hashes = Vec::new();
        let mut verification_failures = Vec::new();

        for (result_index, result) in results.into_iter().enumerate() {
            match result {
                Ok(Ok((proof, proof_hash, _input_index))) => {
                    all_proofs.push(proof);
                    proof_hashes.push(proof_hash);
                }
                Ok(Err(e)) => {
                    // Collect verification failures for batch processing
                    match e {
                        ProverError::Stwo(_) | ProverError::GuestProgram(_) => {
                            verification_failures.push((
                                task_shared.clone(),
                                format!("Input {}: {}", result_index, e),
                                environment_shared.clone(),
                                client_id_shared.clone(),
                            ));
                        }
                        _ => {
                            // Cancel remaining tasks on critical errors
                            cancellation_token.cancel();
                            return Err(e);
                        }
                    }
                }
                Err(join_error) => {
                    return Err(ProverError::JoinError(join_error));
                }
            }
        }

        // Handle all verification failures in batch (avoid nested spawns)
        let failure_count = verification_failures.len();
        for (task, error_msg, env, client) in verification_failures {
            tokio::spawn(track_verification_failed(
                (*task).clone(),
                error_msg,
                (*env).clone(),
                (*client).clone(),
            ));
        }

        // If we have verification failures, we still return an error
        if failure_count > 0 {
            return Err(ProverError::MalformedTask(format!(
                "{} inputs failed verification",
                failure_count
            )));
        }

        let final_proof_hash = Self::combine_proof_hashes(&task_shared, &proof_hashes);

        Ok((all_proofs, final_proof_hash, proof_hashes))
    }

    /// Generate hash for a proof
    fn generate_proof_hash(proof: &Proof) -> String {
        let proof_bytes = postcard::to_allocvec(proof).expect("Failed to serialize proof");
        format!("{:x}", Keccak256::digest(&proof_bytes))
    }

    /// Combine multiple proof hashes based on task type
    fn combine_proof_hashes(task: &Task, proof_hashes: &[String]) -> String {
        match task.task_type {
            crate::nexus_orchestrator::TaskType::AllProofHashes
            | crate::nexus_orchestrator::TaskType::ProofHash => {
                Task::combine_proof_hashes(proof_hashes)
            }
            _ => proof_hashes.first().cloned().unwrap_or_default(),
        }
    }
}
