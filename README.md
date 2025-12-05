# CoreLink

Decentralized mesh network with physical consensus mechanism.

## Architecture

- **core**: Shared library (identity, messaging, consensus)
- **node**: Network daemon
- **simulator**: Multi-node testing environment
- **web**: Leptos dashboard (WASM)

## Build
```bash
cargo build --release
```

## Run Node
```bash
cargo run --bin corelink-node -- --port 4001 --name node1
```

## Run Simulator
```bash
cargo run --bin corelink-simulator
```

## Run Web Dashboard
```bash
cd web
trunk serve
```

## Testing
```bash
cargo test --workspace
```