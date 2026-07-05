#![allow(clippy::unused_async)]
//! Nexus Orchestrator Client
//!
//! A client for the Nexus Orchestrator, allowing for proof task retrieval and submission.
use crate::environment::Environment;
use crate::nexus_orchestrator::{
    GetProofTaskRequest, GetProofTaskResponse, NodeType, RegisterNodeRequest, RegisterNodeResponse,
    RegisterUserRequest, SubmitProofRequest, UserResponse,
};
use crate::orchestrator::Orchestrator;
use crate::orchestrator::error::OrchestratorError;
use crate::system::{estimate_peak_gflops, get_memory_info};
use crate::task::Task;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use prost::Message;
use reqwest::{Client, ClientBuilder, Response};
use std::sync::OnceLock;
use std::time::Duration;

/// Proof payload returned by `select_proof_payload`.
pub(crate) type ProofPayload = (Vec<u8>, Vec<Vec<u8>>, Vec<String>);

// Build timestamp
static BUILD_TIMESTAMP: &str = match option_env!("BUILD_TIMESTAMP") {
    Some(timestamp) => timestamp,
    None => "Build timestamp not available",
};

const USER_AGENT: &str = concat!("nexus-cli/", env!("CARGO_PKG_VERSION"));

pub(crate) static COUNTRY_CODE: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct OrchestratorClient {
    client: Client,
    environment: Environment,
}

impl OrchestratorClient {
    pub fn new(environment: Environment) -> Self {
        // Try to build an HTTP client with custom timeouts; if building fails,
        // log a warning and fall back to a default client to avoid panicking.
        let client = ClientBuilder::new()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|e| {
                eprintln!(
                    "Warning: failed to build reqwest Client with custom settings: {}; falling back to default Client",
                    e
                );
                Client::new()
            });

