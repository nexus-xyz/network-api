
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
use crate::track;
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