"""CI-only smoke test for the River release binary (Windows x86_64).

Runs the real server plus the real CLI end to end without depending on
PowerShell's handling of native-process stdio: the server's output goes to
a file, CLI stdin comes from a file (deterministic EOF), and every stage
has a hard timeout, so this harness cannot hang indefinitely.

Usage:
    python windows_smoke.py --exe path/to/river.exe --port 2107 --db path/to/smoke.db

Exits 0 on success, nonzero with a diagnostic on stderr on failure.
Stdlib only. Does not touch River source code.
"""

import argparse
import os
import socket
import subprocess
import sys
import tempfile
import time

READY_TIMEOUT_S = 30
CLI_TIMEOUT_S = 60
STOP_TIMEOUT_S = 10


def log(message):
    print(f"[smoke] {message}", flush=True)


def wait_for_port(host, port, deadline_s):
    """True once something accepts TCP on (host, port), False on timeout."""
    end = time.time() + deadline_s
    while time.time() < end:
        try:
            with socket.create_connection((host, port), timeout=2):
                return True
        except OSError:
            time.sleep(0.5)
    return False


def run_cli(exe, port, commands, env):
    """Run `exe cli` with commands on stdin (file-backed for real EOF)."""
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".txt", delete=False, encoding="utf-8"
    ) as handle:
        handle.write("".join(line + "\n" for line in commands))
        input_path = handle.name
    try:
        with open(input_path, "rb") as stdin_file:
            return subprocess.run(
                [exe, "cli", "--port", str(port)],
                stdin=stdin_file,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                timeout=CLI_TIMEOUT_S,
                env=env,
            )
    finally:
        os.unlink(input_path)


def main():
    parser = argparse.ArgumentParser(description="River Windows smoke test")
    parser.add_argument("--exe", required=True)
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--db", required=True)
    args = parser.parse_args()
    env = dict(
        os.environ,
        RIVER_DB_PATH=args.db,
        RIVER_ADDR=f"127.0.0.1:{args.port}",
    )

    # Stage 1: the binary starts and prints usage for bad subcommands.
    log("stage 1/3: usage check")
    try:
        usage = subprocess.run(
            [args.exe, "bad-command"],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=READY_TIMEOUT_S,
            env=env,
        )
    except subprocess.TimeoutExpired:
        return "usage check timed out: binary did not exit"
    if usage.returncode != 2 or "Usage: river" not in usage.stdout:
        return f"unexpected usage output (exit {usage.returncode}): {usage.stdout!r}"

    # Stage 2: boot the server with stdio detached to a file, then wait
    # for the port with raw TCP connects (no CLI involved).
    log("stage 2/3: starting server")
    server_log = tempfile.NamedTemporaryFile(
        mode="w", suffix=".log", delete=False, encoding="utf-8"
    )
    server_log.close()
    server = subprocess.Popen(
        [args.exe, "server"],
        env=env,
        stdout=open(server_log.name, "w", encoding="utf-8"),
        stderr=subprocess.STDOUT,
    )
    try:
        log("waiting for server port")
        if not wait_for_port("127.0.0.1", args.port, READY_TIMEOUT_S):
            return "server did not become ready (port never accepted)"

        # Stage 3: real CLI roundtrip. QUIT/EOF both exit the CLI cleanly.
        log("stage 3/3: CLI roundtrip")
        try:
            cli = run_cli(args.exe, args.port, ["PING", "GET missing", "QUIT"], env)
        except subprocess.TimeoutExpired:
            return "CLI roundtrip timed out"
        if cli.returncode != 0:
            return f"CLI did not exit cleanly: {cli.returncode}"
        body = cli.stdout.decode("utf-8", "replace")
        if "PONG" not in body:
            return f"PING roundtrip failed: {body!r}"
        if "(nil)" not in body:
            return f"GET-missing roundtrip failed: {body!r}"
    finally:
        log("stopping server")
        server.terminate()
        try:
            server.wait(timeout=STOP_TIMEOUT_S)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=STOP_TIMEOUT_S)

    log("smoke test passed")
    return None


if __name__ == "__main__":
    error = main()
    if error is not None:
        print(f"[smoke] FAILED: {error}", file=sys.stderr, flush=True)
        sys.exit(1)