        Self {
            client,
            environment,
        }
    }

    /// Public accessor for privacy-preserving country code (cached during run)
    #[allow(dead_code)]
    pub async fn country(&self) -> String {
        self.get_country().await
    }

    fn build_url(&self, endpoint: &str) -> String {
        format!(
            "{}/{}",
            self.environment.orchestrator_url().trim_end_matches('/'),
            endpoint.trim_start_matches('/')
        )
    }

    fn encode_request<T: Message>(request: &T) -> Vec<u8> {
        request.encode_to_vec()
    }

    fn decode_response<T: Message + Default>(bytes: &[u8]) -> Result<T, OrchestratorError> {
        T::decode(bytes).map_err(OrchestratorError::Decode)
    }

    pub(crate) fn select_proof_payload(
        task_type: crate::nexus_orchestrator::TaskType,
        legacy_proof: Vec<u8>,
        proofs: Vec<Vec<u8>>,
        individual_proof_hashes: &[String],
    ) -> ProofPayload {
        match task_type {
            crate::nexus_orchestrator::TaskType::ProofHash => (Vec::new(), Vec::new(), Vec::new()),
            crate::nexus_orchestrator::TaskType::AllProofHashes => {
                (Vec::new(), Vec::new(), individual_proof_hashes.to_vec())
            }
            _ => {
                let legacy = if proofs.len() == 1 {
                    legacy_proof
                } else {
                    Vec::new()
                };
                (legacy, proofs, Vec::new())
            }
        }
    }

    async fn handle_response_status(response: Response) -> Result<Response, OrchestratorError> {
        if !response.status().is_success() {
            return Err(OrchestratorError::from_response(response).await);
        }
        Ok(response)
    }

    async fn get_request<T: Message + Default>(
        &self,
        endpoint: &str,
    ) -> Result<T, OrchestratorError> {
        let url = self.build_url(endpoint);
        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .header("X-Build-Timestamp", BUILD_TIMESTAMP)
            .send()
            .await?;
        let response = Self::handle_response_status(response).await?;
        let response_bytes = response.bytes().await?;
        Self::decode_response(&response_bytes)
    }

    async fn post_request<T: Message + Default>(
        &self,
        endpoint: &str,
        body: Vec<u8>,
    ) -> Result<T, OrchestratorError> {
        let url = self.build_url(endpoint);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .header("User-Agent", USER_AGENT)
            .header("X-Build-Timestamp", BUILD_TIMESTAMP)
            .body(body)
            .send()
            .await?;
        let response = Self::handle_response_status(response).await?;
        let response_bytes = response.bytes().await?;
        Self::decode_response(&response_bytes)
    }

    async fn post_request_no_response(
        &self,
        endpoint: &str,
        body: Vec<u8>,
    ) -> Result<(), OrchestratorError> {
        let url = self.build_url(endpoint);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .header("User-Agent", USER_AGENT)
            .header("X-Build-Timestamp", BUILD_TIMESTAMP)
            .body(body)
            .send()
            .await?;
        Self::handle_response_status(response).await?;
        Ok(())
    }

    fn create_signature(
        &self,
        signing_key: &SigningKey,
        task_id: &str,
        proof_hash: &str,
    ) -> (Vec<u8>, Vec<u8>) {
        let signature_version = 0;
        let msg = format!("{} | {} | {}", signature_version, task_id, proof_hash);
        let signature = signing_key.sign(msg.as_bytes());
        let verifying_key: VerifyingKey = signing_key.verifying_key();
        (
            signature.to_bytes().to_vec(),
            verifying_key.to_bytes().to_vec(),
        )
    }

    async fn get_country(&self) -> String {
        if let Some(country) = COUNTRY_CODE.get() {
            return country.clone();
        }
        let country = self.detect_country().await;
        let _ = COUNTRY_CODE.set(country.clone());
        country
    }

    async fn detect_country(&self) -> String {
        if let Ok(country) = self.get_country_from_cloudflare().await {
            return country;
        }
        if let Ok(country) = self.get_country_from_ipinfo().await {
            return country;
        }
        "US".to_string()
    }

    async fn get_country_from_cloudflare(&self) -> Result<String, Box<dyn std::error::Error>> {
        let response = self
            .client
            .get("https://cloudflare.com/cdn-cgi/trace")
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        let text = response.text().await?;
        for line in text.lines() {
            if let Some(country) = line.strip_prefix("loc=") {
                let country = country.trim().to_uppercase();
                if country.len() == 2 && country.chars().all(|c| c.is_ascii_alphabetic()) {
                    return Ok(country);
                }
            }
        }
        Err("Country not found in Cloudflare response".into())
    }

    async fn get_country_from_ipinfo(&self) -> Result<String, Box<dyn std::error::Error>> {
        let response = self
            .client
            .get("https://ipinfo.io/country")
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        let country = response.text().await?;
        let country = country.trim().to_uppercase();
        if country.len() == 2 && country.chars().all(|c| c.is_ascii_alphabetic()) {
            Ok(country)
        } else {
            Err("Invalid country code from ipinfo.io".into())
        }
    }
}

pub(crate) async fn detect_country_once() -> String {
    if let Some(country) = COUNTRY_CODE.get() {
        return country.clone();
    }

    let client = match ClientBuilder::new().timeout(Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(_) => {
            let fallback = "US".to_string();
            let _ = COUNTRY_CODE.set(fallback.clone());
            return fallback;
        }
    };

    // Try Cloudflare first
    if let Ok(response) = client
        .get("https://cloudflare.com/cdn-cgi/trace")
        .send()
        .await
    {
        if let Ok(text) = response.text().await {
            for line in text.lines() {
                if let Some(country) = line.strip_prefix("loc=") {
                    let country = country.trim().to_uppercase();
                    if country.len() == 2 && country.chars().all(|c| c.is_ascii_alphabetic()) {
                        let _ = COUNTRY_CODE.set(country.clone());
                        return country;
                    }
                }
            }
        }
    }

    // Fallback to ipinfo.io
    if let Ok(response) = client.get("https://ipinfo.io/country").send().await {
        if let Ok(text) = response.text().await {
            let country = text.trim().to_uppercase();
            if country.len() == 2 && country.chars().all(|c| c.is_ascii_alphabetic()) {
                let _ = COUNTRY_CODE.set(country.clone());
                return country;
            }
        }
    }

    // Default fallback if everything else failed
    let fallback = "US".to_string();
    let _ = COUNTRY_CODE.set(fallback.clone());
    fallback
}

