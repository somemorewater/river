# Observability

River exposes leveled logs and in-memory metrics. No external monitoring
system is required.

Source:
- `src/logging.rs` (level parsing, subscriber setup)
- `src/metrics.rs` (atomic counters, connection guard)
- `STATS` formatting: `src/commands/mod.rs`

## Logging

The server logs lifecycle and operational events with `tracing`:

- startup (bound address, persistence file, store state)
- client connect / disconnect
- malformed RESP frames and non-command frames (`warn`, kind only — no key/value payloads)
- unknown commands and invalid syntax (`warn`)
- persistence save failures (`error`)
- background TTL cleanup removals (`info`)
- per-command execution at `debug` (`command`, `elapsed_us`)

CLI user output stays on stdout; logs go to stderr.

## RIVER_LOG

```bash
RIVER_LOG=debug cargo run -- server
```

Accepted values: `error`, `warn`, `info`, `debug` (case-insensitive).
Missing or invalid values fall back to `info` with a stderr warning.
Default: `info`.

## Metrics

`Metrics` holds only `AtomicUsize` counters (owned by `ConcurrentStore`,
reachable from every layer without new plumbing):

- `commands_total`, per-command `cmd_set/get/del/expire/setex/ping/stats/health/exit/auth`
- `command_errors` (unknown + invalid syntax + failed executions)
- `protocol_errors` (malformed frames, non-command shapes)
- `auth_success` / `auth_failures` / `auth_required`
- `connections_active` (gauge) / `connections_total`
- `persistence_saves` / `persistence_failures`
- `expired_keys` (all removal paths) / `cleanup_runs`

Connection accounting uses an RAII guard, so EOF, protocol errors,
disconnects, and server errors all decrement the gauge. Aborting a task
(server shutdown) is the only path that bypasses it.

Limitations: passive-expiry removals during `GET` are counted, but
startup-time expiration cleanup is not; no latency histograms yet.

## STATS

`STATS` returns a bulk string; the first two lines keep the historical
shape (`keys:`, `operations:`), followed by `uptime:` and every counter
above. The CLI prints the body verbatim, so new fields appear
automatically.

## HEALTH

Unchanged and lightweight: `status`, `keys`, `operations`, `uptime`.
It still scans key counts under read locks but performs no persistence
or network checks. Use `STATS` for diagnostics.
