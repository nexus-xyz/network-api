
use serde_json::json;
use tokio_tungstenite::{
    // WebSocketStream: Manages WebSocket protocol (messages, frames, etc.)
    // - Built on top of TcpStream
    // - Handles WebSocket handshake
    // - Provides async send/receive
    WebSocketStream,

    // MaybeTlsStream: Wrapper for secure/insecure connections
    // - Handles both ws:// and wss:// URLs
    // - Provides TLS encryption when needed
    MaybeTlsStream,
};
use crate::track; //the analytics module
use tokio::net::TcpStream;  // Async TCP connection - the base transport layer
// WebSocket protocol types for message handling
use tokio_tungstenite::tungstenite::protocol::{
    Message,                  // Different types of WebSocket messages (Binary, Text, Ping, etc.)
};
use futures::StreamExt;


pub async fn receive_program_message(
    client: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    ws_addr: &str,
    prover_id: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    match client.next().await {
         // Stream has ended (connection closed)
        None => {
            Err("WebSocket connection closed unexpectedly".into())
        },
        Some(Ok(Message::Binary(bytes))) => Ok(bytes),
        Some(Ok(other)) => {
            track(
                "unexpected_message".into(),
                "Unexpected message type".into(),
                ws_addr,
                json!({ 
                    "prover_id": prover_id,
                    "message_type": format!("{:?}", other) 
                }),
            );
            Err("Unexpected message type".into())
        },
        Some(Err(e)) => {
            track(
                "websocket_error".into(),
                format!("WebSocket error: {}", e),
                ws_addr,
                json!({
                    "prover_id": prover_id,
                    "error": e.to_string(),
                }),
            );
            Err(format!("WebSocket error: {}", e).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::SinkExt;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::connect_async;
    use tokio::net::TcpListener;


    /// Tests the happy path for receiving a program message:
    /// 1. Sets up a mock WebSocket server
    /// 2. Sends a binary message from server to client
    /// 3. Verifies the message is received correctly
    /// 4. Cleans up server resources
    #[tokio::test]
    async fn test_receive_program_message() -> Result<(), Box<dyn std::error::Error>> {
        // Setup a mock WebSocket server
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server_url = format!("ws://{}", addr);

        // Spawn the mock server
        let test_message = b"test program data".to_vec();
        let test_message_clone = test_message.clone();
        
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
            ws_stream.send(Message::Binary(test_message_clone)).await.unwrap();
        });

        // Connect client
        let (ws_stream, _) = connect_async(&server_url).await?;
        let mut client = ws_stream;

        // Test receive_program_message
        let received = receive_program_message(
            &mut client,
            &server_url,
            "test_prover"
        ).await?;

        assert_eq!(received, test_message);

        server_handle.abort();
        Ok(())
    }


     /// Tests error handling when receiving unexpected message types:
    /// 1. Sets up a mock WebSocket server
    /// 2. Sends a text message instead of expected binary
    /// 3. Verifies receive_program_message returns appropriate error
    /// 4. Cleans up server resources
    #[tokio::test]
    async fn test_receive_program_message_unexpected_type() -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server_url = format!("ws://{}", addr);

        // Spawn server that sends text instead of binary
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
            ws_stream.send(Message::Text("wrong type".into())).await.unwrap();
        });

        let (ws_stream, _) = connect_async(&server_url).await?;
        let mut client = ws_stream;

        // Should return an error for unexpected message type
        let result = receive_program_message(
            &mut client,
            &server_url,
            "test_prover"
        ).await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Unexpected message type"
        );

        server_handle.abort();
        Ok(())
    }
}