# Architecture

This document describes how River is organized today and why the modules are separated the way they are.

## System Flow

River currently behaves like a tiny Redis-style interface:

```
CLI Input
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

- **CLI Input / REPL**: `src/main.rs`
- **Parser**: `src/commands/parser.rs`
- **Command Handler**: `src/main.rs` (the `match` over `Command`)
- **Store**: `src/store/engine.rs`

## Why Modular Design?

River keeps parsing and storage separate for three reasons:

1. **Correctness and safety**: parsing and validation are centralized; the store never needs to interpret raw user input.
2. **Testability**: the parser and store can be tested independently.
3. **Future transport support**: a TCP server can accept bytes/lines, then reuse the exact same parser and dispatch logic.

## Parser vs. Storage Responsibilities

**Parser responsibilities** (`commands` module):

- Trim leading/trailing whitespace
- Treat any run of whitespace as a separator (normalization)
- Validate command names and argument counts
- Return a structured `Command` value or a specific error

**Storage responsibilities** (`store` module):

- Manage in-memory state (`HashMap<String, String>`)
- Implement `set`, `get`, and `delete` operations
- Track lightweight runtime metrics (`keys`, `operations`)
- Expose read-only stats through `store.stats()`

## Designing for Networking (Next Stage)

A future TCP server can be layered on top without changing the store:

```
TCP connection
  ↓
read line / frame
  ↓
commands::parser::parse_line
  ↓
dispatch (same match as REPL)
  ↓
RiverStore
  ↓
write response
```

The important idea is: **only the input/output transport changes**, not the parser or storage engine.

## Observability Path

River’s stats flow follows the same separation:

```
CLI Input (`STATS`)
  ↓
Parser (`Command::Stats`)
  ↓
Command Handler
  ↓
RiverStore::stats()
  ↓
keys / operations response
```

Metrics are intentionally owned by the store. The REPL does not calculate key counts or operation totals itself.
