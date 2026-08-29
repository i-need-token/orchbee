# SSH Environment Protocol

Strict binary framing shared by Desktop and the per-Environment SSH Runtime. Version `0.1.0`
provides an exact Hello handshake, Runtime instance and epoch identity, heartbeat liveness, channel
lifecycle records, binary channel data, and byte-credit flow control.

## Wire contract

Every record starts with one 16-byte header:

| Offset | Bytes | Meaning                                     |
| ------ | ----- | ------------------------------------------- |
| 0      | 4     | ASCII magic `OBSE`                          |
| 4      | 3     | Exact major, minor, and patch version bytes |
| 7      | 1     | Record kind                                 |
| 8      | 4     | Big-endian unsigned payload length          |
| 12     | 4     | Reserved zero bytes                         |

The entire record is bounded at 1 MiB. Control payloads are strict bounded UTF-8 JSON. Channel data
uses a four-byte big-endian channel identity followed by opaque bytes; credit records contain a
channel identity and byte count. Decoders reject unknown versions, kinds, fields, invalid UTF-8,
invalid JSON, nonzero reserved bytes, and inconsistent or oversized lengths.

Hello acceptance identifies one immutable Runtime installation build, one daemon instance, and one
daemon epoch. A reconnect may attach to the same epoch; a restarted daemon necessarily reports a new
epoch. The Runtime advertises exactly `project-root-v1`, and Desktop requires that exact singleton.
There is no downgrade, dual-stack negotiation, or placeholder capability.

## Project-root service contract

One `project-root-v1` channel carries exactly one strict request:

```json
{ "type": "inspect-project-root", "path": "/absolute/posix/path" }
```

The path is valid UTF-8, absolute POSIX text, at most 4,096 encoded bytes, and contains no NUL or
control character. Neither side performs tilde, environment-variable, wildcard, command, or shell
expansion. The complete request and result are each at most 16 KiB, and each direction receives
exactly 64 KiB of channel credit.

A result is either `{"kind":"resolved","canonicalPath":"/..."}` or
`{"kind":"failed","reason":"..."}`. Failure reasons are closed to `invalid-path`, `not-found`,
`not-directory`, `permission-denied`, `timed-out`, `busy`, and `unavailable`. The channel identity is
the only correlation identity; the service schema introduces no Project, Checkout, Workspace, or
directory identity.

## Offline release contract

Release schema `1` defines one canonical unsigned signing payload and one canonical signed manifest
for the exact ordered `darwin-arm64` and `linux-x64` artifact matrix. Each artifact has a nonzero
bounded size and SHA-256 digest, and both share one explicit semantic Runtime version. Trust schema
`1` binds canonical Ed25519 SPKI, its `ed25519-spki-sha256:<digest>` identity, and the SHA-256 of the
exact final manifest bytes.

The signing payload has no trailing newline; the final manifest and trust descriptor each have one.
Schemas reject unknown fields, platform reordering, unsupported platforms, noncanonical signatures,
and out-of-bound values. Fixed directory and file names are exported with the schemas so Runtime
release tooling and Desktop packaging cannot silently choose different layouts.

## Authority boundary

This package is a data contract only. It cannot build or sign an artifact, hold a private key,
verify package trust, compose a Desktop release, create a connection, install or start a Runtime,
authenticate a peer, retry an attachment, authorize an inspection, or perform filesystem work. It
owns the exact `project-root-v1` schema and codec, but deliberately has no generic execute-command
record. Any additional Runtime service must be explicitly advertised, admitted, authorized, and
implemented by its owning subsystem.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/ssh-environment-protocol build
pnpm --filter @orchbee/ssh-environment-protocol typecheck
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-protocol test:exports
```
