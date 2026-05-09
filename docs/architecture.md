# Architecture

This document describes how River is organized today and why the modules are separated the way they are.

## System Flow

River currently behaves like a tiny Redis-style TCP database server:

```
TCP Client
  ↓
TCP Layer
  ↓
Parser
  ↓
Command Handler
  ↓
Store
  ↓
Response
```

Mapping to code:

- **TCP Layer**: `src/server/tcp.rs`
- **Parser**: `src/commands/parser.rs`
- **Command Handler**: `src/commands/mod.rs`
- **Store**: `src/store/engine.rs`
- **Bootstrap**: `src/main.rs`

## Why Modular Design?

River keeps parsing and storage separate for three reasons:

1. **Correctness and safety**: parsing and validation are centralized; the store never needs to interpret raw user input.
2. **Testability**: the parser and store can be tested independently.
3. **Future transport support**: HTTP, RESP, or a CLI can reuse the same parser and dispatch logic.

## Parser vs. Storage Responsibilities

**Parser responsibilities** (`commands` module):

- Trim leading/trailing whitespace
- Treat any run of whitespace as a separator (normalization)
- Validate command names and argument counts
- Return a structured `Command` value or a specific error

**Command responsibilities** (`commands` module):

- Convert parsed commands into store operations
- Format user-facing responses
- Map parser errors to stable error strings

**Storage responsibilities** (`store` module):

- Manage in-memory state (`HashMap<String, String>`)
- Implement `set`, `get`, and `delete` operations
- Track lightweight runtime metrics (`keys`, `operations`)
- Expose read-only stats through `store.stats()`

## TCP Networking

TCP is isolated in `src/server/tcp.rs`:

```
TCP connection
  ↓
read line / frame
  ↓
commands::parser::parse_line
  ↓
commands::handle_input
  ↓
RiverStore
  ↓
write response
```

Each client connection runs in its own Tokio task. Shared database state is protected with `Arc<tokio::sync::Mutex<RiverStore>>`.

## Observability Path

River’s stats flow follows the same separation:

```
TCP Input (`STATS`)
  ↓
Parser (`Command::Stats`)
  ↓
Command Handler
  ↓
RiverStore::stats()
  ↓
keys / operations response
```

Metrics are intentionally owned by the store. The TCP layer does not calculate key counts or operation totals itself.
