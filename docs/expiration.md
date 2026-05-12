# Expiration

River supports Redis-style key expiration through `EXPIRE` and `SETEX`.

Source:
- `src/store/engine.rs`
- `src/commands/parser.rs`
- `src/commands/mod.rs`

## Data Model

Expiration metadata is stored separately from key-value data:

```text
RiverStore
  ├── data: HashMap<String, String>
  └── expirations: HashMap<String, u64>
```

The expiration map stores Unix timestamps in seconds. This keeps persistence simple because timestamps can be serialized directly with `serde` and `bincode`.

## Commands

`EXPIRE key seconds` attaches a TTL to an existing key.

```text
*3
$6
EXPIRE
$7
session
$2
60
```

Response:

```text
:1
```

`SETEX key seconds value` sets a value and TTL together.

```text
*4
$5
SETEX
$7
session
$2
60
$3
abc
```

Response:

```text
+OK
```

## Passive Expiration

When a key is read, River checks its expiration timestamp first. If the key is expired, River removes it and returns RESP null:

```text
$-1
```

Expired data is never returned to clients.

## Active Cleanup

The TCP server starts a lightweight Tokio task that runs once per second:

```text
interval tick
  ↓
store.cleanup_expired()
  ↓
save river.db if keys were removed
```

This keeps stale keys from accumulating when they are not read.

## Persistence

Expiration metadata is persisted with the store. On startup, River removes any keys that expired while the server was offline.

## Future Work

This design can grow toward LRU/LFU eviction, memory pressure cleanup, smarter scheduling, and cache policy experimentation.
