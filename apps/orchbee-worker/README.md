# Rust Orchbee Worker

Experimental, unshipped private Rust Worker application for the first POSIX local Server-Worker
process spine. The emitted executable is `orchbee-worker`. It participates in the root Cargo gate
but is not packaged, installed, activated by `orchbee`, or connected to Terminal Host.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Process boundary

On macOS and Linux, the exact private development invocation
`orchbee-worker --local-endpoint <absolute-path>` connects to the supplied Unix-domain socket,
requires the Server to have the same effective operating-system user, submits one bounded Worker
handshake with the executable cohort and fixed development-local Worker identity, and verifies the
typed admission response. On acceptance it writes `ready\n` to stdout and remains attached without
sending work. Server disconnect is a bounded unavailable outcome and exits status `1`. Invalid
invocation, connection, authentication, rejection, protocol, output, or transport failure is silent
and also exits status `1`.

The Worker performs no retry, discovery, activation, daemonization, child-process supervision,
filesystem inspection, shell resolution, Terminal Host connection, PTY operation, Agent behavior,
or durable write. It uses no current-directory, Project, Environment, Checkout, Worktree, or launch
profile fact. Endpoint selection and lifecycle supervision belong to later slices.

## Ownership and model mapping

The [`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.md) crate owns Worker
framing; [`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.md) owns the
kernel same-user proof. Worker owns only connect, authenticate, register, acknowledge, and
observe-Server-loss order. Server owns admission, active identity, and generation. The
[`LocalSiteProcessSpine`](../orchbee-server/formal/tla/LocalSiteProcessSpine.tla) model maps this
process to candidate, active Worker, disconnect, and Server-stop transitions; the Server guide owns
the complete modeling record and evidence limits.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-worker --all-targets --all-features --locked
cargo clippy -p orchbee-worker --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-local-peer-credentials --all-targets --all-features --locked
node scripts/tla/run-models.mjs --models local-site-process-spine
pnpm verify
```
