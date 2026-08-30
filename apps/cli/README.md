# Orchbee CLI

Product-owned Node client for invoking explicitly admitted Orchbee semantic commands. The workspace
contains the transport-independent client core, strict generic, Workbench, and Agent command
projections, the release-matched embedded Agent Skill, lifecycle runner, trusted local locator and
authenticated HTTP adapter, and the real process entry used by the Desktop distribution.
Desktop-created terminals receive one exact launcher path without a global installation.

## Command surface

The application runner accepts these generic command families:

```text
orchbee status
orchbee operation list
orchbee operation describe <operation-id>
orchbee operation invoke <operation-id> --input-file <file|-> [--detach] [--request-id <request-id>]
orchbee execution get <request-or-execution-id>
orchbee execution wait <request-or-execution-id>
orchbee execution cancel <request-or-execution-id>
```

It also provides high-level projections over the eight Workbench operations admitted by the
production Desktop catalog:

```text
orchbee project list [--limit <n>] [--cursor <cursor>]
orchbee directory list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee workspace list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee tab list (--standalone | --project <project-id> --workspace <workspace-id>) [--limit <n>] [--cursor <cursor>]
orchbee panel list (--standalone | --project <project-id> --workspace <workspace-id>) --tab <tab-id> [--limit <n>] [--cursor <cursor>]
orchbee terminal list (--standalone | --project <project-id> --workspace <workspace-id>) [--limit <n>] [--cursor <cursor>]
orchbee terminal read <presentation-id> [--rows <1-1000>] [--detach] [--request-id <request-id>]
orchbee terminal create (--standalone | --project <project-id> --workspace <workspace-id>) [--directory <directory-id>] [--detach] [--request-id <request-id>]
```

The production Desktop catalog also admits five read-only Agent-observation operations. Their
high-level projections are:

```text
orchbee agent list [--all] [--limit <n>] [--cursor <cursor>]
orchbee agent get <session-id>
orchbee agent explain <session-id>
orchbee agent changes [--cursor <cursor>] [--limit <n>] [--timeout <milliseconds>] [--detach] [--request-id <request-id>]
orchbee agent wait <session-id> [--until <condition>]... [--after-revision <revision>] [--timeout <milliseconds>] [--detach] [--request-id <request-id>]
```

`orchbee --skill` is the separate coordinator-free command for printing the embedded Skill.

Every coordinator-backed command accepts `--target <endpoint-id>` and `--output json|text`. Omitted
targets succeed only when trusted local discovery resolves exactly one live coordinator. Duplicate,
unknown, or cross-command options fail before connection. Generic invocation reads one strict UTF-8
JSON value from the named file or stdin. High-level commands validate bounded arguments through the
Workbench or Agent owner's exported schemas and map to exactly one existing operation. Workbench
pages accept 1-200 items; Agent pages accept 1-100 and default to live Sessions, while `--all`
includes retained ended Sessions. Invalid input is rejected before allocating a request identity or
contacting Desktop.

`tab list` preserves the canonical shared Tab collection order and returns Panel and Terminal
membership counts. `panel list` requires an exact returned Tab ID and preserves that Tab's canonical
depth-first, left-to-right Panel order. These queries expose no active, selected, focused,
maximized, active-child, split-weight, pixel-geometry, renderer, or screen state. Their returned
positions describe one revision only and never serve as mutation identities.

`terminal read` pins one exact `presentationId` returned by `terminal list` and reads the current
tail through a capture-only Terminal Host projection. It defaults to 80 rows, accepts at most 1,000,
and returns at most 128 KiB of plain UTF-8 text together with the exact generation, revision, row
bounds, and truncation state. Soft-wrapped rows are joined; ANSI escapes, styling, graphics,
hyperlinks, raw VT bytes, and future output are not exposed. Live local Terminals, already-connected
SSH Terminals, and retained local archives are eligible. The query never starts or reconnects a
Terminal, sends input, resizes, focuses, or retries across a stale frame. A stale, retired,
unavailable, cleanup-failed, cancelled, or five-second read-deadline outcome is explicit and never
returns old text. Output is not automatically redacted and may contain secrets; callers should not
echo, persist, or forward content they do not need. Only the existing bounded volatile semantic
execution ledger retains the result; this command is a one-shot read, not a wait or durable history.

`agent changes` is a global canonical change feed. The first invocation without `--cursor` returns
an empty baseline and a signed `nextCursor` at the current Agent runtime revision. Pass that cursor
back to receive at most 100 ordered applied transitions or to long-poll while it is current. Timeout
defaults to 30,000 milliseconds, is bounded from 1 to 600,000, and completes with an empty `current`
page. A `reset-required` page means the cursor fell behind the 512 retained in-memory changes: take a
new `agent list --all` snapshot, then continue with the reset page's cursor. Never treat reset as a
continuous feed. Change records contain only revision, canonical transition kind, and a content-free
Session summary.

