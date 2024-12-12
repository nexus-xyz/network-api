use crate::analytics::track;
use colored::Colorize;
use serde_json::json;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async_tls_with_config, Connector}; // 确保使用 Connector
use native_tls::TlsConnector;

pub async fn connect_to_orchestrator(
    ws_addr: &str,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Box<dyn std::error::Error + Send + Sync>> {
    // Parse the WebSocket address
    let request = ws_addr.into_client_request()?;

    // Create a custom TLS connector that disables certificate validation
    let tls_connector = TlsConnector::builder()
        .danger_accept_invalid_certs(true) // 忽略无效证书
        .danger_accept_invalid_hostnames(true) // 忽略主机名验证
        .build()?;

    // Use the Connector::NativeTls variant
    let connector = Connector::NativeTls(tls_connector);

    // Call connect_async_tls_with_config with the appropriate parameters
    let (client, _) = connect_async_tls_with_config(request, None, true, Some(connector))
        .await
        .map_err(|e| {
            let error_message = format!(
                "Failed to connect to orchestrator at {}: {}",
                ws_addr,
                e
            )
            .red();
            eprintln!("{}", error_message);
            // Log failure to analytics
            track(
                "orchestrator_connection_failed".to_string(),
                error_message.to_string(), // Convert ColoredString to String
                ws_addr,                   // WebSocket address
                json!({ "error": e.to_string() }), // Event properties
                true                        // Print description
            );
            e
        })?;

    // Log success connection event
    track(
        "orchestrator_connection_success".to_string(),
        "Successfully connected to orchestrator.".to_string(), // Description of the success
        ws_addr, // WebSocket address
        json!({}), // No additional properties
        true       // Print description
    );

    Ok(client)
}

pub async fn connect_to_orchestrator_with_infinite_retry(
    ws_addr: &str,
    prover_id: &str,
) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    let mut attempt = 1;

    loop {
        match connect_to_orchestrator(ws_addr).await {
            Ok(client) => {
                println!("\t✓ Connected to Nexus Network.");

                track(
                    "connected".into(),
                    "Connected.".into(),
                    ws_addr,
                    json!({"prover_id": prover_id}),
                    false,
                );
                return client;
            }
            Err(_e) => {
                eprintln!(
                    "Could not connect to orchestrator (attempt {}). Retrying in {} seconds...",
                    attempt,
                    2u64.pow(attempt.min(6)),
                );

                tokio::time::sleep(tokio::time::Duration::from_secs(2u64.pow(attempt.min(6))))
                    .await;

                attempt += 1;
            }
        }
    }
}

pub async fn connect_to_orchestrator_with_limited_retry(
    ws_addr: &str,
    prover_id: &str,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Box<dyn std::error::Error + Send + Sync>> {
    let max_attempts = 5;
    let mut attempt = 1;

    loop {
        if attempt >= max_attempts {
            return Err(format!("Failed to connect after {} attempts", max_attempts).into());
        }

        match connect_to_orchestrator(ws_addr).await {
            Ok(client) => {
                track(
                    "connected".into(),
                    "Connected.".into(),
                    ws_addr,
                    json!({"prover_id": prover_id}),
                    false,
                );
                println!("{}", "✓ Success! Connected to Nexus Network.\n".green());
                return Ok(client);
            }
            Err(e) => {
                if attempt >= max_attempts {
                    return Err(format!(
                        "Failed to connect after {} attempts: {}",
                        max_attempts, e
                    )
                    .into());
                }

                eprintln!(
                    "Could not connect to orchestrator (attempt {}/{}). Retrying in {} seconds...",
                    attempt,
                    max_attempts,
                    2u64.pow(attempt.min(6)),
                );

                tokio::time::sleep(tokio::time::Duration::from_secs(2u64.pow(attempt.min(6))))
                    .await;

                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::connect_to_orchestrator; // Just the function we're testing
    use futures::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;

    #[tokio::test]
    async fn test_basic_connection() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Setup mock server
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let ws_addr = format!("ws://{}/prove", addr);

        // Spawn server that sends a test message
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;

            // SinkExt::send is available because of the import above
            ws_stream.send(Message::Text("test".into())).await?;

            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        });

        // Connect client
        let mut client = connect_to_orchestrator(&ws_addr).await?;

        // StreamExt::next is available for receiving
        if let Some(msg) = client.next().await {
            assert_eq!(msg?.into_text()?, "test");
        } else {
            panic!("No message received");
        }

        server_handle.await??;
        Ok(())
    }
}
