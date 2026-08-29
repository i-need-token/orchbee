# Orchbee CLI

Product-owned Node client for invoking explicitly admitted Orchbee semantic commands. The workspace
contains the transport-independent client core, strict command parser and lifecycle runner, the
trusted local locator and authenticated HTTP adapter, and the real process entry used by the
Desktop distribution. Desktop-created terminals receive one exact launcher path without a global
installation.

## Command surface

The application runner accepts only these command families:

```text
orchbee status
orchbee operation list
orchbee operation describe <operation-id>
orchbee operation invoke <operation-id> --input-file <file|-> [--detach] [--request-id <request-id>]
orchbee execution get <request-or-execution-id>
orchbee execution wait <request-or-execution-id>
orchbee execution cancel <request-or-execution-id>
```

Every command accepts `--target <endpoint-id>` and `--output json|text`. Omitted targets succeed
only when trusted local discovery resolves exactly one live coordinator. Duplicate, unknown, or
cross-command options fail before connection. Invocation reads one strict UTF-8 JSON value from the
named file or stdin and rejects malformed or over-capacity input before allocating a request
identity or contacting Desktop.

Default output is exactly one compact versioned JSON document on stdout. Invoke results always
include the request identity. `--output text` is an explicitly unstable indented projection of that
same document; automation must not parse it. The current runner emits no payload-bearing stderr
diagnostics.

| Exit | Meaning                                                                              |
| ---- | ------------------------------------------------------------------------------------ |
| `0`  | Completed, unchanged, active, accepted for detached observation, or successful query |
| `2`  | CLI usage, option, input-source, JSON, or input-capacity failure                     |
| `3`  | Blocked, unsupported, policy-rejected, operation-unavailable, or request conflict    |
| `4`  | Cancelled terminal result                                                            |
| `5`  | Failed terminal result                                                               |
| `6`  | Target, transport, compatibility, receipt, internal, or unresolved-outcome failure   |

## Product launch contract

The Desktop and launcher composition uses these values. Desktop injects the first two into every
Terminal launch that it owns; the launcher owns the third:

| Name                            | Contract                                                                                                                                   |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `ORCHBEE_CLI`                   | Exact current-platform launcher path. Agents and Skills invoke this path directly; it is never added to `PATH`.                            |
| `ORCHBEE_CLI_RUNTIME_DIRECTORY` | Exact product-scoped rendezvous directory selected by Desktop. It is not derived from the current directory, Project, or repository files. |
| `ORCHBEE_CLI_NATIVE_ADDON_PATH` | Internal process value resolved and exported by the trusted launcher; Terminal callers do not select it.                                   |

On POSIX, `ORCHBEE_CLI` names `resources/orchbee-cli`; on Windows it names
`resources/orchbee-cli.cmd`. In a source checkout, the launcher resolves the sibling
`dist/main/orchbee-cli.cjs` entry and Desktop native addon and uses the repository's required Node
runtime. In a packaged application, the launcher resolves the colocated entry and addon and runs
the packaged Electron executable with `ELECTRON_RUN_AS_NODE=1`. Desktop packaging places all three
artifacts outside `app.asar`, verifies that they exist, and verifies the POSIX launcher's executable
mode. The launcher starts no coordinator and falls back to no TCP or project-local discovery.

For example, after Desktop creates a POSIX Terminal:

```sh
"$ORCHBEE_CLI" status
```

PowerShell uses the same injected path through `& $env:ORCHBEE_CLI status`. These are
distribution-internal, version-locked launch artifacts rather than a global executable,
package-manager installation, or stable third-party embedding API.

### Process model

