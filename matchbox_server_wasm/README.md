# matchbox_server_wasm

A WASI-compatible WebRTC signaling server built with [wstd](https://docs.rs/wstd).

This is a port of `matchbox_server` that can run as a WebAssembly component in WASI-compatible runtimes like Wasmtime.

## Building

```bash
# Add the WASI target if you haven't already
rustup target add wasm32-wasip2

# Build the WASM component (release)
cargo build -p matchbox_server_wasm --target wasm32-wasip2 --release
```

The output will be at `target/wasm32-wasip2/release/matchbox-signaling-wasm.wasm`

## Running with Wasmtime

```bash
# Run the signaling server on port 3536
wasmtime serve --addr 127.0.0.1:3536 -S common --dir .::/ \
  target/wasm32-wasip2/release/matchbox-signaling-wasm.wasm
```

Flags explained:
- `--addr 127.0.0.1:3536` - Listen address
- `-S common` - Enable common WASI interfaces (CLI, environment)
- `--dir .::/` - Map current directory to `/` for state file persistence

## Protocol

This server uses **HTTP long-polling** instead of WebSockets for WASI compatibility.

### Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check, returns "OK" |
| `/poll/{room}` | GET | Join a room, returns peer_id and events |
| `/poll/{room}?peer_id={id}` | GET | Poll for new events |
| `/signal` | POST | Send a signal to another peer |

### Join/Poll Response

```json
{
  "peer_id": "uuid-string",
  "events": [
    "{\"IdAssigned\":\"uuid\"}",
    "{\"NewPeer\":\"uuid\"}",
    "{\"Signal\":{\"sender\":\"uuid\",\"data\":{...}}}"
  ]
}
```

### Signal Request

```bash
curl -X POST http://localhost:3536/signal \
  -H "Content-Type: application/json" \
  -H "X-Peer-Id: your-peer-id" \
  -d '{"Signal":{"receiver":"target-peer-id","data":{"type":"offer","sdp":"..."}}}'
```

## Example Flow

```bash
# Terminal 1: Start server
wasmtime serve --addr 127.0.0.1:3536 -S common --dir .::/ \
  target/wasm32-wasip2/release/matchbox-signaling-wasm.wasm

# Terminal 2: Client 1 joins
curl http://localhost:3536/poll/my_room
# Returns: {"peer_id":"abc...","events":["{\"IdAssigned\":\"abc...\"}"]}

# Terminal 3: Client 2 joins
curl http://localhost:3536/poll/my_room
# Returns: {"peer_id":"def...","events":["{\"IdAssigned\":\"def...\"}","{\"NewPeer\":\"abc...\"}"]}

# Terminal 2: Client 1 polls and sees Client 2
curl "http://localhost:3536/poll/my_room?peer_id=abc..."
# Returns: {"peer_id":"abc...","events":["{\"NewPeer\":\"def...\"}"]}

# Send a signal from Client 1 to Client 2
curl -X POST http://localhost:3536/signal \
  -H "X-Peer-Id: abc..." \
  -H "Content-Type: application/json" \
  -d '{"Signal":{"receiver":"def...","data":{"type":"offer"}}}'
```

## State Persistence

State is persisted to `matchbox_state.json` in the working directory. This allows the signaling state to survive across HTTP requests (which run in isolated WASI instances).

## Architecture

Unlike the native `matchbox_server`, this version:

- Uses `wstd::http` with `#[wstd::http_server]` for HTTP handling
- Uses HTTP long-polling instead of WebSockets (WASI HTTP doesn't support upgrades yet)
- Persists state to a JSON file between requests
- Runs in wstd's single-threaded async runtime

## Limitations

1. **Long-polling vs WebSockets**: Clients must poll for events instead of receiving pushes
2. **No TLS support**: TLS must be provided by a reverse proxy
3. **Single-threaded execution**: WASI 0.2 doesn't support multi-threading
4. **File-based state**: State is stored in a JSON file (could use wasi-keyvalue in the future)

## Protocol Compatibility

The signaling messages (IdAssigned, NewPeer, PeerLeft, Signal) use the same `matchbox_protocol` format as the native server. Clients need to be adapted for long-polling instead of WebSocket.

## Comparison with Native matchbox_server

| Feature | Native | WASM |
|---------|--------|------|
| Runtime | tokio | wstd |
| HTTP Framework | axum | wstd http_server |
| Transport | WebSocket | HTTP long-polling |
| State | In-memory | File-based JSON |
| Threading | Multi-threaded | Single-threaded |
| Binary Size | ~10MB | ~1MB |
| TLS | Built-in option | External proxy |
