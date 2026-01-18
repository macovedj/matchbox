//! HTTP long-polling handlers for the WASM signaling server
//!
//! This module implements long-polling based signaling that works over 
//! plain HTTP without WebSocket upgrades or long-lived connections.

use crate::state::{RoomId, ServerState};
use matchbox_protocol::{JsonPeerEvent, JsonPeerRequest, PeerId, PeerRequest};
use std::str::FromStr;
use wstd::http::{Body, Request, Response};

/// Extract room ID from path like "/room_name" or "/events/room_name"
fn extract_room(path: &str) -> Option<RoomId> {
    let path = path.trim_start_matches('/');
    if path.is_empty() || path == "poll" || path == "events" {
        return None;
    }
    // Strip /poll/ or /events/ prefix if present
    let room = path
        .strip_prefix("poll/")
        .or_else(|| path.strip_prefix("events/"))
        .unwrap_or(path);
    
    if room.is_empty() || room == "health" || room == "signal" {
        None
    } else {
        Some(RoomId(room.to_string()))
    }
}

/// Get query parameter from URI
fn get_query_param<'a>(query: Option<&'a str>, key: &str) -> Option<&'a str> {
    query?
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let k = parts.next()?;
            let v = parts.next()?;
            if k == key { Some(v) } else { None }
        })
}

/// Join a room - returns peer ID and any pending events
async fn handle_join(
    room_id: RoomId,
    peer_id: Option<PeerId>,
    state: &ServerState,
) -> Result<Response<Body>, wstd::http::Error> {
    let (peer_id, events) = state.join_or_poll(room_id, peer_id);
    
    // Serialize events as JSON array
    let events_json: Vec<String> = events.into_iter().collect();
    let response_body = serde_json::json!({
        "peer_id": peer_id.to_string(),
        "events": events_json
    });

    Ok(Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .header("access-control-allow-origin", "*")
        .body(Body::from(response_body.to_string()))
        .unwrap())
}

/// Handle a signal POST request
async fn handle_signal(
    request: Request<Body>,
    state: &ServerState,
) -> Result<Response<Body>, wstd::http::Error> {
    // Get the sender's peer ID from header
    let sender_id = request
        .headers()
        .get("x-peer-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(PeerId);

    let sender_id = match sender_id {
        Some(id) => id,
        None => {
            return Ok(Response::builder()
                .status(400)
                .header("access-control-allow-origin", "*")
                .body(Body::from("Missing or invalid X-Peer-Id header"))
                .unwrap());
        }
    };

    // Read the body
    let mut body = request.into_body();
    let body_str = match body.str_contents().await {
        Ok(s) => s.to_string(),
        Err(e) => {
            return Ok(Response::builder()
                .status(400)
                .header("access-control-allow-origin", "*")
                .body(Body::from(format!("Failed to read body: {:?}", e)))
                .unwrap());
        }
    };

    // Parse the signal request
    let signal_request = match JsonPeerRequest::from_str(&body_str) {
        Ok(req) => req,
        Err(e) => {
            return Ok(Response::builder()
                .status(400)
                .header("access-control-allow-origin", "*")
                .body(Body::from(format!("Invalid request: {:?}", e)))
                .unwrap());
        }
    };

    match signal_request {
        PeerRequest::Signal { receiver, data } => {
            let signal_event = JsonPeerEvent::Signal {
                sender: sender_id,
                data,
            }
            .to_string();

            match state.queue_event(receiver, signal_event) {
                Ok(()) => Ok(Response::builder()
                    .status(200)
                    .header("access-control-allow-origin", "*")
                    .body(Body::from("OK"))
                    .unwrap()),
                Err(_) => Ok(Response::builder()
                    .status(404)
                    .header("access-control-allow-origin", "*")
                    .body(Body::from("Peer not found"))
                    .unwrap()),
            }
        }
        PeerRequest::KeepAlive => Ok(Response::builder()
            .status(200)
            .header("access-control-allow-origin", "*")
            .body(Body::from("OK"))
            .unwrap()),
    }
}

/// Handle CORS preflight
fn handle_cors_preflight() -> Result<Response<Body>, wstd::http::Error> {
    Ok(Response::builder()
        .status(204)
        .header("access-control-allow-origin", "*")
        .header("access-control-allow-methods", "GET, POST, OPTIONS")
        .header("access-control-allow-headers", "content-type, x-peer-id")
        .header("access-control-max-age", "86400")
        .body(Body::empty())
        .unwrap())
}

/// Handle an HTTP request - main router
pub async fn handle_request(
    request: Request<Body>,
    state: &ServerState,
) -> Result<Response<Body>, wstd::http::Error> {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path();

    // CORS preflight
    if method == wstd::http::Method::OPTIONS {
        return handle_cors_preflight();
    }

    // Health check
    if path == "/health" {
        return Ok(Response::builder()
            .status(200)
            .header("access-control-allow-origin", "*")
            .body(Body::from("OK"))
            .unwrap());
    }

    // Signal endpoint (POST)
    if path == "/signal" && method == wstd::http::Method::POST {
        return handle_signal(request, state).await;
    }

    // Poll/join endpoint (GET /poll/{room} or GET /{room})
    if method == wstd::http::Method::GET {
        if let Some(room_id) = extract_room(path) {
            // Get optional peer_id from query string
            let peer_id = get_query_param(uri.query(), "peer_id")
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
                .map(PeerId);
            
            return handle_join(room_id, peer_id, state).await;
        }

        // Regular GET / - return info page
        return Ok(Response::builder()
            .status(200)
            .header("content-type", "text/plain")
            .header("access-control-allow-origin", "*")
            .body(Body::from(
                "Matchbox WASI Signaling Server (Long-Polling)\n\
                 \n\
                 Endpoints:\n\
                 - GET /health - Health check\n\
                 - GET /poll/{room}?peer_id={id} - Join/poll room for events\n\
                 - POST /signal - Send signal (X-Peer-Id header required)\n\
                 \n\
                 Protocol:\n\
                 1. GET /poll/{room} to join and get peer_id + initial events\n\
                 2. Poll GET /poll/{room}?peer_id={id} for new events\n\
                 3. POST /signal with X-Peer-Id header to send signals\n\
                 \n\
                 Response format: {\"peer_id\": \"uuid\", \"events\": [...]}\n"
            ))
            .unwrap());
    }

    // Unknown endpoint
    Ok(Response::builder()
        .status(404)
        .header("access-control-allow-origin", "*")
        .body(Body::from("Not Found"))
        .unwrap())
}
