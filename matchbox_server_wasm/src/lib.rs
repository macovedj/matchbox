//! WASI-compatible WebRTC signaling server using wstd
//!
//! This crate provides a signaling server that can be compiled to WebAssembly
//! and run in WASI-compatible runtimes using HTTP long-polling.
//!
//! # Protocol
//!
//! Instead of WebSockets, this server uses HTTP long-polling:
//!
//! - **GET /poll/{room}?peer_id={id}** - Join/poll for events
//! - **POST /signal** - Send signal requests (X-Peer-Id header required)
//! - **GET /health** - Health check
//!
//! ## Response Format (server → client)
//!
//! JSON response with peer_id and pending events:
//! ```json
//! {"peer_id": "<uuid>", "events": ["..."]}
//! ```
//!
//! Events are JSON strings:
//! - `{"IdAssigned": "<uuid>"}` - Your peer ID
//! - `{"NewPeer": "<uuid>"}` - New peer joined
//! - `{"PeerLeft": "<uuid>"}` - Peer disconnected
//! - `{"Signal": {"sender": "<uuid>", "data": ...}}` - Signal from peer
//!
//! ## Signal Requests (client → server)
//!
//! POST to /signal with X-Peer-Id header and JSON body:
//! - `{"Signal": {"receiver": "<uuid>", "data": ...}}`
//! - `"KeepAlive"`
//!
//! # Example
//!
//! ```bash
//! # Start the server
//! wasmtime serve -S common --addr 127.0.0.1:3536 matchbox-signaling-wasm.wasm
//!
//! # Join a room (returns peer_id and initial events)
//! curl http://127.0.0.1:3536/poll/my_room
//!
//! # Poll for new events
//! curl "http://127.0.0.1:3536/poll/my_room?peer_id=<your-id>"
//!
//! # Send a signal
//! curl -X POST -H "X-Peer-Id: <your-id>" -H "Content-Type: application/json" \
//!   -d '{"Signal":{"receiver":"<peer-id>","data":"hello"}}' \
//!   http://127.0.0.1:3536/signal
//! ```

#![forbid(unsafe_code)]

pub mod error;
pub mod handler;
pub mod state;

pub use error::SignalingError;
pub use handler::handle_request;
pub use state::{RoomId, ServerState};
