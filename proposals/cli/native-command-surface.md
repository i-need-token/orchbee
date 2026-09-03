# Native Orchbee Command Surface

| Field              | Value                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Replacement direction is accepted, the additive Phase 3A native-entry foundation is complete, and the bare local interactive route plus bounded lazy activation are accepted pending implementation; broader command grammar, stable output and exit contracts, remote targeting, compatibility window, and cutover remain reserved                                                         |
| Owner              | Product command experience and the maintainers of the Rust `orchbee` Client, Server semantic-command boundary, TUI, packaging, and release integration                                                                                                                                                                                                                                      |
| Current authority  | [`apps/cli/README.md`](../../apps/cli/README.md) remains authoritative for the delivered TypeScript CLI until cutover                                                                                                                                                                                                                                                                       |
| Parent design      | [`native-runtime-topology.md`](../platform/native-runtime-topology.md) owns executable topology and process responsibility; this proposal owns the public Rust command surface                                                                                                                                                                                                              |
| Decision authority | The product owner accepted replacing and deleting `apps/cli`, implementing CLI and TUI in one public Rust `orchbee` executable, redesigning commands and parameters without preserving the current experimental grammar, allowing bounded framework-neutral Workbench semantic snapshots as CLI output, the exact unshipped Phase 3A foundation, and the bare local interactive route below |
| Exit condition     | Deliver the accepted Rust command slices, switch the packaged public entry with rollback evidence, delete `apps/cli` and obsolete launch artifacts, move maintained behavior into the new application guide and tests, then delete this proposal                                                                                                                                            |

Simplified Chinese review translation:
[native-command-surface.zh-CN.md](native-command-surface.zh-CN.md).
English remains canonical.

## Accepted product boundary

- `orchbee` is the only public native executable name. Server, Worker, Terminal Host, and Agent
  Bridge remain private implementation processes.
- Bare `orchbee` selects the interactive TUI. Explicit commands and options select one-shot CLI,
  lifecycle, help, or version behavior; grammar beyond the exact Phase 3A foundation below remains
  reserved.
- When the local Site is absent, bare `orchbee` may initiate bounded lazy activation of the exact
  compatible installed Server and local Worker cohort and wait for readiness. Once ready, the
  private processes own their lifetimes; `orchbee` never remains their supervisor.
- Every TUI invocation is one independent Client Session under the same model as a Web tab,
  Desktop window, or mobile Client. It owns local focus, viewport, scroll, and presentation state,
  but no shared Site or terminal authority merely because it initiated activation.
- `--help` and `--version` remain side-effect free and never discover or activate private services.
- One-shot CLI execution does not initialize TUI rendering or load Server, Worker, Terminal Host, or
  Agent engines into the public process.
- The Rust command surface is a new Client implementation. It does not inherit the current
  TypeScript parser, command names, flags, output schema, exit-code table, locator protocol,
  `ORCHBEE_CLI` variables, embedded Skill layout, or Electron-as-Node launcher contract.
- Existing semantic capabilities are migration inputs, not automatic compatibility requirements.
  Each capability appears in the new CLI only after its owning domain explicitly admits a new
  command contract.
- A read-only CLI operation may request a bounded versioned Workbench semantic projection snapshot
  from Server. The snapshot is structured product data, not HTML, a React tree, pixels, or another
  Client's local focus, viewport, selection, or window state; Web, TUI, and mobile render their own
  interfaces from the shared contract.

This acceptance changes future design only. The existing TypeScript application and its current
guides remain authoritative until a separately accepted replacement slice switches the shipped
entry. No source or package is deleted by this proposal alone.

## Accepted bare local interactive route

The local-terminal milestone in
[`native-runtime-topology.md`](../platform/native-runtime-topology.md) accepts the observable bare
invocation behavior. This proposal owns its command-facing boundary:

| Concern          | Accepted behavior                                                                                                                                                                                 |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Invocation       | An empty argument vector selects the interactive TUI route. No alternate public executable or hidden mode is required from the user.                                                              |
| Existing Site    | Connect to the exact compatible local Server and create a new Client Session; another invocation creates another peer session rather than reusing local view state.                               |
| Absent Site      | Initiate bounded lazy activation of the exact installed private Server and local Worker cohort, wait for a typed ready, unavailable, or incompatible result, then enter TUI only after readiness. |
| Lifetime         | Activation is a bootstrap effect, not parent-child supervision. Closing the TUI releases only its Client Session and attachments; it does not stop the Site, services, or retained terminals.     |
| Boundary         | The public process neither embeds backend engines nor connects directly to Terminal Host. It reaches terminal behavior through the accepted Server-relayed Client path.                           |
| Deferred surface | Explicit Site lifecycle commands, one-shot business commands, stable automation output and exit categories, remote targeting, completion, Skills, packaging, and public cutover remain reserved.  |

