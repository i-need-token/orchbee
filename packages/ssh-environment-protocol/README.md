# SSH Environment Protocol

Strict binary framing shared by Desktop and the per-Environment SSH Runtime. Version `0.4.0`
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

Hello acceptance identifies one immutable Runtime build, one release cohort, one Terminal Host
version/build, one daemon instance, one Runtime epoch, and the validated absolute login-Shell path
declared by the remote account. A reconnect may attach to the same epoch; a restarted Runtime
necessarily reports a new epoch. The Runtime advertises exactly the ordered
`[project-root-v2, terminal-host-v1]` pair, and Desktop requires it. There is no downgrade, dual-stack
negotiation, or placeholder capability.

## Project-root service contract

One `project-root-v2` channel carries exactly one strict request. Canonical inspection uses:

```json
{ "type": "inspect-project-root", "path": "/absolute/posix/path" }
```

Authenticated directory listing uses:

```json
{ "type": "list-project-directory", "path": null }
```

`null` selects the Runtime process's validated absolute `HOME`; an absolute path navigates directly.
Every path is valid UTF-8, absolute POSIX text, at most 4,096 encoded bytes, and contains no NUL or
control character. Neither side performs tilde, environment-variable, wildcard, command, or shell
expansion. The complete request and result are each at most 128 KiB, and each direction receives
exactly 128 KiB of channel credit.

A result is either `{"kind":"resolved","canonicalPath":"/..."}` or
`{"kind":"failed","reason":"..."}`. Failure reasons are closed to `invalid-path`, `not-found`,
`not-directory`, `permission-denied`, `timed-out`, `busy`, and `unavailable`. The channel identity is
the only correlation identity; the service schema introduces no Project, Checkout, Workspace, or
directory identity. A listing result contains the canonical path, an optional canonical parent, at
most 256 sorted child entries containing only a UTF-8 directory name of at most 255 bytes, and a
`truncated` flag. It exposes no file entry or file byte, and a symlink appears only when its target
can be opened as a directory.

## Terminal Host relay contract

One `terminal-host-v1` channel carries opaque Terminal Host protocol bytes. Each direction begins
with exactly 64 KiB credit; every data chunk is nonempty and at most 64 KiB. Credit is returned only
after the receiving boundary consumes bytes. One Runtime attachment admits one relay, while channel
identifiers remain globally monotonic across relay and project-root services.

Open failures are closed to `service-busy`, `service-unavailable`, and
`compatibility-unsupported` and remain channel-local. Attachment loss closes the relay without
replay; outer epoch, channel, framing, ordering, or credit corruption retires the attachment. This
contract neither defines nor parses Terminal Host records and grants no generic command authority.

## Offline release contract

Frozen release schema `1` defines the Runtime-only two-platform contract. Release schema `2`
separately defines the exact ordered `ssh-environment-runtime` and `terminal-host` components; each
component contains the exact ordered `darwin-arm64` and `linux-x64` artifacts and retains its own
semantic version. Every artifact has a nonzero size bounded at 128 MiB and a SHA-256 digest. Trust
schemas `1` and `2` bind canonical Ed25519 SPKI, its `ed25519-spki-sha256:<digest>` identity, and the
SHA-256 of the exact matching final-manifest bytes.

The signing payload has no trailing newline; the final manifest and trust descriptor each have one.
Schemas reject unknown fields, component or platform reordering, missing or unsupported entries,
noncanonical signatures, and out-of-bound values. Fixed directory and file names are exported with
the schemas so repository release automation and Desktop packaging cannot silently choose different
layouts or mix v1 and v2.

## Authority boundary

This package is a data contract only. It cannot build or sign an artifact, hold a private key,
verify package trust, compose a release, create a connection, install or start a component,
authenticate a peer, retry an attachment, authorize an inspection, or perform filesystem work. It
owns the exact `project-root-v2` schema/codec and the `terminal-host-v1` opaque relay envelope, but
deliberately has no generic execute-command record. Any additional Runtime service must be explicitly
advertised, admitted, authorized, and implemented by its owning subsystem.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/ssh-environment-protocol build
pnpm --filter @orchbee/ssh-environment-protocol typecheck
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-protocol test:exports
```
