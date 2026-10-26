# Native Orchbee Command Surface

| Field              | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Replacement direction and the target TUI, bare activation, first CLI information architecture, structured-output, mutation-safety, packaging, and service-lifecycle contracts are accepted; the additive Phase 3A native-entry foundation and checked Client-side local activation model are complete, while runtime implementation, exact wire/schema details, remote targeting, public release cutover remain future work; legacy source retirement is complete under its independent authorization |
| Owner              | Product command experience and the maintainers of the Rust `orchbee` Client, Server semantic-command boundary, TUI, packaging, and release integration                                                                                                                                                                                                                                                                                                                                                |
| Current authority  | [`apps/orchbee/README.md`](../../apps/orchbee/README.md) owns delivered native CLI/TUI behavior; the TypeScript entry has retired.                                                                                                                                                                                                                                                                                                                                                                    |
| Parent design      | [`native-runtime-topology.md`](../platform/native-runtime-topology.md) owns executable topology and process responsibility; this proposal owns the public Rust command surface                                                                                                                                                                                                                                                                                                                        |
| Decision authority | The product owner accepted one public Rust `orchbee` for CLI and TUI; the peer-Client TUI model and initial full-screen interaction contract; the resource-first command families below; human and versioned machine output; bounded input and at-most-once mutation rules; one public packaged entry; and the accepted bare local activation and user-service surfaces below                                                                                                                         |
| Exit condition     | Deliver the remaining accepted native command slices and release boundary, then retire this proposal. Legacy source deletion is authorized independently of new packaging.                                                                                                                                                                                                                                                                                                                            |

Simplified Chinese review translation:
[native-command-surface.zh-CN.md](native-command-surface.zh-CN.md).
English remains canonical.

## Accepted product boundary

- `orchbee` is the only public native executable name. Server, Worker, Terminal Host, and Agent
  Bridge remain private implementation processes.
- Bare `orchbee` selects the interactive TUI. Explicit `orchbee tui` selects the same surface;
  explicit resource commands, Session lifecycle commands, help, version, and completion select
  one-shot behavior without initializing the TUI.
- When the local Session is absent, bare `orchbee` may initiate bounded lazy activation of the exact
  compatible installed Server and local Worker cohort and wait for readiness. Once ready, the
  private processes own their lifetimes; `orchbee` never remains their supervisor.
- Every TUI invocation is one independent Client Session under the same model as a Web tab,
  Desktop window, or mobile Client. It owns local focus, viewport, scroll, and presentation state,
  but no shared Session or terminal authority merely because it initiated activation.
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

| Concern          | Accepted behavior                                                                                                                                                                                                                                          |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Invocation       | An empty argument vector selects the interactive TUI route. No alternate public executable or hidden mode is required from the user.                                                                                                                       |
| Terminal device  | Bare and explicit TUI routes require an interactive TTY and never open `/dev/tty` implicitly. Help, version, and machine-readable one-shot output remain pipeable.                                                                                         |
| Existing Session | Connect to the exact compatible local Server and create a new Client Session; another invocation creates another peer session rather than reusing local view state.                                                                                        |
| Absent Session   | Under one per-user activation lock, initiate the exact installed private Server and local Worker cohort, converge concurrent invocations on that singleton, and enter TUI only after typed readiness. Terminal Host remains lazy until first terminal use. |
| Lifetime         | Activation is a bootstrap effect, not parent-child supervision. Closing the TUI releases only its Client Session and attachments; it does not stop the Session, services, or retained terminals.                                                           |
| Boundary         | The public process neither embeds backend engines nor connects directly to Terminal Host. It reaches terminal behavior through the accepted Server-relayed Client path.                                                                                    |
| Feedback         | A fast successful bootstrap stays silent. A delayed bootstrap shows localized progress. One monotonic total deadline ends in a typed unavailable or incompatible diagnostic; `orchbee` never kills, replaces, or downgrades an existing cohort to recover. |
| Deferred detail  | Exact timeout values and progress thresholds, remote targeting, detailed command schemas, public release cutover remain reserved. Legacy source retirement is separately accepted.                                                                         |