## Problem

Users should install and remember one command while choosing between an interactive TUI and
automation-friendly one-shot operations. The current `apps/cli` was built as a Desktop-bundled Node
projection over an experimental local coordinator. Its grammar, locator, output, launcher, and
packaging assumptions do not describe the new independent Server/Worker topology and would force a
new architecture to preserve an accidental interface.

## Scope

This proposal owns:

- the public `orchbee` invocation modes and command hierarchy;
- argument, input, output, diagnostic, and exit contracts;
- local Site discovery and explicitly admitted activation commands;
- the boundary between human text and versioned automation output;
- CLI/TUI process isolation inside one Rust artifact;
- compatibility, cutover, rollback, and deletion of the current `apps/cli`; and
- evidence that the public process grants no backend or arbitrary-command authority.

## Non-goals

This proposal does not:

- own Server state, operation catalogs, Worker placement, PTY/VT state, Agent semantics, or
  Workbench transitions;
- define Terminal Host, Server-Worker, or Agent Bridge wire protocols;
- expose private executable names as normal user commands;
- preserve every current CLI command or add a compatibility alias by default;
- turn terminal-create permission into raw shell, arbitrary argv, filesystem, Git, or generic RPC
  authority;
- require TUI initialization for CLI use or a running Desktop application; or
- authorize deletion before the selected replacement capabilities and rollback cohort are verified.

## Process and authority model

| Concern                                                                  | Authority                                         |
| ------------------------------------------------------------------------ | ------------------------------------------------- |
| Command parsing and presentation                                         | One `orchbee` invocation                          |
| Human session, semantic admission, and Workbench projection snapshots    | Server                                            |
| Worker selection and effect authorization                                | Server                                            |
| Node-local execution                                                     | Exact Worker and its specialized capability owner |
| PTY, VT, history, and terminal lifecycle                                 | Exact Terminal Host                               |
| TUI focus, viewport, selection presentation, and local interaction state | Exact TUI Client Session                          |

The CLI is stateless across invocations unless a separately accepted Client-local preference owns
an explicit store. It submits semantic intent and validates typed results. It never reads product
state files, discovers a Project from the current directory, connects directly to Terminal Host,
or treats terminal output as Agent or Workbench authority.

## Command-design contract

The future grammar must decide each dimension explicitly before parser implementation:

| Dimension       | Required decision                                                                                                                                                           |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Modes           | Bare TUI, one-shot resource commands, Site lifecycle commands, help, version, completion, and any machine-only discovery mode                                               |
| Nouns and verbs | Stable product vocabulary, nesting, aliases, abbreviation policy, and how unsupported capabilities are reported                                                             |
| Targeting       | Local Site default, explicit remote Site or Worker selection, identity validation, ambiguity, and no-target behavior                                                        |
| Input           | Positional values, options, stdin/files, size bounds, secret classification, confirmation, and noninteractive failure                                                       |
| Output          | Human projection, bounded Workbench semantic snapshots, versioned automation schema, stdout/stderr ownership, streaming, pagination, redaction, and compatibility lifecycle |
| Completion      | Synchronous result, detached request identity, observation, wait, cancellation, signals, timeout, and unknown-outcome behavior                                              |
| Exit            | Stable categories for usage, rejection, unavailable service, failed effect, cancellation, transport loss, incompatibility, and unknown outcome                              |
| Activation      | Which commands may request service activation, which are observation-only, and how version mismatch or partial cohort installation fails                                    |
| Extensibility   | How an owner admits a new semantic command without exposing an operation registry, raw RPC, private executable, or another parser authority                                 |

Automation consumes only the explicitly versioned output mode selected by the accepted command
contract. Human-formatted output is never an automation API. A mutating invocation carries one
request identity, submits at most once, and reconciles only that identity after an ambiguous
response; it never replays an effect automatically.

## Accepted Phase 3A native-entry foundation

