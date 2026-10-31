# Local Peer Credentials

Private, experimental Rust adapter for proving that both ends of a local stream run as the
same effective operating-system user. Server, Worker, and local Client connection owners consume
this crate without sharing their framing, handshake, identity, admission, or lifecycle contracts.

## Contract

`require_same_effective_user` obtains the connected peer identity from the kernel and compares it
with the current effective user. Linux uses `SO_PEERCRED`; Apple and supported BSD targets use
`getpeereid`. The safe `nix` APIs own both platform calls. A mismatch returns
`PermissionDenied`; an unsupported Unix target returns `Unsupported`.

The caller already owns a connected local stream and remains responsible for endpoint selection,
permissions, connection order, timeouts, framing, role authentication, admission, and stream
lifetime. This crate performs no I/O on the stream, starts no thread or process, and retains no
identity.

## Limits

Native execution evidence covers same-user socket pairs on macOS and Linux. It does not execute a
cross-user negative case, authenticate a remote peer, or
claim hostile multi-tenant isolation. Same-user process separation limits accidental authority and
failure coupling; it is not a sandbox.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-local-peer-credentials --all-targets --all-features --locked
cargo clippy -p orchbee-local-peer-credentials --all-targets --all-features --locked -- -D warnings
pnpm verify
```

## Windows peer proof

For Windows `LocalStream` peers, `require_same_effective_user` delegates to `orchbee-local-ipc`: the pipe supplies the peer PID, a kernel process handle supplies its token SID, and that SID must equal the current token user. No PID file, username, or environment value is identity evidence. Native same-user tests exercise the adapter; a cross-user negative process test is not claimed.
