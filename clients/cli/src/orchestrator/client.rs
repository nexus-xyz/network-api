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
///
/// Tuple components (in order):
/// 1. `Vec<u8>`: legacy single-proof bytes (set only when exactly one proof is present and the
///    server expects a single proof field; otherwise empty)
/// 2. `Vec<Vec<u8>>`: list of full proof byte blobs for multi-input proofs (empty for
///    `ProofHash`/`AllProofHashes`)
/// 3. `Vec<String>`: list of per-input proof hashes (used for `AllProofHashes`; empty otherwise)
pub(crate) type ProofPayload = (Vec<u8>, Vec<Vec<u8>>, Vec<String>);

/// Result of fetching a proof task, including both the task and its actual difficulty
#[derive(Debug, Clone)]
pub struct ProofTaskResult {
    pub task: Task,
    pub actual_difficulty: crate::nexus_orchestrator::TaskDifficulty,
}

impl std::fmt::Display for ProofTaskResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Task {} (difficulty: {:?})",
            self.task.task_id, self.actual_difficulty
        )
    }
}

// Build timestamp in milliseconds since epoch
static BUILD_TIMESTAMP: &str = match option_env!("BUILD_TIMESTAMP") {
    Some(timestamp) => timestamp,
    None => "Build timestamp not available",
};

// User-Agent string with CLI version
const USER_AGENT: &str = concat!("nexus-cli/", env!("CARGO_PKG_VERSION"));

// Privacy-preserving country detection for network optimization.
// Only stores 2-letter country codes (e.g., "US", "CA", "GB") to help route
// requests to the nearest Nexus network servers for better performance.
// No precise location, IP addresses, or personal data is collected or stored.
pub(crate) static COUNTRY_CODE: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct OrchestratorClient {
    client: Client,
    environment: Environment,
}

