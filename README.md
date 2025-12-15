# CoreLink

**A Decentralized Mesh Network Protocol for Peer-to-Peer File Sharing**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](https://github.com/ChronoCoders/corelink)

## Overview

CoreLink enables secure, decentralized file sharing through peer-to-peer mesh networking. Built in Rust with libp2p, it features chunk-based file transfer with SHA256 verification, automatic peer discovery, and a novel Physical Proof of Proximity (PoPI) consensus mechanism (in development).

**Current Status:** Core file transfer protocol operational. Consensus and distributed storage in active development.

## ✨ Features

### Working Today ✅
- **Peer-to-Peer Networking**: Automatic peer discovery via mDNS
- **Chunk-Based File Transfer**: 64KB chunks with SHA256 verification
- **Auto-Download**: Automatic file retrieval when offered by peers
- **Multi-Chunk Batching**: Parallel chunk requests (5 at a time)
- **Progress Tracking**: Real-time transfer progress (0-100%)
- **LRU Caching**: Efficient chunk serving with 100-chunk cache
- **CLI Interface**: Simple commands (`offer`, `help`)
- **Encrypted Connections**: Noise protocol encryption (XX pattern)
- **Stream Multiplexing**: Yamux for efficient connection usage

### In Development 🚧
- **Web Dashboard**: Real-time visualization (Leptos WASM)
- **Physical Proof of Proximity**: GPS/WiFi/BLE consensus
- **DHT Storage**: Kademlia-based distributed storage
- **WebSocket API**: Real-time backend communication

### Planned 📋
- **CORE Token**: Utility token for network incentives
- **DAO Governance**: Community-driven protocol upgrades
- **Hardware Integration**: LoRa, ESP32, Raspberry Pi
- **Mobile Apps**: iOS and Android support
- **Multi-Source Downloads**: Download from multiple peers simultaneously

## 🚀 Quick Start

### Prerequisites
- Rust 1.75 or higher
- Cargo (comes with Rust)
- Git

### Installation
```bash
# Clone repository
git clone https://github.com/ChronoCoders/corelink.git
cd corelink

# Build release binary
cargo build --release

# Binary location
./target/release/corelink-node
```

### Running Your First Node

**Terminal 1 - Start first node:**
```bash
cargo run --release --bin corelink-node -- --port 4001
```

**Terminal 2 - Start second node:**
```bash
cargo run --release --bin corelink-node -- --port 4002
```

The nodes will automatically discover each other via mDNS and establish encrypted connections.

### Sharing a File

In Terminal 1, type:
```
offer
```

The node will:
1. Create a test file (`test.txt`) if it doesn't exist
2. Split it into 64KB chunks
3. Compute SHA256 hash per chunk
4. Broadcast availability to connected peers

Terminal 2 will automatically:
1. Receive the file offer
2. Request chunks in batches of 5
3. Verify each chunk's integrity
4. Assemble the complete file
5. Save to `./storage/complete/test.txt`

### Verifying the Transfer
```bash
# Check downloaded file
cat storage/complete/test.txt

# Expected output:
# Hello CoreLink! This is a test file.
# Chunk-based transfer protocol working!
# SHA256 verification enabled.
```

## 📖 Usage

### CLI Commands

| Command | Description |
|---------|-------------|
| `offer` | Share test.txt with connected peers |
| `help`  | Show available commands |

### File Storage Structure
```
./storage/
├── uploads/      # Files you're offering (cached chunks)
├── downloads/    # In-progress downloads (partial files)
└── complete/     # Completed transfers (verified files)
```

### Example Session
```
🚀 Starting CoreLink node on port 4001
🔑 Peer ID: 12D3KooWALh24BMAfj5JaE5XwHcP8N7UukMHPzNiED24oWKihm4e
📍 Listening on /ip4/0.0.0.0/tcp/4001
💡 Commands: 'offer' to share test.txt, 'help' for more

🔍 Discovered peer: 12D3KooWJXt... at /ip4/192.168.1.100/tcp/4002
✅ Connection established with 12D3KooWJXt...

> offer
📝 Created test.txt
📤 Offering: test.txt (123 bytes, 2 chunks)
```

## 🏗️ Architecture

```
┌─────────────────────────────────────────┐
│           Application Layer             │
│  CLI • Web Dashboard • REST API         │
└─────────────────────────────────────────┘
                  │
┌─────────────────────────────────────────┐
│           Business Logic                │
│  FileTransferManager • ConsensusEngine  │
└─────────────────────────────────────────┘
                  │
┌─────────────────────────────────────────┐
│           Protocol Layer                │
│  Custom Messaging (/corelink/msg/1.0.0)│
└─────────────────────────────────────────┘
                  │
┌─────────────────────────────────────────┐
│         Network Layer (libp2p)          │
│  TCP • Noise • Yamux • mDNS • DHT       │
└─────────────────────────────────────────┘
```

### Core Components

- **libp2p**: Modular P2P networking framework
- **Noise Protocol**: End-to-end encryption (XX pattern)
- **Yamux**: Stream multiplexing for efficient connection usage
- **mDNS**: Local peer discovery on the same network
- **Custom Protocol**: Message types for file transfer and consensus
- **FileTransferManager**: Coordinates uploads, downloads, and chunk serving
- **LRU Cache**: Fast chunk retrieval for frequently requested files

## 🔧 Development

### Project Structure
```
corelink/
├── core/               # Shared types and utilities
│   └── src/
│       ├── file.rs     # File metadata, chunks, verification
│       ├── message.rs  # Message protocol definitions
│       ├── identity.rs # Node identity and cryptography
│       └── lib.rs      # Public exports
├── node/               # Network node implementation
│   └── src/
│       ├── main.rs                  # Entry point and CLI
│       ├── messaging_behaviour.rs   # Network behavior
│       ├── protocol_handler.rs      # Stream handling
│       └── file_transfer.rs         # File transfer logic
├── web/                # Web dashboard (Leptos)
│   └── src/
├── simulator/          # Network simulator
└── README.md          # This file
```

### Building from Source
```bash
# Debug build (faster compilation)
cargo build

# Release build (optimized, recommended for testing)
cargo build --release

# Run tests
cargo test

# Check code without building
cargo check

# Format code
cargo fmt

# Lint with Clippy (strict mode)
cargo clippy -- -D warnings
```

### Running Tests
```bash
# All tests
cargo test

# Specific package
cargo test --package corelink-core
cargo test --package corelink-node

# Verbose output
cargo test -- --nocapture

# Release mode tests (faster execution)
cargo test --release
```

## 📊 Performance

> **Note**: These are preliminary benchmarks on a local network (same machine). Real-world performance will vary based on network conditions, hardware, and peer count.

**Theoretical Benchmarks (2-node local network):**

| Metric | Value | Notes |
|--------|-------|-------|
| Chunk Size | 64 KB | Configurable |
| Batch Size | 5 chunks | Parallel requests |
| Verification | SHA256 | Per chunk |
| Cache Size | 100 chunks | LRU eviction |
| Encryption | Noise XX | 256-bit keys |

**Network Overhead:**
- Discovery: ~100ms (mDNS)
- Connection: ~50ms (TCP + Noise handshake)
- Message: ~1ms (local network)

*Actual throughput depends on network quality, file size, and peer configuration. Benchmarks will be updated as the project matures.*

## 🗺️ Roadmap

### Q1 2025 (Current)
- [x] Core file transfer protocol
- [x] Peer discovery (mDNS)
- [x] Auto-download functionality
- [x] CLI interface
- [ ] Web dashboard integration
- [ ] WebSocket + REST API
- [ ] Multi-source downloads

### Q2 2025
- [ ] DHT storage layer (Kademlia)
- [ ] PoPI GPS integration (initial)
- [ ] Content addressing (IPFS-style)
- [ ] Testnet deployment (10-20 nodes)
- [ ] Public beta access

### Q3 2025
- [ ] PoPI consensus complete
- [ ] Enhanced security audits
- [ ] Performance optimization
- [ ] Documentation overhaul

### Q4 2025
- [ ] CORE token design
- [ ] DAO governance framework
- [ ] Bug bounty program
- [ ] Production readiness

### 2026 and Beyond
- [ ] Hardware integration (LoRa, ESP32)
- [ ] Mobile applications
- [ ] Cross-chain bridges
- [ ] Enterprise features

## 🤝 Contributing

We welcome contributions! Here's how you can help:

### Areas Needing Help
- 🦀 **Rust Development**: Protocol implementation, optimization
- 🎨 **Frontend**: Web dashboard (Leptos/WASM)
- 📝 **Documentation**: Guides, tutorials, API docs
- 🧪 **Testing**: Network testing, bug reports
- 🔐 **Security**: Audits, vulnerability reports
- 🌐 **Translation**: Internationalization

### Getting Started

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Setup
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone your fork
git clone https://github.com/YOUR_USERNAME/corelink.git
cd corelink

# Create feature branch
git checkout -b feature/my-contribution

# Make changes, test
cargo test

# Format and lint
cargo fmt
cargo clippy -- -D warnings

# Submit PR
```

### Code Style
- Follow Rust standard style (`cargo fmt`)
- Pass all Clippy lints (`cargo clippy -- -D warnings`)
- Write tests for new features
- Document public APIs with `///` doc comments
- Add examples to documentation

### Pull Request Guidelines
- Clear description of changes
- Reference related issues
- Include tests
- Update documentation
- Keep commits focused and atomic

## 📚 Documentation

- **Architecture Overview**: See [Architecture](#-architecture) above
- **Project Structure**: See [Development](#-development) above
- **API Reference**: Auto-generated with `cargo doc --open`

## 🛡️ Security

### Reporting Vulnerabilities

**DO NOT** open public issues for security vulnerabilities.

Instead, please report security issues by:
1. Opening a GitHub security advisory (preferred)
2. Creating a private issue with the `security` tag
3. Expected response time: 48-72 hours

We take security seriously and will acknowledge reports promptly.

### Security Features
- ✅ Noise protocol encryption (256-bit keys)
- ✅ SHA256 chunk verification
- ✅ Ed25519 signatures
- ✅ Secure temporary file handling
- 🚧 Physical Proof of Proximity (in development)
- 📋 End-to-end message encryption (planned)
- 📋 Zero-knowledge proofs (research phase)

### Known Limitations
- mDNS discovery limited to local network
- Single-source downloads (multi-source planned)
- No persistence of peer connections across restarts
- Test mode uses dummy cryptographic keys (production keys coming)

## 🐛 Known Issues

- Input buffering may cause double commands in some terminals
- Windows line ending (CRLF) warnings on git operations
- Storage paths are relative to execution directory

See [GitHub Issues](https://github.com/ChronoCoders/corelink/issues) for full list and workarounds.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **libp2p Team**: For the excellent P2P networking framework
- **Rust Community**: For outstanding tooling and support
- **Contributors**: Everyone who has contributed code, documentation, or feedback
- **Tokio Project**: For async runtime
- **Leptos Team**: For the reactive web framework

## 📞 Contact

- **GitHub**: [ChronoCoders/corelink](https://github.com/ChronoCoders/corelink)
- **Issues**: [GitHub Issues](https://github.com/ChronoCoders/corelink/issues)
- **Discussions**: [GitHub Discussions](https://github.com/ChronoCoders/corelink/discussions)

## 🌟 Star History

If you find CoreLink useful, please consider giving it a star ⭐ on GitHub!

---

**Built with ❤️ by the CoreLink community**

*Last Updated: December 2025*