The checked model and its maintained bounds now live in the
[`apps/orchbee` guide](../../apps/orchbee/README.md#accepted-local-session-activation-modeling-record).
It supplies design evidence only; the current bare route remains unwired and unavailable.

## Accepted TUI Client contract

TUI is a peer Workbench Client, at the same architectural level as Web, Desktop, and mobile. It is
not a global terminal picker and does not introduce a second Workbench model.

| Concern               | Accepted behavior                                                                                                                                                                                                                                                                            |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shared document       | Server-owned Workbench state contains Project, Workspace, terminal context, Tab, Panel, and Presentation membership and ordering. A mutation accepted from any Client is published to every authorized Client.                                                                               |
| Client view           | Each TUI invocation owns its active location, active Tab, focus, layout projection, viewport, scroll, search, and selection presentation. Opening another TUI creates another independent Client View, just as opening another Browser tab does.                                             |
| Initial location      | Bare TUI defaults to the standalone terminal context and its first ordered Tab. Opening an empty context may create its first terminal exactly once through one Server-coordinated semantic operation; it does not show a startup terminal-picker modal.                                     |
| Empty state           | Closing the final Tab leaves an explicit empty context. It does not close the TUI, stop the Session, or terminate unrelated retained terminals.                                                                                                                                              |
| Attachment            | Only a visible active terminal needs a transient display attachment. Switching releases the old attachment and opens the new one; neither action creates, clones, restarts, or terminates the retained terminal. Product copy uses “open” or “connect”; `attachment` is an engineering term. |
| Controller            | Several attachments may view one terminal. One combined input-and-resize controller exists at a time. Attach acquires it only when unowned; otherwise the Client enters explicit view-only mode. Release never auto-promotes another viewer or steals control.                               |
| First-create geometry | The full-screen TUI obtains one complete actual cell and pixel geometry before submitting immediate Create. It never fabricates 80x24. Deferred Create plus Start remains available to other flows whose real first geometry is not yet known.                                               |

The initial TUI uses a full-screen alternate screen. Its wide layout has an Orchbee business
sidebar on the left and Workbench tabs, panel tree, and the real terminal on the right; narrow
layouts collapse the sidebar into an overlay. Ratatui plus Crossterm is the selected initial Rust
stack. The default prefix is `Ctrl+B`, with a visible prefix-mode indicator and this initial map:

| Binding                 | Action                                  |
| ----------------------- | --------------------------------------- |
| Prefix, then `c`        | Create a Tab                            |
| Prefix, then `n` / `p`  | Select next / previous Tab              |
| Prefix, then `1` … `9`  | Select the corresponding ordered Tab    |
| Prefix, then `h/j/k/l`  | Move between Panels                     |
| Prefix, then `b`        | Toggle the business sidebar             |
| Prefix, then `w`        | Open the Workbench navigator            |
| Prefix, then `?`        | Open shortcut help                      |
| Prefix, then `q` or `d` | Detach this TUI Client                  |
| Prefix, then `Ctrl+B`   | Send a literal `Ctrl+B` to the terminal |

Outside prefix mode, `Ctrl+C`, `Ctrl+D`, and ordinary terminal input pass through to the active
terminal. Orchbee owns its information architecture and visual design.

## Accepted CLI information architecture

Commands use explicit resource-first noun/verb vocabulary, with no fuzzy abbreviations and no
public aliases for private executable names. The accepted target command families are:

| Family            | Accepted commands                                                                                                                               | Delivery boundary                                                                                                |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Entry and tools   | bare `orchbee`, `tui`, `status`, `completion`, `config check`, `api schema`, `api snapshot`, `--help`, and `--version`                          | Entry, basic tools, schema discovery, and bounded read-only snapshots belong in the first public command cohort. |
| Project           | `project list`, `project get`                                                                                                                   | Read-only in the first cohort.                                                                                   |
| Workspace         | `workspace list`, `workspace get`, `workspace create`, `workspace focus`, `workspace rename`, `workspace close`                                 | `list` and `get` first; mutations follow only after their Server state machines are admitted.                    |
| Tab               | `tab list`, `tab get`, `tab create`, `tab focus`, `tab rename`, `tab close`                                                                     | `list` and `get` first; mutations follow only after their Server state machines are admitted.                    |
| Panel             | `panel list`, `panel get`, `panel focus`, `panel split`, `panel resize`, `panel move`, `panel close`                                            | `list` and `get` first; mutations follow only after their Server state machines are admitted.                    |
| Terminal          | `terminal list`, `terminal get`, `terminal read`, `terminal open`; later `terminal send-text`, `terminal send-keys`, and `terminal wait-output` | Read-only and `open` first; active input automation remains a later explicitly authorized slice.                 |
| Session lifecycle | `session enable`, `server stop`, and `session restart`                                                                                          | Later lifecycle slices; there is no public `orchbee server` command.                                             |
| Later domains     | Agent, managed Worktree, Archive, Launch Profile, and integration commands                                                                      | Added only by their owning accepted domain contracts.                                                            |

`Tab`, `Panel`, and `Terminal` remain distinct product nouns. There is no monolithic
`--no-session` mode and no compatibility grammar built around the old raw `session` term. Root help
guides Agents toward `api schema`, `api snapshot`, and stable CLI commands rather than restoring the
old `--skill` interface. Update commands, if admitted, remain aware of the installation manager and
never overwrite a Brew- or npm-managed installation behind that manager's back.

## Accepted automation, input, and mutation contract

- Localized human-readable text is the default. An explicitly selected machine mode emits stable,
  versioned JSON; streaming operations use NDJSON. The exact selection flag, schema identifiers,
  pagination bounds, and numeric exit codes remain implementation-slice decisions.
- Machine data belongs only on stdout. Localized diagnostics and progress belong on stderr. Stable
  exit categories distinguish usage, rejection, service unavailable, failed effect, cancellation,
  transport loss, incompatibility, and unknown outcome.
- CLI input from an argument, file, or stdin is explicitly selected and byte bounded. Secrets never
  appear in argv, human output, diagnostics, or shell completion.
- Every mutation carries a globally unique request identity and is submitted at most once. After an
  ambiguous result, the Client reconciles that identity and never retries the effect as a new request.
- Destructive operations require an explicit noninteractive confirmation contract. `--detach` is
  admitted only for operations with durable Server-owned identity and later observation; it never
  converts terminal input into a fire-and-forget effect.
- `completion` generates shell completion from the accepted grammar. Installed Skills are separate,
  versioned product assets owned by the feature that exposes them, not compatibility copies of the
  current embedded Skill.

## Accepted packaging boundary

One standard installation exposes only `bin/orchbee`. Compatible private Server, Worker, Terminal
Host, manifest, and—when Agent integration is included—Agent Bridge artifacts live in a private
versioned `libexec` cohort. Shell installer, Homebrew, and npm distribute the same signed and
digest-verified native cohort; npm uses platform-specific optional packages and does not require a
Node.js runtime to execute Orchbee. macOS and Linux are the first supported native release targets;
Windows remains later work.

The release layout uses side-by-side version directories and one atomic active pointer. A failed
install or migration rolls back the whole compatible cohort. Public cutover waits for the selected
vertical commands, legacy-consumer migration, packaged upgrade and rollback evidence, and measured
startup/size budgets; those budgets are derived from real artifacts rather than chosen in this
proposal.

## Problem

Users should install and remember one native `orchbee` command for interactive TUI and one-shot automation. The former Desktop-bundled TypeScript CLI and its grammar, locator, launcher, and packaging assumptions have retired; they are not compatibility requirements for the native Server/Worker topology.

## Scope

This proposal owns:

- the public `orchbee` invocation modes and command hierarchy;
- argument, input, output, diagnostic, and exit contracts;
- local Session discovery and explicitly admitted activation commands;
- the boundary between human text and versioned automation output;
- CLI/TUI process isolation inside one Rust artifact;
- compatibility, public cutover, and rollback of future native release cohorts; and
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
- authorize future public release cutover before the selected capabilities and rollback cohort are verified.

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

The accepted target fixes the product vocabulary above. Each implementation slice must still
materialize these dimensions explicitly before extending the parser:

| Dimension       | Required decision                                                                                                                                                           |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Modes           | Bare TUI, one-shot resource commands, Session lifecycle commands, help, version, completion, and any machine-only discovery mode                                            |
| Nouns and verbs | Stable product vocabulary, nesting, aliases, abbreviation policy, and how unsupported capabilities are reported                                                             |
| Targeting       | Local Session default, explicit remote Session or Worker selection, identity validation, ambiguity, and no-target behavior                                                  |
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
| Boundaries            | `apps/orchbee` owns only its process entry, invocation decision, locale projection, and development presentation. It has no runtime dependency beyond the Rust standard library and grants no Server, Worker, Host, filesystem, or process capability. The new binary is a Cargo verification input but not a release or packaging input.                                                                                                                                                                                                                         |
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

1. Inventory future installed consumers against the native cohort.
2. Select the first one-shot Rust command cohort and record its exact grammar, schemas, activation policy,
   identities, bounds, and excluded legacy capabilities.
3. Implement the Rust CLI core independently from TUI rendering and backend engines, then add TUI
   composition without changing CLI startup dependencies.
4. Prove only deliberately retained semantic behavior against its owner contract. Do not use parser
   parity as the acceptance oracle.
5. Package one compatible `orchbee`/Server/Worker/Host cohort, plus Agent Bridge when that
   capability is installed, and atomically switch the public entry.
   Before any incompatible durable migration, rollback restores the complete prior cohort.
6. Legacy source retirement proceeds under the separately accepted explicit sunset. Future release
   packaging and public-entry activation require their own evidence and do not block feature development.

No transition state exposes two authoritative public command surfaces under the same installation.
Legacy source retirement is complete independently of release packaging. Future packaging selects
exactly one native public entry and compatible cohort. The completed, unshipped Phase 3A foundation
provides no evidence of a packaged-entry cutover.

## Evidence plan

| Claim                                                                                        | Required evidence                                                                                                                                                                                                         |
| -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI startup does not initialize TUI or backend engines                                       | Dependency and process-start tests, cold/warm startup measurements, artifact-size and peak-memory measurements                                                                                                            |
| One invocation cannot gain Server, Worker, or Host authority                                 | Capability-negative tests, malformed-input tests, authentication tests, and direct-Host connection rejection                                                                                                              |
| Mutating commands submit at most once                                                        | Request-identity property tests, transport-loss traces, restart reconciliation, and unknown-outcome tests                                                                                                                 |
| Human output is not the automation contract                                                  | Versioned-schema conformance, stdout/stderr tests, redaction tests, and compatibility fixtures                                                                                                                            |
| Bare invocation and explicit CLI modes do not select ambiguous paths                         | Parser decision-table tests, help/version tests, TUI bootstrap tests, and unsupported-mode cases                                                                                                                          |
| Bare local activation creates a peer Client Session without owning service lifetime          | Absent/present/incompatible cohort process tests, simultaneous-TUI traces, and Client-exit retention tests                                                                                                                |
| TUI shares Workbench semantics without sharing another Client's view                         | Multi-TUI/Web trace, empty-context first-create test, Tab/Panel navigation tests, and attachment/controller fencing                                                                                                       |
| Piped one-shot commands never seize a terminal device or initialize the alternate screen     | TTY/non-TTY process matrix, `/dev/tty` negative test, exact stdout/stderr fixtures, and signal/terminal restoration tests                                                                                                 |
| Phase 3A has no hidden backend or TUI initialization and localizes every authored message    | Dependency metadata, exact process-output tests in both locales, and release-profile startup/size/memory baseline                                                                                                         |
| Cutover leaves one public entry and one compatible service cohort                            | Packaged installation, upgrade, rollback, partial-installation, and stale-private-executable tests                                                                                                                        |
| Legacy source retirement leaves no retained consumer bound to removed workspaces             | Consumer scan, retained-workspace dependency/build/test checks, native asset checks, and current-authority/documentation cleanup establish the explicitly accepted source retirement without requiring release packaging. |
| CLI Workbench snapshots expose product semantics without Web or another Client's local state | Server projection fixtures, schema bounds, authorization/redaction tests, and negative HTML/DOM/pixel/client-view cases                                                                                                   |

## Reserved decisions

1. Select the exact option spelling for machine output, input sources, pagination, confirmation,
   targeting, timeouts, and detached observation; define the versioned JSON/NDJSON schemas and
   numeric exit codes without changing the accepted semantic categories.
2. Select the exact first-release subset from the accepted read-only and `terminal open` families,
   then admit each later mutation only with its owner state machine and safety evidence.
3. Define remote Session targeting and authentication. Remote enrollment, production human/device
   identity, TLS, Internet reachability, multi-user tenancy, and mobile background behavior are not
   implied by the local command surface.
4. Define detailed TUI visual composition, accessibility projection, configurable binding policy,
   mouse behavior, and noninitial layout operations beyond the accepted full-screen structure and
   default keyboard map.
5. Define the installed Skill catalog and completion-installation mechanics independently from the
   old embedded-Skill contract; the accepted `completion` generator and Agent-facing schema
   discovery do not select those distribution details.
6. Derive supported-platform startup, artifact-size, and peak-memory budgets from measured release
   candidates; the Phase 3A reference baseline does not set an acceptance budget.
7. Authorize future packaged public cutover only with consumer, compatibility, signed-cohort upgrade, rollback, documentation, and installed-artifact evidence. Legacy source retirement is separately authorized and does not depend on that future release.
