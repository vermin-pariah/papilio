# Papilio Server

The backend REST API server for the Papilio music center, built with Rust and Axum.

## 🚀 Features

- **RESTful API**: Clean and efficient endpoints for music management, user authentication, and system control.
- **Real-time Transcoding**: Integrated FFmpeg for on-the-fly audio transcoding.
- **Valkey Integration**: Session management and caching using Valkey (Redis-compatible).
- **Static File Serving**: High-performance serving of album covers and user avatars.
- **Industrial Logging**: Comprehensive tracing and audit logs.

## 🛠 Configuration

Configuration is managed via environment variables (see root `.env`).

## 🏃 Running

```bash
cargo run --release
```
