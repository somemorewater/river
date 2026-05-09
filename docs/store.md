# Store Engine (`RiverStore`)

River’s storage engine is currently a single struct: `RiverStore`, backed by an in-memory `HashMap<String, String>` with a small internal metrics counter.

Source:
- `src/store/engine.rs`

## The `RiverStore` Struct

Conceptually:

```
RiverStore
  └── data: HashMap<String, String>
  └── operations: usize
```

It stores string keys and string values, and tracks how many store operations have run since startup. This keeps the initial system simple while the project focuses on parsing, command dispatch, observability, and clean architecture.

## Operations

### `set(key, value)`

Stores a value under a key:

- If the key does not exist, it is inserted.
- If the key already exists, it is overwritten.
- Increments the operations counter.

### `get(key)`

Fetches the value for a key:

- Returns `Some(&String)` if present
- Returns `None` if missing
- Increments the operations counter.

The REPL layer converts this into user-facing output:

- present → prints the value
- missing → prints `NULL`

### `delete(key)`

Removes a key from the store:

- If the key exists, it is removed.
- If it does not exist, the operation is still safe (no error).
- Increments the operations counter.

The REPL prints `OK` for `DEL` regardless of whether the key existed. This keeps the interface simple and predictable.

### `stats()`

Returns a `StoreStats` snapshot without mutating state:

```
StoreStats
  └── keys: usize
  └── operations: usize
```

`keys` is derived from `HashMap.len()`. `operations` is the total number of `set`, `get`, and `delete` calls since startup.

## Why `HashMap`?

`HashMap` is a natural starting point for an in-memory key-value store:

- Average-case **O(1)** insert/get/delete
- Simple and familiar API
- Good enough performance for early experimentation

As River evolves, `HashMap` can be replaced or wrapped with more advanced designs:

- Sharding for concurrency
- Custom allocators or memory layouts
- Persistence layers (snapshotting / append-only logs)

## Memory Behavior (High Level)

Because the store is in-memory:

- All keys and values live in the process heap.
- Each `SET` allocates (or reuses) memory for the stored strings.
- `DEL` removes entries, and memory may be reclaimed by Rust’s allocator over time.

In this stage, River does not attempt to control allocation strategy; correctness and clarity come first.

## Observability

The store owns its own metrics so higher layers do not need to duplicate storage knowledge.

```
Command Handler
  ↓
store.stats()
  ↓
keys + operations
```

This is the foundation for future commands like `INFO`, memory statistics, or uptime reporting.
