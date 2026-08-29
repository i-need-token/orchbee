# SSH Environment Runtime

`orchbee-ssh-environment-runtime` is the detached per-Environment daemon reached through a
replaceable SSH attachment. It is a native transport endpoint, not a remote shell and not yet a
remote terminal, Project, Git, or Agent service. Its only admitted filesystem behavior is the
bounded read-only `project-root-v1` observation described below.

## Commands

```text
orchbee-ssh-environment-runtime --version
orchbee-ssh-environment-runtime serve --runtime-directory <absolute-path> --instance-id <32-hex> --build-digest <64-hex>
orchbee-ssh-environment-runtime attach --runtime-directory <absolute-path> --instance-id <32-hex>
```

`serve` owns one daemon epoch, lifetime lock, private Unix-domain socket, and the exact signed
artifact digest admitted by the Desktop bootstrap. `attach` authenticates that same-user endpoint
and bridges only stdin/stdout protocol bytes. The Desktop bootstrap launches `serve` with inherited
streams detached and then replaces its remote shell with `attach`; this separation lets the SSH
child disappear while the daemon remains alive.

The Runtime directory, lock, and socket must be owned by the effective user, have exact owner-only
permissions, contain no extended ACL grant, and never follow a symlink at admission. The server also
checks the effective user identity of every local peer. One attachment may be active; a competing
attachment receives `runtime-attachment-conflict` and is closed.

## Protocol and lifetime

The Runtime implements version `0.1.0` from `@orchbee/ssh-environment-protocol`. It accepts one exact
Hello, returns the immutable artifact version and SHA-256 build digest plus a fresh daemon epoch,
and acknowledges heartbeats. Partial records and incomplete handshakes are bounded by time and size.
Disconnecting an attachment does not stop the daemon or change its epoch; restarting the daemon
always produces a new epoch.

The Runtime advertises exactly `project-root-v1`; Desktop rejects every other capability set. There
is deliberately no execute-command message. Later terminal or other services require their own
accepted owner contract and private Runtime-side adapter.

## Project-root inspection

One Desktop-initiated channel inspects one explicit absolute POSIX UTF-8 path. The Runtime follows
remote operating-system symlinks, requires the final target to be an accessible directory, and
returns only the canonical absolute path. Invalid path, absence, non-directory, permission,
deadline, capacity, and internal availability outcomes are mapped to the bounded protocol reasons;
raw operating-system errors never cross the channel or enter logs.

At most four inspections run per Environment. Each filesystem observation executes in a restricted
child of the same immutable Runtime binary; the path reaches that child only over stdin, stderr is
discarded, and stdout carries one private bounded result. The daemon polls those children alongside
the attachment, so filesystem latency cannot block heartbeat. Cancellation or attachment loss
kills the matching child, and the 30-second deadline forcibly terminates it before publishing
`timed-out`. A reconnect never inherits or replays an old channel.

This service does not list a directory, read file bytes, retain a handle, watch or mutate a path,
run a shell, or assign Project, Checkout, Workspace, or directory identity. Requested and canonical
paths remain process-local.

Daemon retention currently covers attachment loss while the user session and operating system keep
the process alive. Survival across a total logout or reboot requires a later reviewed service or
linger integration. No command removes old installed versions or remote work.

## Compatibility and release identity

The only admitted remote build targets are:

- `aarch64-macos`, reported on the wire as `darwin-arm64`; and
- `x86_64-linux-gnu`, reported on the wire as `linux-x64`.

Other targets fail at build selection rather than producing an unclaimed binary. Production builds
are pinned to Zig `0.16.0`, `ReleaseSafe`, stripping, and an explicit
`-Druntime-version=<semver>` supplied by the release pipeline. Version identity is never inferred
from Git state. After verifying the signed manifest and artifact bytes, the no-download bootstrap
passes that artifact's SHA-256 to `serve`; the daemon holds it immutably for its epoch and returns it
in Hello. This avoids the impossible circular requirement for a binary to embed its own final
digest.

## Offline release workflow

`release:prepare` cross-builds the exact matrix into a private candidate and atomically publishes
only the two artifacts plus canonical `manifest-signing-payload.json`. An external release authority
or HSM signs those exact payload bytes and returns one raw 64-byte Ed25519 signature. Repository code
never accepts or reads the production private key.

`release:finalize` independently receives the prepared directory, signature, and public key. It
rejects an expanded layout, symlink, non-regular file, noncanonical payload, wrong signature, or
artifact size/digest drift, then atomically publishes only the artifacts and canonical
`manifest.json`. Requested output paths must be absolute and absent:

```sh
pnpm --filter @orchbee/ssh-environment-runtime release:prepare --output /absolute/prepared-release --runtime-version 1.2.3
pnpm --filter @orchbee/ssh-environment-runtime release:finalize --prepared /absolute/prepared-release --signature /absolute/signature.bin --public-key /absolute/release-public-key.pem --output /absolute/final-release
```

Desktop package composition and the complete trust/rollback contract live in
[`../desktop/docs/ssh-runtime-release.md`](../desktop/docs/ssh-runtime-release.md).

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/ssh-environment-runtime build
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/ssh-environment-runtime format:check
```

The real-process conformance test starts the daemon in a private temporary directory, performs the
cross-language Hello and heartbeat, exercises canonical symlink resolution and bounded failure and
cancel paths, rejects a competing attachment, reconnects to the same epoch, and proves that a daemon
restart changes the epoch. The release tests use only ephemeral keys and also cover the fixed build
arguments, signing round trip, mutations, and exact layouts. No test contacts an SSH server or reads
a production signing key.
