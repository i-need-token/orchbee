# SSH Environment Runtime

`orchbee-ssh-environment-runtime` is the detached per-Environment daemon reached through a
replaceable SSH attachment. It is a native transport endpoint, not a remote shell. It owns the
bounded read-only `project-root-v2` observation and one opaque `terminal-host-v1` relay, but creates
no terminal, Project, Workspace, Git, or Agent effect itself.

## Commands

```text
orchbee-ssh-environment-runtime --version
orchbee-ssh-environment-runtime serve \
  --runtime-directory <absolute-path> --instance-id <32-hex> \
  --build-digest <64-hex> --release-cohort-digest <64-hex> \
  --terminal-host-version <semver> --terminal-host-build-digest <64-hex> \
  --terminal-host-executable <absolute-path> \
  --terminal-host-runtime-directory <absolute-path> \
  --terminal-host-endpoint-id <32-hex>
orchbee-ssh-environment-runtime attach --runtime-directory <absolute-path> --instance-id <32-hex>
```

`serve` owns one daemon epoch, lifetime lock, private Unix-domain socket, and the exact signed cohort
identity admitted by Desktop bootstrap. `attach` authenticates that same-user endpoint and bridges
only stdin/stdout protocol bytes. Before that authenticated connection exists, `attach` retries the
private endpoint for at most five seconds so an abruptly stopped daemon's stale socket cannot race
its replacement; after connection, bridge loss terminates the attachment immediately. The remaining
internal worker commands are exact private ABIs and grant no caller-selected command, shell, or path
execution.

The Runtime directory, lock, and socket must be owned by the effective user, have exact owner-only
permissions, contain no extended ACL grant, and never follow a symlink at admission. The server
binds and secures an unpublished same-directory socket before atomically publishing the admitted
endpoint name, so the public path is never observable with permissive creation-mode permissions. It
also checks the effective user identity of every local peer. One attachment may be active; a
competing attachment receives `runtime-attachment-conflict` and is closed.

## Protocol and lifetime

The Runtime implements exact version `0.4.0` from `@orchbee/ssh-environment-protocol`. It accepts one
exact Hello, returns Runtime build identity, release cohort identity, Terminal Host version/build,
platform, the validated absolute login-Shell path declared by the remote account, ordered
capabilities, and a fresh daemon epoch, then acknowledges heartbeats. Partial records and incomplete
handshakes are bounded by time and size. Disconnecting an attachment does not stop the Runtime or
change its epoch; restarting it always produces a new epoch.

The Runtime advertises exactly `[project-root-v2, terminal-host-v1]`; Desktop rejects every other
capability set. Channel identifiers are globally monotonic for an attachment. There is deliberately
no execute-command message, generic service dispatch, forwarding, or network listener.

## Project-root service

One Desktop-initiated channel either inspects one explicit absolute POSIX UTF-8 path or lists one
directory. A null listing path resolves the Runtime process's validated absolute `HOME`. The Runtime
follows remote operating-system symlinks, requires the final target to be an accessible directory,
and returns the canonical absolute path. Invalid path, absence, non-directory, permission, deadline,
capacity, and internal availability outcomes map to bounded protocol reasons; raw operating-system
errors never cross the channel or enter logs.

At most four operations run per Environment. Each filesystem observation executes in a restricted
child of the same immutable Runtime binary; the path reaches that child only over stdin, stderr is
discarded, and stdout carries one private bounded result. The daemon polls those children alongside
the attachment, so filesystem latency cannot block heartbeat. Cancellation or attachment loss
kills the matching child, and the 30-second deadline forcibly terminates it before publishing
`timed-out`. If the sole result was already emitted when Desktop cancellation crosses it, Runtime
accepts the cancelled close; ordered delivery lets Desktop discard that result before the close
acknowledgement. A reconnect never inherits or replays an old channel.

A listing returns at most 256 lexically sorted, accessible UTF-8 child-directory names plus canonical
path, optional parent, and truncation metadata within 128 KiB. It omits files, invalid or inaccessible
entries, and symlinks whose targets cannot be opened as directories. The service never reads file
bytes, retains a handle, watches or mutates a path, runs a shell, or assigns Project, Checkout,
Workspace, or directory identity; path and listing metadata remain transient and never enter logs or
durable state.

## Terminal Host relay

One `terminal-host-v1` channel connects to the configured same-UID private Host endpoint. If that
endpoint is absent, an isolated child launches only the exact configured signed executable with
fixed `serve` arguments and retries the endpoint. It never removes an endpoint, kills or replaces an
existing Host, selects `PATH`, or parses Terminal Host protocol bytes.

The relay admits one channel, 64 KiB credit in each direction, and nonempty chunks up to 64 KiB.
Endpoint connection, launch, and blocking Host I/O remain in the child; the Runtime event loop polls
its nonblocking pipes beside heartbeat and four project-root workers. Busy, unavailable, and
compatibility outcomes are channel-local. Attachment corruption retires the attachment; attachment
loss kills only the relay worker and never replays an outcome-unknown Host effect. Terminal Host owns
its epoch, retained terminals, inventory, PTYs, and process trees.

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

These application-owned commands remain the frozen schema-version-1 Runtime-only workflow.
Schema-version-2 SSH Environment releases use repository-level
`release:ssh-environment:prepare` and `release:ssh-environment:finalize` to assemble this
workspace's parameter-fixed artifact with the separately built Terminal Host artifact. This
workspace neither imports Terminal Host source nor owns the combined cohort.

Desktop package composition and the complete trust/rollback contract live in
[`../desktop/docs/ssh-runtime-release.md`](../desktop/docs/ssh-runtime-release.md).

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/ssh-environment-runtime... build
pnpm --filter @orchbee/terminal-host build
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/ssh-environment-runtime format:check
```

Build the declared protocol dependencies and the Terminal Host conformance artifact before running
the focused test. The Runtime test lifecycle builds only its own native artifact; it never rebuilds
or temporarily removes a sibling package output while the recursive repository test gate may be
consuming that output. The root `pnpm verify` gate already enforces this build-before-test order.

The real-process conformance test starts the Runtime and actual Terminal Host in private temporary
directories, performs both cross-language Hellos and heartbeat, checks relay credit, exercises
canonical symlink resolution, bounded failure/cancel paths, and result-cancellation crossover,
rejects a competing attachment, reconnects to the same Runtime epoch, forces a daemon crash with a
stale endpoint, proves an already waiting attachment reaches the replacement Runtime's new epoch,
and proves that Runtime restart changes the epoch. The release tests use only ephemeral keys and
also cover fixed build arguments,
signing round trips, mutations, and exact layouts. No test contacts an SSH server or reads a
production signing key.
