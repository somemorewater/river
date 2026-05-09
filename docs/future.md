# Future Work

River is intentionally built in stages. This document outlines likely next steps and what they imply for the codebase.

## TCP Server

Add a server that accepts client connections and handles commands over a socket.

Key design goal:
- Reuse the existing parsing layer (`commands::parser`) and the same command dispatch logic as the REPL.

Expected shape:

```
TCP listener
  ↓
accept connection
  ↓
read line / frame
  ↓
parse command
  ↓
execute against store
  ↓
write response
```

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

The project includes Criterion as a dev-dependency; benchmarks can be added once stable command handling and (later) networking exist.

## Concurrency Upgrades

Once multiple clients are supported, concurrency becomes important.

Possible improvements:

- Shared store behind a lock (simple correctness first)
- Sharded `HashMap` to reduce lock contention
- Dedicated worker model for command execution

The project already depends on Tokio, but the current stage is intentionally synchronous (no async networking yet).
