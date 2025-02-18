use crate::config;
use crate::nexus_orchestrator::{
    GetProofTaskRequest, GetProofTaskResponse, NodeType, SubmitProofRequest,
};
use prost::Message;
use reqwest::Client;

pub struct OrchestratorClient {
    client: Client,
    base_url: String,
    // environment: config::Environment,
}

impl OrchestratorClient {
    pub fn new(environment: config::Environment) -> Self {
        Self {
            client: Client::new(),
            base_url: environment.orchestrator_url(),
            // environment,
        }
    }

    async fn make_request<T, U>(
        &self,
        url: &str,
        method: &str,
        request_data: &T,
    ) -> Result<Option<U>, Box<dyn std::error::Error>>
    where
        T: Message,
        U: Message + Default,
    {
        let request_bytes = request_data.encode_to_vec();
        let url = format!("{}{}", self.base_url, url);

        let response = match method {
            "POST" => {
                self.client
                    .post(&url)
                    .header("Content-Type", "application/octet-stream")
                    .body(request_bytes)
                    .send()
                    .await?
            }
            "GET" => self.client.get(&url).send().await?,
            _ => return Err("Unsupported method".into()),
        };

        if !response.status().is_success() {
            return Err(format!(
                "Unexpected status {}: {}",
                response.status(),
                response.text().await?
            )
            .into());
        }

        let response_bytes = response.bytes().await?;
        if response_bytes.is_empty() {
            return Ok(None);
        }

        match U::decode(response_bytes) {
            Ok(msg) => Ok(Some(msg)),
            Err(e) => {
                println!("Failed to decode response: {:?}", e);
                Ok(None)
            }
        }
    }

    pub async fn get_proof_task(
        &self,
        node_id: &str,
    ) -> Result<GetProofTaskResponse, Box<dyn std::error::Error>> {
        let request = GetProofTaskRequest {
            node_id: node_id.to_string(),
            node_type: NodeType::CliProver as i32,
        };

        let response = self
            .make_request("/tasks", "POST", &request)
            .await?
            .ok_or("No response received from get_proof_task")?;

        Ok(response)
    }

    pub async fn submit_proof(
        &self,
        node_id: &str,
        proof_hash: &str,
        proof: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = SubmitProofRequest {
            node_id: node_id.to_string(),
            node_type: NodeType::CliProver as i32,
            proof_hash: proof_hash.to_string(),
            proof,
            node_telemetry: Some(crate::nexus_orchestrator::NodeTelemetry {
                flops_per_sec: Some(1),
                memory_used: Some(1),
                memory_capacity: Some(1),
                location: Some("US".to_string()),
            }),
        };

        let url = format!("{}/tasks/submit", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/octet-stream")
            .body(request.encode_to_vec())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!(
                "Unexpected status {}: {}",
                response.status(),
                response.text().await?
            )
            .into());
        }

        let response_text = response.text().await?;
        println!("\tNexus Orchestrator response: {}", response_text);
        Ok(())
    }
}
