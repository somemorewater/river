# Future Work

River is intentionally built in stages. This document outlines likely next steps and what they imply for the codebase.

## Protocols

River now has a RESP-inspired TCP protocol. Future protocol work can build on that framing layer.

Potential protocol upgrades:

- Pipelining
- Richer Redis client compatibility
- HTTP endpoints for status or diagnostics
- Authentication handshake before command execution

## Persistence Upgrades

River currently persists a binary database snapshot with `serde` and `bincode`.

Future approaches:

- **Append-only log (AOF)**: record commands so the store can be rebuilt on startup.
- **Background snapshots**: write data without blocking command execution.
- **Crash recovery**: add checksums, manifests, or durable rename strategies.
- **Compression/encryption**: reduce disk usage or protect stored data.

The current implementation favors clarity and correctness over advanced write performance.

## Expiration Upgrades

River now supports key TTLs with passive reads and a lightweight active cleanup worker.

Future expiration work can explore:

- LRU/LFU eviction
- Memory pressure cleanup
- More efficient expiration scheduling
- Per-key TTL inspection commands
- Distributed expiration behavior

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