impl OrchestratorClient {
    pub fn new(environment: Environment) -> Self {
        Self {
            client: ClientBuilder::new()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
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

    /// Selects which proof data to attach based on the `task_type`.
    ///
    /// Returns a tuple `(legacy_proof, proofs, individual_proof_hashes)` with the appropriate
    /// fields populated:
    /// - For `ProofHash`: no proof bytes and no hashes (server derives hash elsewhere).
    /// - For `AllProofHashes`: no proof bytes; `individual_proof_hashes` populated.
    /// - For other types (e.g. `ProofRequired`): `legacy_proof` is set only when exactly
    ///   one proof is present (back-compat), and `proofs` contains the vector of full proofs.
    pub(crate) fn select_proof_payload(
        task_type: crate::nexus_orchestrator::TaskType,
        legacy_proof: Vec<u8>,
        proofs: Vec<Vec<u8>>,
        individual_proof_hashes: &[String],
    ) -> ProofPayload {
        match task_type {
            crate::nexus_orchestrator::TaskType::ProofHash => {
                // For ProofHash tasks, don't send proof or individual hashes
                (Vec::new(), Vec::new(), Vec::new())
            }
            crate::nexus_orchestrator::TaskType::AllProofHashes => {
                // For AllProofHashes tasks, don't send proof but send all individual hashes
                (Vec::new(), Vec::new(), individual_proof_hashes.to_vec())
            }
            _ => {
                // For ProofRequired and backward compatibility:
                // - Always include `proofs` as provided
                // - Include `legacy_proof` only when there is exactly one proof, for servers/paths
                //   that still expect a single legacy proof field
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

    /// Detects the user's country for network optimization purposes.
    ///
    /// Privacy Note: This only detects the country (2-letter code like "US", "CA", "GB")
    /// and does NOT track precise location, IP address, or any personally identifiable
    /// information. The country information helps the Nexus network route requests to
    /// the nearest servers for better performance and reduced latency.
    ///
    /// The detection is cached for the duration of the program run.
    async fn get_country(&self) -> String {
        if let Some(country) = COUNTRY_CODE.get() {
            return country.clone();
        }

        let country = self.detect_country().await;
        let _ = COUNTRY_CODE.set(country.clone());
        country
    }

    async fn detect_country(&self) -> String {
        // Try Cloudflare first (most reliable)
        if let Ok(country) = self.get_country_from_cloudflare().await {
            return country;
        }

        // Fallback to ipinfo.io
        if let Ok(country) = self.get_country_from_ipinfo().await {
            return country;
        }

        // If we can't detect the country, use the US as a fallback
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

/// Detect country code once globally without requiring a client instance.
/// This ensures callers don't need to sequence a warm-up before using the result.
pub(crate) async fn detect_country_once() -> String {
    if let Some(country) = COUNTRY_CODE.get() {
        return country.clone();
    }

    let client = match ClientBuilder::new().timeout(Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(_) => return "US".to_string(),
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

    // Default fallback
    let fallback = "US".to_string();
    let _ = COUNTRY_CODE.set(fallback.clone());
    fallback
}

#[async_trait::async_trait]
impl Orchestrator for OrchestratorClient {
    fn environment(&self) -> &Environment {
        &self.environment
    }

    /// Get the user ID associated with a wallet address.
    async fn get_user(&self, wallet_address: &str) -> Result<String, OrchestratorError> {
        let wallet_path = urlencoding::encode(wallet_address).into_owned();
        let endpoint = format!("v3/users/{}", wallet_path);
        let user_response: UserResponse = self.get_request(&endpoint).await?;
        Ok(user_response.user_id)
    }

    /// Registers a new user with the orchestrator.
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

    /// Registers a new node with the orchestrator.
    async fn register_node(&self, user_id: &str) -> Result<String, OrchestratorError> {
        let request = RegisterNodeRequest {
            node_type: NodeType::CliProver as i32,
            user_id: user_id.to_string(),
        };
        let request_bytes = Self::encode_request(&request);
        let response: RegisterNodeResponse = self.post_request("v3/nodes", request_bytes).await?;
        Ok(response.node_id)
    }

    /// Get the wallet address associated with a node ID.
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
    ) -> Result<ProofTaskResult, OrchestratorError> {
        let request = GetProofTaskRequest {
            node_id: node_id.to_string(),
            node_type: NodeType::CliProver as i32,
            ed25519_public_key: verifying_key.to_bytes().to_vec(),
            max_difficulty: max_difficulty as i32,
        };
        let request_bytes = Self::encode_request(&request);
        let response: GetProofTaskResponse = self.post_request("v3/tasks", request_bytes).await?;

        let task = Task::from(&response);
        let actual_difficulty = task.difficulty;

        Ok(ProofTaskResult {
            task,
            actual_difficulty,
        })
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

        // Detect country for network optimization (privacy-preserving: only country code, no precise location)
        let location = self.get_country().await;
        // Handle different task types
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
                flops_per_sec: Some(flops.clamp(i32::MIN as f64, i32::MAX as f64) as i32),
                memory_used: Some(program_memory),
                memory_capacity: Some(total_memory),
                // Country code for network routing optimization (privacy-preserving)
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

#[cfg(test)]
/// These are ignored by default since they require a live orchestrator to run.
mod live_orchestrator_tests {
    use crate::environment::Environment;
    use crate::orchestrator::Orchestrator;

    #[tokio::test]
    #[ignore] // This test requires a live orchestrator instance.
    /// Should register a new user with the orchestrator.
    async fn test_register_user() {
        let client = super::OrchestratorClient::new(Environment::Production);
        // UUIDv4 for the user ID
        let user_id = uuid::Uuid::new_v4().to_string();
        let wallet_address = "0x1234567890abcdef1234567890cbaabc12345678"; // Example wallet address
        match client.register_user(&user_id, wallet_address).await {
            Ok(_) => println!("User registered successfully: {}", user_id),
            Err(e) => panic!("Failed to register user: {}", e),
        }
    }

    #[tokio::test]
    #[ignore] // This test requires a live orchestrator instance.
    /// Should register a new node to an existing user.
    async fn test_register_node() {
        let client = super::OrchestratorClient::new(Environment::Production);
        let user_id = "78db0be7-f603-4511-9576-c660f3c58395";
        match client.register_node(user_id).await {
            Ok(node_id) => println!("Node registered successfully: {}", node_id),
            Err(e) => panic!("Failed to register node: {}", e),
        }
    }

    #[tokio::test]
    #[ignore] // This test requires a live orchestrator instance.
    /// Should return a new proof task for the node.
    async fn test_get_proof_task() {
        let client = super::OrchestratorClient::new(Environment::Production);
        let node_id = "5880437"; // Example node ID
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rand::thread_rng());
        let verifying_key = signing_key.verifying_key();
        let result = client
            .get_proof_task(
                node_id,
                verifying_key,
                crate::nexus_orchestrator::TaskDifficulty::SmallMedium,
            )
            .await;
        match result {
            Ok(task) => {
                println!("Got proof task: {}", task);
            }
            Err(e) => panic!("Failed to get proof task: {}", e),
        }
    }

    #[tokio::test]
    #[ignore] // This test requires a live orchestrator instance.
    /// Should return the user ID for a wallet address.
    async fn test_get_user() {
        let client = super::OrchestratorClient::new(Environment::Production);
        let wallet_address = "0x1234567890abcdef1234567890cbaabc12345678"; // Example wallet address
        match client.get_user(wallet_address).await {
            Ok(user_id) => println!("User ID: {}", user_id),
            Err(e) => panic!("Failed to get user: {}", e),
        }
    }

    #[tokio::test]
    #[ignore] // This test requires a live orchestrator instance.
    /// Should return the wallet address for a node ID.
    async fn test_get_node() {
        let client = super::OrchestratorClient::new(Environment::Production);
        let node_id = "5880437"; // Example node ID
        match client.get_node(node_id).await {
            Ok(wallet_address) => println!("Wallet address: {}", wallet_address),
            Err(e) => panic!("Failed to get node: {}", e),
        }
    }

    #[tokio::test]
    #[ignore] // This test requires a live orchestrator instance.
    /// Should detect the country for network optimization.
    async fn test_country_detection() {
        let client = super::OrchestratorClient::new(Environment::Production);
        let country = client.get_country().await;
        println!("Detected country: {}", country);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus_orchestrator::TaskType;

    #[tokio::test]
    /// select_proof_payload rules: only ProofRequired sets proof/proofs.
    async fn test_select_proof_payload() {
        // Common inputs
        let legacy = vec![9, 9, 9];
        let proofs_multi = vec![vec![1], vec![2]];
        let proofs_single = vec![vec![7]];
        let hashes = vec!["a".to_string(), "b".to_string()];

        // PROOF_HASH: both proof and proofs empty; no hashes
        let (p, ps, hs) = OrchestratorClient::select_proof_payload(
            TaskType::ProofHash,
            legacy.clone(),
            proofs_multi.clone(),
            &hashes,
        );
        assert!(p.is_empty());
        assert!(ps.is_empty());
        assert!(hs.is_empty());

        // ALL_PROOF_HASHES: both proof and proofs empty; hashes present
        let (p, ps, hs) = OrchestratorClient::select_proof_payload(
            TaskType::AllProofHashes,
            legacy.clone(),
            proofs_multi.clone(),
            &hashes,
        );
        assert!(p.is_empty());
        assert!(ps.is_empty());
        assert_eq!(hs, hashes);

        // PROOF_REQUIRED with multiple proofs: legacy proof empty, proofs set
        let (p, ps, hs) = OrchestratorClient::select_proof_payload(
            TaskType::ProofRequired,
            legacy.clone(),
            proofs_multi.clone(),
            &hashes,
        );
        assert!(p.is_empty());
        assert_eq!(ps, proofs_multi);
        assert!(hs.is_empty());

        // PROOF_REQUIRED with single proof: legacy proof set, proofs set with one element
        let (p, ps, hs) = OrchestratorClient::select_proof_payload(
            TaskType::ProofRequired,
            legacy.clone(),
            proofs_single.clone(),
            &hashes,
        );
        assert_eq!(p, legacy);
        assert_eq!(ps, proofs_single);
        assert!(hs.is_empty());
    }
}
