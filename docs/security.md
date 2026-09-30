# Security and Authentication

River has single-password authentication. There are no users, roles, or
ACLs — that is a deliberate later milestone, not an omission by accident.

Source:
- `src/auth.rs` (configuration, constant-time comparison)
- Session gating: `src/server/tcp.rs`
- `AUTH` execution: `src/commands/mod.rs`
- CLI auto-auth: `src/cli/repl.rs`, `src/cli/client.rs`

## Model

- Authentication is **disabled by default**. An open localhost server
  behaves exactly as before.
- Setting `RIVER_PASSWORD` to a non-empty value enables authentication.
  There is no default password; an empty value means disabled.
- Sessions are **per TCP connection**: the handler keeps a local
  `authenticated` flag, starting `false` when auth is enabled. Successful
  `AUTH` flips it for that connection only. Disconnecting discards it.
  Nothing is stored in the key-value store or on disk.
- `AUTH <password>` returns `+OK` on success, `-ERROR authentication
  failed` otherwise (generic — no hint about which part was wrong), and
  `-ERROR authentication not required` when auth is disabled.
- Before authentication, only `AUTH`, `QUIT`, and `EXIT` are accepted.
  Everything else — including `PING`, `STATS`, and `HEALTH` — is rejected
  with `-ERROR authentication required`, so unauthenticated clients learn
  nothing about the data.
- Passwords are compared in constant time (XOR-accumulate, no early exit
  on content). Passwords never appear in logs, errors, metrics, `STATS`,
  `HEALTH`, or tracing spans; `AuthConfig`'s `Debug` redacts the secret.

## Usage

```bash
RIVER_PASSWORD=s3cr3t cargo run -- server
RIVER_PASSWORD=s3cr3t cargo run -- cli
```

The CLI reads the same variable and authenticates automatically after
connecting (proceeding silently if the server turns out to be open).
There is intentionally no `--password` flag: secrets must not end up in
shell history. No echo-free prompt exists (it would need new
dependencies); the environment variable is the mechanism.

Manual RESP:

```text
*2
$4
AUTH
$6
s3cr3t
```

## Network exposure

- Default bind is `127.0.0.1:2007` (loopback only). This is the safe
  default and is unchanged.
- Localhost without a password is fine for development.
- Binding to a non-loopback interface exposes River to that network:
  enable `RIVER_PASSWORD` first, prefer binding to loopback plus an SSH
  tunnel or VPN, and treat the deployment as network-exposed.
- **Password authentication does NOT encrypt traffic.** Credentials and
  data travel in cleartext RESP. Do not use password auth as the sole
  protection on an untrusted network; provide transport security
  (tunnel/VPN/loopback) separately. No TLS in this milestone.

## Observability

`STATS` reports `auth_success`, `auth_failures`, `auth_required`, and
`commands_auth`. Failed attempts log one `warn` line each (no secret).
Brute force over TCP is inherently rate-limited by connection and
command processing; there is deliberately no lockout (fail-open
lockouts are a denial-of-service vector for a different milestone).

## What this does NOT provide

TLS/encryption, multiple users, roles, permissions, key-prefix ACLs,
auditing of who did what, password rotation, or protection against an
attacker who can read process memory/environment. Those need explicit
future milestones.