The product owner authorized this additive slice on 2026-09-02. It creates a real but unshipped
Rust application boundary and a measurable process baseline without pretending that a TUI, Server
connection, or business command already exists.

| Authorization field   | Accepted decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem               | The target architecture names one public native `orchbee` process, but the repository has no application artifact, deterministic invocation boundary, localized native diagnostics, or reproducible startup measurement for it. Implementing a business command or placeholder TUI first would force unaccepted Server, transport, activation, and interface decisions.                                                                                                                                                                                           |
| Scope                 | Add `apps/orchbee` as an experimental Rust Cargo application and produce the unshipped `orchbee` binary. The only admitted argument vectors are empty, exactly `--help`, and exactly `--version`. Empty arguments select the TUI route, which reports a localized development-unavailable result because TUI composition is outside this slice. Help and the unavailable/usage diagnostics ship in `en-US` and `zh-CN`; the version line is locale-neutral. Add a repeatable release-profile baseline measurement.                                                |
| Non-goals             | No TUI renderer, Workbench command, Server or Worker discovery, activation, IPC, network, authentication, persistence, terminal access, child-process supervision, shell completion, Skill, business command, package-manager artifact, Desktop packaging change, public-entry cutover, compatibility alias, current CLI migration, or deletion is admitted.                                                                                                                                                                                                      |
| Claims                | Parsing is exact and side-effect free. Help and version complete successfully without initializing a TUI or backend capability. Empty arguments reach only the explicit unavailable TUI presentation. Any other, additional, or non-Unicode argument is a usage failure before effects. Help writes only stdout; unavailable and usage diagnostics write only stderr; version writes one `orchbee <version>` line to stdout. Every authored human message has both locales.                                                                                       |
| Boundaries            | `apps/orchbee` owns only its process entry, invocation decision, locale projection, and development presentation. It has no runtime dependency beyond the Rust standard library and grants no Server, Worker, Host, filesystem, or process capability. `apps/cli` and its Desktop launcher remain the only delivered CLI authority. The new binary is a Cargo verification input but not a release or packaging input.                                                                                                                                            |
| Evidence              | Table-driven parser and locale tests cover every admitted vector, extra arguments, non-Unicode input where supported, locale precedence, and fallback. Process tests assert exit status and exact stdout/stderr for both locales. Cargo dependency metadata and source review establish the absence of backend/TUI/process dependencies. The release measurement records artifact bytes, repeated warm `--version` latency, and peak resident memory where the host exposes it. Root format, lint, build, test, project, and documentation gates remain required. |
| Delivery and rollback | Record this accepted slice first; add the application contract and source; integrate it into the existing Cargo workspace; collect the baseline; and converge documentation and gates. The application is additive, unshipped, owns no durable state, and changes no current process. Rollback removes the application member and its accepted-slice record without data migration or compatibility handling.                                                                                                                                                     |

The exact Phase 3A reference result and its maintenance lifecycle are recorded in the application
guide's [Measurement](../../apps/orchbee/README.md#measurement) section.

### Modeling record

| Field                   | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Inputs                  | The argument vector and `LC_ALL`, `LC_MESSAGES`, and `LANG` values are untrusted process input. Locale selection checks those variables in precedence order, recognizes a `zh` language prefix, and otherwise falls back to `en-US`; it never persists or authorizes from locale data.                                                                                                                                                           |
| Outputs                 | Help and version return development exit status `0`; the unavailable bare TUI route returns `1`; invalid invocation returns `2`. These numeric statuses describe this unshipped foundation only and do not accept the future stable exit table. A result contains at most one static stdout payload and one static stderr payload; the admitted cases never populate both.                                                                       |
| Decisions               | Zero arguments select `Tui`; one exact `--help` selects `Help`; one exact `--version` selects `Version`; every other vector selects `UsageFailure`. There are no short flags, aliases, abbreviations, combined options, target inference, or current-directory behavior.                                                                                                                                                                         |
| Model                   | A pure parser maps arguments to one invocation variant or usage failure. A pure presenter maps that decision plus locale to one complete process result. The process adapter reads bounded inputs, writes each selected stream once, and maps the development status to the operating-system exit code.                                                                                                                                          |
| Dependencies and state  | Production code uses only standard-library argument, environment, and stream capabilities. It opens no file or socket, starts no process or thread, initializes no terminal mode, reads no repository or product state, and owns no durable or mutable cross-invocation state.                                                                                                                                                                   |
| Verification and limits | Exhaustive examples cover the finite admitted grammar and both supported locale projections. No TLA+ or property model is selected because the slice has no asynchronous event, retry, mutation, persistence, resource lifecycle, or independently ordered transition. Startup measurements are reference observations on the named host and profile, not cross-platform budgets, cold-cache guarantees, or proof of future TUI/CLI performance. |

