# Future Work

River is intentionally built in stages. This document outlines likely next steps and what they imply for the codebase.

## Protocols

River now has a line-based TCP server. Future protocol work can build on that networking layer.

Potential protocol upgrades:

- RESP support for Redis-like clients
- HTTP endpoints for status or diagnostics
- Authentication handshake before command execution

## Persistence (Snapshots / Logs)

Persist data so River can restart without losing state.

Potential approaches:

- **Snapshotting**: periodically serialize the entire `HashMap` and write it to disk.
- **Append-only log (AOF)**: record commands so the store can be rebuilt on startup.

The crate dependencies already include `serde` and `bincode`, which can be used later for serialization, but persistence is not implemented yet.

## INFO Command

`STATS` currently exposes key count and operation count. A future `INFO` command can build on the same store-owned metrics model.

Possible fields:

- Key count
- Operation count
- Uptime
- Approximate memory usage
- Persistence status

## Benchmarking

River can be benchmarked to understand the costs of parsing, dispatch, and storage operations.

The project includes Criterion as a dev-dependency; benchmarks can measure parser, store, and TCP command round-trip behavior.

## Concurrency Upgrades

Multiple clients are supported through Tokio tasks and shared state behind a mutex. Future work can improve throughput under load.

Possible improvements:

- Shared store behind a lock (simple correctness first)
- Sharded `HashMap` to reduce lock contention
- Dedicated worker model for command execution

The current server uses Tokio for async networking.
