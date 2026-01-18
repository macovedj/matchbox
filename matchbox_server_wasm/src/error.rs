//! Error types for the WASM signaling server

use thiserror::Error;

/// Errors that can occur during signaling
#[derive(Error, Debug)]
pub enum SignalingError {
    /// Failed to send message to peer
    #[error("Failed to send message: {0}")]
    SendError(String),

    /// Unknown peer
    #[error("Unknown peer")]
    UnknownPeer,

    /// WebSocket error
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// JSON parsing error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(String),
}

/// Errors from client requests
#[derive(Error, Debug)]
pub enum ClientRequestError {
    /// Connection was closed
    #[error("Connection closed")]
    Close,

    /// JSON parsing error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Unsupported message type
    #[error("Unsupported message type")]
    UnsupportedType,

    /// WebSocket error
    #[error("WebSocket error: {0}")]
    WebSocket(String),
}