#[async_trait::async_trait]
impl Orchestrator for OrchestratorClient {
    fn environment(&self) -> &Environment {
        &self.environment
    }

    async fn get_user(&self, wallet_address: &str) -> Result<String, OrchestratorError> {
        let wallet_path = urlencoding::encode(wallet_address).into_owned();
        let endpoint = format!("v3/users/{}", wallet_path);
        let user_response: UserResponse = self.get_request(&endpoint).await?;
        Ok(user_response.user_id)
    }

    async fn register_user(
        &self,
        user_id: &str,
        wallet_address: &str,
    ) -> Result<(), OrchestratorError> {
        let request = RegisterUserRequest {
            uuid: user_id.to_string(),
            wallet_address: wallet_address.to_string(),
        };
        let request_bytes = Self::encode_request(&request);
        self.post_request_no_response("v3/users", request_bytes)
            .await
    }

    async fn register_node(&self, user_id: &str) -> Result<String, OrchestratorError> {
        let request = RegisterNodeRequest {
            node_type: NodeType::CliProver as i32,
            user_id: user_id.to_string(),
        };
        let request_bytes = Self::encode_request(&request);
        let response: RegisterNodeResponse = self.post_request("v3/nodes", request_bytes).await?;
        Ok(response.node_id)
    }

    async fn get_node(&self, node_id: &str) -> Result<String, OrchestratorError> {
        let endpoint = format!("v3/nodes/{}", node_id);
        let node_response: crate::nexus_orchestrator::GetNodeResponse =
            self.get_request(&endpoint).await?;
        Ok(node_response.wallet_address)
    }

    async fn get_proof_task(
        &self,
        node_id: &str,
        verifying_key: VerifyingKey,
        max_difficulty: crate::nexus_orchestrator::TaskDifficulty,
    ) -> Result<Task, OrchestratorError> {
        let request = GetProofTaskRequest {
            node_id: node_id.to_string(),
            node_type: NodeType::CliProver as i32,
            ed25519_public_key: verifying_key.to_bytes().to_vec(),
            max_difficulty: max_difficulty as i32,
        };
        let request_bytes = Self::encode_request(&request);
        let response: GetProofTaskResponse = self.post_request("v3/tasks", request_bytes).await?;
        Ok(Task::from(&response))
    }

    async fn submit_proof(
        &self,
        task_id: &str,
        proof_hash: &str,
        proof: Vec<u8>,
        proofs: Vec<Vec<u8>>,
        signing_key: SigningKey,
        num_provers: usize,
        task_type: crate::nexus_orchestrator::TaskType,
        individual_proof_hashes: &[String],
    ) -> Result<(), OrchestratorError> {
        let (program_memory, total_memory) = get_memory_info();
        let flops = estimate_peak_gflops(num_provers);
        let (signature, public_key) = self.create_signature(&signing_key, task_id, proof_hash);
        let location = self.get_country().await;
        let (proof_to_send, proofs_to_send, all_proof_hashes_to_send) =
            OrchestratorClient::select_proof_payload(
                task_type,
                proof,
                proofs,
                individual_proof_hashes,
            );
        let request = SubmitProofRequest {
            task_id: task_id.to_string(),
            node_type: NodeType::CliProver as i32,
            proof_hash: proof_hash.to_string(),
            proof: proof_to_send,
            proofs: proofs_to_send,
            node_telemetry: Some(crate::nexus_orchestrator::NodeTelemetry {
                flops_per_sec: Some(flops as i32),
                memory_used: Some(program_memory),
                memory_capacity: Some(total_memory),
                location: Some(location),
            }),
            ed25519_public_key: public_key,
            signature,
            all_proof_hashes: all_proof_hashes_to_send,
        };
        let request_bytes = Self::encode_request(&request);
        self.post_request_no_response("v3/tasks/submit", request_bytes)
            .await
    }
}
