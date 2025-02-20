use crate::config;
use crate::flops::measure_flops;
use crate::memory_stats::get_memory_info;
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

        let friendly_connection_error =
            "[CONNECTION] Unable to reach server. The service might be temporarily unavailable."
                .to_string();
        let friendly_messages = match method {
            "POST" => match self
                .client
                .post(&url)
                .header("Content-Type", "application/octet-stream")
                .body(request_bytes)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(_) => return Err(friendly_connection_error.into()),
            },
            "GET" => match self.client.get(&url).send().await {
                Ok(resp) => resp,
                Err(_) => return Err(friendly_connection_error.into()),
            },
            _ => return Err("[METHOD] Unsupported HTTP method".into()),
        };

        if !friendly_messages.status().is_success() {
            let status = friendly_messages.status();
            let error_text = friendly_messages.text().await?;

            // Clean up error text by removing HTML
            let clean_error = if error_text.contains("<html>") {
                format!("HTTP {}", status.as_u16())
            } else {
                error_text
            };

            let friendly_message = match status.as_u16() {
                400 => "[400] Invalid request".to_string(),
                401 => "[401] Authentication failed. Please check your credentials.".to_string(),
                403 => "[403] You don't have permission to perform this action.".to_string(),
                404 => "[404] The requested resource was not found.".to_string(),
                408 => "[408] The server timed out waiting for your request. Please try again.".to_string(),
                429 => "[429] Too many requests. Please try again later.".to_string(),
                502 => "[502] Unable to reach the server. Please try again later.".to_string(),
                504 => "[504] Gateway Timeout: The server took too long to respond. Please try again later.".to_string(),
                500..=599 => format!("[{}] A server error occurred. Our team has been notified. Please try again later.", status),
                _ => format!("[{}] Unexpected error: {}", status, clean_error),
            };

            return Err(friendly_message.into());
        }

        let response_bytes = friendly_messages.bytes().await?;
        if response_bytes.is_empty() {
            return Ok(None);
        }

        match U::decode(response_bytes) {
            Ok(msg) => Ok(Some(msg)),
            Err(_e) => {
                // println!("Failed to decode response: {:?}", e);
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
        let (program_memory, total_memory) = get_memory_info();
        let flops = measure_flops();

        let request = SubmitProofRequest {
            node_id: node_id.to_string(),
            node_type: NodeType::CliProver as i32,
            proof_hash: proof_hash.to_string(),
            proof,
            node_telemetry: Some(crate::nexus_orchestrator::NodeTelemetry {
                flops_per_sec: Some(flops as i32),
                memory_used: Some(program_memory),
                memory_capacity: Some(total_memory),
                location: Some("US".to_string()),
            }),
        };

        self.make_request::<SubmitProofRequest, ()>("/tasks/submit", "POST", &request)
            .await?;

        Ok(())
    }
}