| Field        | Contract                                                                                                                                                                                                                      |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A human, Agent, or Skill already running in a Desktop-owned Terminal needs one nonvisual invocation path to the same live local coordinator without global installation or another state owner.                               |
| Inputs       | Arguments, file paths, file/stdin bytes, and signals are untrusted. The code-located launcher, native addon location, and Desktop-selected runtime directory are trusted product composition.                                 |
| Data         | The process owns no durable state. It holds one bounded input buffer, one caller-generated request identity when invoking, and one transient client connection.                                                               |
| Outputs      | One versioned stdout document and one exit code. Current stderr is empty; operation input and result payloads never enter diagnostics.                                                                                        |
| Invariants   | Parse precedes connection; input is capped before JSON decoding; one invocation is submitted at most once; one process writes its result once; no launch path changes operation authority.                                    |
| Decisions    | Command grammar selects the runner branch; `--target` selects one exact validated endpoint; `--input-file -` selects bounded stdin instead of a bounded file; the output option selects only serialization.                   |
| Model        | `runSemanticCommandCliProcess` composes the deterministic runner with filesystem, randomness, trusted locator, and native-transport capabilities. `startSemanticCommandCliProcess` owns signal registration and final writes. |
| Cancellation | `SIGINT` and `SIGTERM` abort only CLI-owned execution observation. They never issue semantic cancellation; business cancellation still requires `execution cancel`.                                                           |
| Verification | Unit tests control input, configuration, signals, and writes. Desktop native integration launches the built entry with Node, Electron's Node mode, and the exact product launcher against one real private endpoint.          |

## Client boundary

`SemanticCommandClient` depends on one injected `SemanticCommandClientPort`. It validates every
unknown response with `@orchbee/semantic-command-protocol`, rejects a mismatched coordinator
generation, and returns payload-free client errors for invalid input, malformed response, transport
failure, or locally cancelled observation.

The core supports status, operation list and description, invocation admission, execution lookup,
cooperative cancellation, and bounded-backoff waiting. `waitForExecution` repeatedly performs only
the read-only lookup. Aborting the caller signal stops local observation; it never calls invocation
or cancellation and never creates a server-side waiter.

The runner invokes an operation at most once. After a lost response, it reconnects and observes the
same request identity only. A different coordinator generation returns
`outcome-unknown-after-restart`; it never replays the mutation. A positional execution reference is
resolved with read-only request and execution lookups. If both identities exist but identify
different executions, the runner returns `execution-reference-ambiguous` and does not cancel
either execution. Receipt absence is explicit and is never treated as successful completion.

The core has no HTTP, socket, named-pipe, SSH, Desktop, Browser, filesystem, or process dependency.
`LocalHttpSemanticCommandClientPort` is the separate Node adapter that maps that port to the bundled
local HTTP contract and validates every response before the core observes it.

## Trusted local connection

`connectLocalSemanticCommandCoordinator` accepts one trusted product runtime directory and either
the packaged native client addon or an injected equivalent. Without a selector, it enumerates only
strict `*.c.json` locator names in that directory. Zero live generation-matched coordinators returns
`coordinator-unavailable`; more than one returns `coordinator-ambiguous`. An explicit selector is
one exact validated endpoint identity and never means newest process, current directory, or a
project-supplied locator.

The native client validates locator ownership and privacy before reading it, connects only to the
corresponding Unix-domain socket or Windows named pipe, and authenticates the server as the current
effective OS user. It opens no TCP connection. Discovery probes `/v1/status` and discards a locator
whose coordinator generation does not match. Each later operation uses one bounded request and one
bounded response connection; transport loss is reported to the core and never causes invocation
replay, cancellation, or implicit Desktop startup. SSH, Browser access, remote targets, and command
presentation remain absent.

## Ownership boundary

The CLI never reads a Workbench data file, connects to Terminal Host, enumerates product Actions,
or interprets operation payloads. The coordinator owns catalog admission and executions; feature
packages own operation input, output, targeting, and effects; the protocol package owns only
serializable generic envelopes.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/cli build
pnpm --filter @orchbee/cli typecheck
pnpm --filter @orchbee/cli test
pnpm --filter @orchbee/desktop test:native
pnpm desktop:package
```
