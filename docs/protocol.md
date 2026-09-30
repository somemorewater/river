# RESP-Inspired Protocol

River uses a small RESP-inspired protocol for structured TCP communication.

Source:
- `src/protocol/frame.rs`
- `src/protocol/resp.rs`

## Frame Types

River currently supports:

- `Frame::Simple(String)` encoded as `+value\r\n`
- `Frame::Integer(i64)` encoded as `:1\r\n`
- `Frame::Bulk(String)` encoded as `$len\r\nvalue\r\n`
- `Frame::Array(Vec<Frame>)` encoded as `*len\r\n...`
- `Frame::Error(String)` encoded as `-message\r\n`
- `Frame::Null` encoded as `$-1\r\n`

## Command Format

Commands are sent as arrays of strings:

```text
*2
$3
GET
$4
name
```

This becomes:

```text
["GET", "name"]
```

The command parser then validates the command name and argument count.

## Response Format

Examples:

```text
+PONG
```

```text
+OK
```

```text
:1
```

```text
$5
Water
```

```text
$-1
```

```text
-ERROR unknown command
```

## Stream Parsing

The decoder can return:

- Complete frame with bytes consumed
- Incomplete frame when more TCP bytes are needed
- Protocol error for malformed input

This lets the TCP server handle partial reads without assuming that one socket read equals one command.

## Current Scope

River implements a RESP-based (RESP-inspired) subset, not the full Redis protocol. The framing layer already supports pipelining (multiple frames per TCP read are drained in order) and LF-only manual input. Future work is richer Redis client compatibility, authentication, and replication — not additional core framing.
