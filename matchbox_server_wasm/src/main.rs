//! WASI HTTP server entry point for the SSE-based signaling server
//!
//! This module provides the main entry point when running as a WASI HTTP component
//! using `wasmtime serve`.

use matchbox_server_wasm::{ServerState, handle_request};
use std::cell::RefCell;
use wstd::http::{Body, Request, Response};

// Thread-local state for the server (WASI is single-threaded)
thread_local! {
    static STATE: RefCell<Option<ServerState>> = const { RefCell::new(None) };
}

fn get_or_init_state() -> ServerState {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        if state.is_none() {
            *state = Some(ServerState::new());
        }
        state.as_ref().unwrap().clone()
    })
}

/// The main HTTP handler for WASI
///
/// This function is called by the WASI runtime for each incoming HTTP request.
#[wstd::http_server]
async fn main(request: Request<Body>) -> Result<Response<Body>, wstd::http::Error> {
    let state = get_or_init_state();
    handle_request(request, &state).await
}
