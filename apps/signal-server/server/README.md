# starlab-signal-server

A general WebRTC signal server for device-to-device communication, written in Rust and powered by async networking and WebSockets.

## Features

- Simple WebSocket-based signaling for WebRTC devices
- Device registration and discovery
- Message relay between devices
- Asynchronous, scalable, and easy to deploy

## Usage

From the starlab-mpc monorepo root:

```sh
cargo build -p starlab-signal-server --release
```

Run the server (default port: 9000):

```sh
cargo run -p starlab-signal-server --release
```

The server listens for WebSocket connections on `0.0.0.0:9000`.

## Protocol

Clients communicate with the server using JSON messages with a
`"type"` discriminator (serde `#[serde(tag = "type", rename_all =
"snake_case")]`). The authoritative enum definitions live in
`src/lib.rs` — `ClientMsg` at `:47` and `ServerMsg` at `:16`.
The examples below cover only the basic signaling primitives
that predate the monorepo; for the full 7 + 7 variant list
including session-discovery, see the table in
[`docs/deployment/CLOUDFLARE_DEPLOYMENT.md`](../../../docs/deployment/CLOUDFLARE_DEPLOYMENT.md)
§ Message types handled.

### Basic signaling — Register

```json
{ "type": "register", "device_id": "your-unique-id" }
```

### Basic signaling — List Devices

```json
{ "type": "list_devices" }
```

### Basic signaling — Relay Message

```json
{ "type": "relay", "to": "target-device-id", "data": { ... } }
```

### Basic signaling — Server Responses

- List of devices:
  ```json
  { "type": "devices", "devices": ["device1", "device2"] }
  ```
- Relayed message:
  ```json
  { "type": "relay", "from": "device1", "data": { ... } }
  ```
- Error:
  ```json
  { "type": "error", "error": "description" }
  ```

### Session-discovery variants (post-monorepo)

Not shown above but shipped today — see the enum definitions and
the CLOUDFLARE_DEPLOYMENT.md table for the full shape:

- Client → server: `announce_session`, `request_active_sessions`,
  `session_status_update`, `query_my_active_sessions`
- Server → client: `session_available`, `sessions_for_device`,
  `session_list_request`, `session_removed`

## License

MIT OR Apache-2.0

## Repository

This crate lives in the [starlab-mpc monorepo](https://github.com/hecoinfo/starlab-mpc)
under `apps/signal-server/server/`. It was previously published from
`stars-labs/crypto-rust-tools` before being absorbed into the monorepo;
older crates.io metadata may still point there.
