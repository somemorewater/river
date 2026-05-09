# Persistence

River persists database state to disk using `serde` and `bincode`.

Source:
- `src/persistence/storage.rs`

## Responsibilities

- Serialize the current `RiverStore`
- Write binary database data to disk
- Load database state on startup
- Treat missing database files as an empty store
- Report read/write/serialization errors without crashing the server

## Default File

River writes to:

```text
river.db
```

Use `RIVER_DB_PATH` to choose another file:

```bash
RIVER_DB_PATH=/tmp/river-dev.db cargo run
```

## Save Behavior

River saves automatically after mutating commands:

- `SET`
- `DEL`

Read-only commands do not trigger disk writes:

- `GET`
- `PING`
- `STATS`
- `HEALTH`

## Startup Recovery

On startup:

1. If the database file exists, River attempts to deserialize it.
2. If loading succeeds, the store is restored.
3. If the file is missing, River starts empty.
4. If the file is corrupted or unreadable, River logs the error and starts empty.

## Current Tradeoff

This stage writes a full binary snapshot after each mutation. That is simple and reliable for learning, but not optimized for large datasets.

Future durability systems can add append-only files, background snapshots, checksums, compression, encryption, and replication.
