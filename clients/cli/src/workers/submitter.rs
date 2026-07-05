//! Proof submission with network retry logic

use super::core::{EventSender, WorkerConfig};
use crate::analytics::{
    track_proof_accepted, track_proof_submission_error, track_proof_submission_success,
};
use crate::consts::cli_consts::{proof_submission, rate_limiting};
use crate::events::EventType;
use crate::logging::LogLevel;
use crate::network::{NetworkClient, ProofSubmission, RequestTimer, RequestTimerConfig};
use crate::orchestrator::Orchestrator;
use crate::prover::ProverResult;
use crate::task::Task;
use ed25519_dalek::SigningKey;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SubmitError {
    #[error("Network error: {0}")]
    Network(#[from] crate::orchestrator::error::OrchestratorError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] postcard::Error),
    #[error("No proofs available to submit: proof generation produced an empty result")]
    NoProofs,
}

/// Proof submitter with built-in retry and error handling
pub struct ProofSubmitter {
    signing_key: SigningKey,
    orchestrator: Box<dyn Orchestrator>,
    network_client: NetworkClient,
    event_sender: EventSender,
    config: WorkerConfig,
}

impl ProofSubmitter {
    pub fn new(
        signing_key: SigningKey,
        orchestrator: Box<dyn Orchestrator>,
        event_sender: EventSender,
        config: &WorkerConfig,
    ) -> Self {
        // Configure request timer for proof submission
        let timer_config = RequestTimerConfig::combined(
            proof_submission::rate_limit_interval(),
            rate_limiting::SUBMISSION_MAX_REQUESTS_PER_WINDOW,
            rate_limiting::submission_window(),
            proof_submission::initial_backoff(), // Use as default retry delay
        );
        let request_timer = RequestTimer::new(timer_config);

        // Create network client with more retries for critical submissions
        let network_client = NetworkClient::new(request_timer, proof_submission::MAX_RETRIES);

        Self {
            signing_key,
            orchestrator,
            network_client,
            event_sender,
            config: config.clone(),
        }
    }

    /// Submit proof with automatic retry and proper logging
    pub async fn submit_proof(
        &mut self,
        task: &Task,
        proof_result: &ProverResult,
    ) -> Result<(), SubmitError> {
        // Log start of submission
        self.event_sender
            .send_proof_event(
                format!("Step 3 of 4: Submitting proof for task {}...", task.task_id),
                EventType::StateChange,
                LogLevel::Info,
            )
            .await;

        // Serialize proofs
        let proofs_bytes: Vec<Vec<u8>> = proof_result
            .proofs
            .iter()
            .map(postcard::to_allocvec)
            .collect::<Result<_, _>>()?;
        // Fail explicitly rather than forwarding empty bytes as the proof payload.
        // An empty submission is silently rejected by the orchestrator, causing the node
        // to lose the task reward without any visible error.
        if proofs_bytes.is_empty() {
            return Err(SubmitError::NoProofs);
        }
        let legacy_proof_bytes = proofs_bytes[0].clone();

        // Submit through network client with retry logic
        let mut submission = ProofSubmission::new(
            task.task_id.clone(),
            proof_result.combined_hash.clone(),
            legacy_proof_bytes,
            task.task_type,
        );

        // Populate individual hashes for ALL_PROOF_HASHES and optionally for ProofHash
        if task.task_type == crate::nexus_orchestrator::TaskType::AllProofHashes {
            submission =
                submission.with_individual_hashes(proof_result.individual_proof_hashes.clone());
        }

        // Populate proofs for PROOF_REQUIRED; leave empty otherwise
        if task.task_type == crate::nexus_orchestrator::TaskType::ProofRequired {
            submission = submission.with_proofs(proofs_bytes);
        }

        match self
            .network_client
            .submit_proof(
                self.orchestrator.as_ref(),
                submission,
                self.signing_key.clone(),
                1, // num_provers (single worker)
            )
            .await
        {
            Ok(attempts) => {
                // Log successful submission with attempt count
                let attempt_text = if attempts == 1 {
                    "".to_string()
                } else {
                    format!(" (after {} attempts)", attempts)
                };

                self.event_sender
                    .send_proof_event(
                        format!(
                            "Step 4 of 4: Proof submitted successfully for task {}{}\n",
                            task.task_id, attempt_text
                        ),
                        EventType::Success,
                        LogLevel::Info,
                    )
                    .await;

                // Track analytics for successful submission
                self.track_successful_submission(task).await;

                // Reporting now handled inside analytics success functions

                Ok(())
            }
            Err((e, attempts)) => {
                // Log submission failure with attempt count and appropriate level
                let log_level = self.network_client.classify_error(&e);
                self.event_sender
                    .send_proof_event(
                        format!(
                            "Failed to submit proof for task {} after {} attempts: {}",
                            task.task_id, attempts, e
                        ),
                        EventType::Error,
                        log_level,
                    )
                    .await;

                // Track analytics for submission error
                tokio::spawn(track_proof_submission_error(
                    task.clone(),
                    e.to_string(),
                    None,
                    self.config.environment.clone(),
                    self.config.client_id.clone(),
                ));

                Err(SubmitError::Network(e))
            }
        }
    }

    /// Track successful submission analytics based on task type
    async fn track_successful_submission(&self, task: &Task) {
        if task.task_type == crate::nexus_orchestrator::TaskType::ProofHash {
            tokio::spawn(track_proof_accepted(
                task.clone(),
                self.config.environment.clone(),
                self.config.client_id.clone(),
            ));
        } else {
            tokio::spawn(track_proof_submission_success(
                task.clone(),
                self.config.environment.clone(),
                self.config.client_id.clone(),
            ));
        }
    }
}
