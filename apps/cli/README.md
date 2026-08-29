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

It also provides high-level projections over the five Workbench operations already admitted by the
production Desktop catalog:

```text
orchbee project list [--limit <n>] [--cursor <cursor>]
orchbee directory list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee workspace list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee terminal list (--standalone | --project <project-id> --workspace <workspace-id>) [--limit <n>] [--cursor <cursor>]
orchbee terminal create (--standalone | --project <project-id> --workspace <workspace-id>) [--directory <directory-id>] [--detach] [--request-id <request-id>]
```

The production Desktop catalog also admits four read-only Agent-observation operations. Their
high-level projections are:

```text
orchbee agent list [--all] [--limit <n>] [--cursor <cursor>]
orchbee agent get <session-id>
orchbee agent explain <session-id>
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
waiting. `waitForExecution` repeatedly performs only the read-only execution lookup. In contrast,
`agent wait` invokes one server-owned asynchronous query over the canonical Agent snapshot source,
then observes that semantic execution through the same ledger. Aborting the CLI caller signal stops
only local execution observation; it never calls invocation or cancellation implicitly.

`agent wait` pins one exact Session ID. Its default conditions are attention, outcomes, idle, and
ended; repeated `--until` selects explicit semantic-status or `fresh`, `stale`, and `disconnected`
conditions. Semantic status matches only a fresh observation. `--after-revision` requires a strictly
newer global Agent runtime revision, and `--timeout` is bounded from 1 to 3,600,000 milliseconds with
a 600,000 millisecond default. Use `--detach` to retain the request or execution identity for later
observation or explicit `execution cancel`. A coordinator-generation change makes an in-flight wait
outcome unknown; the CLI never replays it.

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

The CLI never reads a Workbench data file, reads provider or screen state, connects to Terminal Host,
enumerates product Actions, or stores or reconstructs feature state. Its high-level commands
construct only inputs validated by the Workbench or Agent owner's exported semantic-command schemas
and verify the complete expected public operation descriptor before invocation. Agent queries return
only canonical bounded snapshots and projections; `agent explain` describes provenance, freshness,
capability, and the status basis without exposing prompts, transcripts, tool content, paths, provider
errors, or screen evidence. The coordinator owns catalog admission and executions; feature packages
own operation input, output, targeting, and effects; the protocol package owns only serializable
generic envelopes. Adding a CLI projection grants no operation authority.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/cli build
pnpm --filter @orchbee/cli typecheck
pnpm --filter @orchbee/cli test
pnpm --filter @orchbee/desktop test:native
pnpm desktop:package
```