## Migration and rollback

1. Inventory every real consumer of `apps/cli`, its packaged launchers, `ORCHBEE_CLI` variables,
   embedded Skill, current command families, output, and exit meanings.
2. Select the first one-shot Rust command cohort and record its exact grammar, schemas, activation policy,
   identities, bounds, and excluded legacy capabilities.
3. Implement the Rust CLI core independently from TUI rendering and backend engines, then add TUI
   composition without changing CLI startup dependencies.
4. Prove only deliberately retained semantic behavior against its owner contract. Do not use parser
   parity as the acceptance oracle.
5. Package one compatible `orchbee`/Server/Worker/Host cohort and atomically switch the public entry.
   Before any incompatible durable migration, rollback restores the complete prior cohort.
6. After the replacement cohort and rollback path are verified, delete `apps/cli`, its Desktop
   launcher and injected variables, obsolete protocol/Skill consumers, tests, documentation, and
   project contract in one accepted deletion slice.

No transition state exposes two authoritative public command surfaces under the same installation.
The old implementation may coexist in source during development, but packaging selects exactly one.
Phase 3A is the deliberately unshipped source-coexistence state: it completes neither selection of
the first business-command cohort nor the packaged-entry cutover.

## Evidence plan

| Claim                                                                                        | Required evidence                                                                                                       |
| -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| CLI startup does not initialize TUI or backend engines                                       | Dependency and process-start tests, cold/warm startup measurements, artifact-size and peak-memory measurements          |
| One invocation cannot gain Server, Worker, or Host authority                                 | Capability-negative tests, malformed-input tests, authentication tests, and direct-Host connection rejection            |
| Mutating commands submit at most once                                                        | Request-identity property tests, transport-loss traces, restart reconciliation, and unknown-outcome tests               |
| Human output is not the automation contract                                                  | Versioned-schema conformance, stdout/stderr tests, redaction tests, and compatibility fixtures                          |
| Bare invocation and explicit CLI modes do not select ambiguous paths                         | Parser decision-table tests, help/version tests, TUI bootstrap tests, and unsupported-mode cases                        |
| Bare local activation creates a peer Client Session without owning service lifetime          | Absent/present/incompatible cohort process tests, simultaneous-TUI traces, and Client-exit retention tests              |
| Phase 3A has no hidden backend or TUI initialization and localizes every authored message    | Dependency metadata, exact process-output tests in both locales, and release-profile startup/size/memory baseline       |
| Cutover leaves one public entry and one compatible service cohort                            | Packaged installation, upgrade, rollback, partial-installation, and stale-private-executable tests                      |
| `apps/cli` deletion removes no deliberately selected capability                              | Consumer scan, selected-capability traceability, packaging diff, and current-authority/documentation cleanup checks     |
| CLI Workbench snapshots expose product semantics without Web or another Client's local state | Server projection fixtures, schema bounds, authorization/redaction tests, and negative HTML/DOM/pixel/client-view cases |

## Reserved decisions

1. Define the complete command tree, option names, aliases, and help organization beyond the
   accepted bare local route and exact `--help` and `--version` vectors.
2. Select human and automation output formats, versioning, streaming, pagination, diagnostics, and
   stable exit categories, including the exact Workbench semantic snapshot command and schema.
3. Decide remote Site targeting and authentication, explicit lifecycle commands, and production
   service installation and activation mechanics beyond the accepted bounded local bootstrap.
4. Select which current semantic capabilities, if any, enter the first Rust release and which are
   removed without compatibility aliases.
5. Define the installed Skill and shell-completion distribution model independently from the old
   embedded-Skill contract.
6. Set measured startup, artifact-size, and peak-memory budgets for supported platforms; the
   Phase 3A reference baseline does not set an acceptance budget.
7. Define the exact packaged cutover, update cohort, compatibility rejection, rollback, and
   `apps/cli` deletion authorization.