Default output is exactly one compact versioned JSON document on stdout. Invoke results always
include the request identity. `--output text` is an explicitly unstable indented projection of that
same document; automation must not parse it. The current runner emits no payload-bearing stderr
diagnostics. `--skill` is the deliberate exception: it prints the canonical Agent Skill bundled in
the same build, performs no runtime file read, and makes no coordinator connection.

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
"$ORCHBEE_CLI" --skill
```

PowerShell uses the same injected path through `& $env:ORCHBEE_CLI status`. These are
distribution-internal, version-locked launch artifacts rather than a global executable,
package-manager installation, or stable third-party embedding API.

### Process model

| Field        | Contract                                                                                                                                                                                                                                      |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | A human, Agent, or Skill already running in a Desktop-owned Terminal needs one nonvisual invocation path to the same live local coordinator without global installation or another state owner.                                               |
| Inputs       | Arguments, high-level owner-backed inputs, file paths, file/stdin bytes, and signals are untrusted. The code-located launcher, embedded Skill, native addon location, and Desktop-selected runtime directory are trusted product composition. |
| Data         | The process owns no durable state. It holds one bounded input buffer, one caller-generated request identity when invoking, and one transient client connection.                                                                               |
| Outputs      | One versioned stdout document or the exact embedded Skill, plus one exit code. Current stderr is empty; operation input and result payloads never enter diagnostics.                                                                          |
| Invariants   | Parse precedes connection; input is capped before JSON decoding; one invocation is submitted at most once; one process writes its result once; no launch path changes operation authority.                                                    |
| Decisions    | Command grammar selects the runner branch; `--target` selects one exact validated endpoint; `--input-file -` selects bounded stdin instead of a bounded file; the output option selects only serialization.                                   |
| Model        | `runSemanticCommandCliProcess` composes the deterministic runner with filesystem, randomness, trusted locator, and native-transport capabilities. `startSemanticCommandCliProcess` owns signal registration and final writes.                 |
| Cancellation | `SIGINT` and `SIGTERM` abort only CLI-owned execution observation. They never issue semantic cancellation; business cancellation still requires `execution cancel`.                                                                           |
| Verification | Unit tests control input, configuration, signals, and writes. Desktop native integration launches the built entry with Node, Electron's Node mode, and the exact product launcher against one real private endpoint.                          |

## Client boundary

`SemanticCommandClient` depends on one injected `SemanticCommandClientPort`. It validates every
unknown response with `@orchbee/semantic-command-protocol`, rejects a mismatched coordinator
generation, and returns payload-free client errors for invalid input, malformed response, transport
failure, or locally cancelled observation.

The core supports status, operation list and description, owner-backed Workbench and Agent command
projections, invocation admission, execution lookup, cooperative cancellation, and bounded-backoff
waiting. `terminal read` invokes one server-owned, bounded Terminal Host projection; the client does
not inspect screen state itself. `waitForExecution` repeatedly performs only the read-only execution
lookup. In contrast, `agent wait` and a cursor-following `agent changes` invoke server-owned
asynchronous queries over the canonical Agent sources, then observe those semantic executions
through the same ledger. Aborting the CLI caller signal stops only local execution observation; it
never calls invocation or cancellation implicitly.

`agent wait` pins one exact Session ID. Its default conditions are attention, outcomes, idle, and
ended; repeated `--until` selects explicit semantic-status or `fresh`, `stale`, and `disconnected`
conditions. Semantic status matches only a fresh observation. `--after-revision` requires a strictly
newer global Agent runtime revision, and `--timeout` is bounded from 1 to 3,600,000 milliseconds with
a 600,000 millisecond default. Use `--detach` to retain the request or execution identity for later
observation or explicit `execution cancel`. A coordinator-generation change makes an in-flight wait
outcome unknown; the CLI never replays it.

`agent changes` is stateless at the CLI boundary. It neither caches events nor inspects providers,
processes, terminals, or screens. Use `--detach` only when retaining the exact execution identity for
later observation or cancellation of the long poll; cancellation never cancels Agent work. Signed
cursors are bound to the operation and coordinator generation and become invalid after restart.

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

The CLI never reads a Workbench data file, reads provider or screen state directly, connects to
Terminal Host, enumerates product Actions, or stores or reconstructs feature state. Its high-level
commands construct only inputs validated by the Workbench or Agent owner's exported
semantic-command schemas and verify the complete expected public operation descriptor before
invocation. Terminal text arrives only through the admitted Desktop operation and its capture-only
Host projection. Agent queries return only canonical bounded snapshots, projections, and
content-free change records; `agent explain` describes provenance, freshness, capability, and the
status basis without exposing prompts, transcripts, tool content, paths, provider errors, or screen
evidence. The coordinator owns catalog admission and executions; feature packages
own operation input, output, targeting, and effects; the protocol package owns only serializable
generic envelopes. Adding a CLI projection grants no operation authority.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/cli build
pnpm --filter @orchbee/cli typecheck
pnpm --filter @orchbee/cli test
pnpm --filter @orchbee/desktop test:native
pnpm desktop:package:local
```
