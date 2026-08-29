# CLI Agent Control Surface

| Field             | Value                                                                                                                                                                                                                             |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | The existing TypeScript CLI direction and read-only Slices 1 through 4 are implemented; later terminal-control slices remain reserved                                                                                             |
| Decision owner    | The active requester acting as Orchbee product and architecture decision owner                                                                                                                                                    |
| Semantic owners   | `apps/cli`, Desktop application composition, `@orchbee/semantic-command-protocol`, `@orchbee/workbench`, and the Agent capability                                                                                                 |
| Current authority | [`apps/cli/README.md`](../../apps/cli/README.md), [semantic-command execution](../../apps/desktop/docs/semantic-command-execution.md), and [Agent runtime observation](../agent/agent-runtime-observation.md)                     |
| Reference         | Herdr revision `c2637dc182ddc5425108824d5ed15d24ce38c4e3` is a pinned interaction reference, not an Orchbee state or architecture authority                                                                                       |
| Exit condition    | Deliver each separately accepted slice, move maintained behavior and evidence into the owning application and package guides and tests, resolve or remove every remaining decision, then delete this proposal and its translation |

Simplified Chinese review translation:
[agent-control-surface.zh-CN.md](agent-control-surface.zh-CN.md). English remains canonical.

## Change authorization

| Field         | Accepted authorization                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tier          | Consequential because this is an Agent-facing process contract and will grow cross-workspace product operations; Modeled because Slices 2 and 4 introduce server-owned Agent waits and a retained change feed with subscription, timeout, cancellation, revision, overflow, and coordinator-lifetime transitions                                                                                           |
| Problem       | Orchbee already ships a secure local semantic-command CLI, but it exposes only generic operation and execution commands. An Agent must discover operation IDs and construct JSON manually, and there is no release-matched Skill that teaches the installed command surface or its safety boundaries                                                                                                       |
| Outcome       | Evolve the existing `apps/cli` TypeScript application into Orchbee's single nonvisual Agent control surface. Add owner-backed product nouns and a release-matched Skill incrementally while retaining server-owned facts, exact identities, structured output, explicit admission, and conservative unknown outcomes                                                                                       |
| Compatibility | Keep the existing `status`, `operation`, and `execution` commands, `ORCHBEE_CLI` launch contract, output version, exit meanings, target selection, input bounds, and no-replay behavior. A high-level command is an additional projection over an admitted semantic operation, never another execution path                                                                                                |
| Non-goals     | Rewriting the CLI in Zig, creating another CLI or daemon, reproducing Herdr's TUI or flat Agent status, reading screen text as semantic authority, connecting directly to Terminal Host, adding arbitrary shell/filesystem/RPC access, global installation, remote access, or admitting launch, prompt, input, close, focus, approval, or destructive operations without their own accepted owner contract |
| Recovery      | Remove a high-level command or Skill section without removing the generic operation surface. A lost mutation response continues to reconcile only the same request identity; it is never replayed. Unsupported or unavailable owner operations fail explicitly                                                                                                                                             |

The active requester first selected a Zig CLI, then explicitly superseded that choice on
2026-08-29: extend the existing CLI and keep its current TypeScript implementation. This proposal
records the later instruction as authoritative.

## Accepted architecture

| Alternative                                     | Consequence                                                                                                 | Decision |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | -------- |
| Replace `apps/cli` with a Zig executable        | Discards a tested client and packaging path without adding product authority                                | Rejected |
| Create a second Herdr-like CLI or state daemon  | Creates competing discovery, lifecycle, state, and authorization owners                                     | Rejected |
| Keep only generic `operation invoke`            | Preserves safety but forces every Agent and Skill to duplicate product schemas and invocation details       | Rejected |
| Extend the existing CLI with owner-backed nouns | Reuses the authenticated endpoint and ledger while making admitted capabilities discoverable and scriptable | Accepted |

```text
Agent + release-matched Skill
             |
      apps/cli command projection
             |
  semantic-command protocol and local transport
             |
 Desktop admitted catalog and execution ledger
             |
 Workbench / Agent / Terminal capability owners
```

- The CLI remains a stateless client. Desktop or a future Hub owns authoritative state, operation
  admission, execution receipts, authorization, and commit boundaries.
- Product nouns may construct and validate only the input and output contracts exported by their
  semantic owner. They never dispatch UI Actions or connect to an owner-private transport.
- Stable opaque IDs, not labels, order, focused UI state, process IDs, paths, or guessed topology,
  select a mutation target.
- Default output remains one versioned JSON document. The Skill treats installed CLI output and
  exact returned IDs as authority.
- `orchbee --skill` prints instructions bundled from the same source revision as the executable. It
  requires no coordinator connection and grants no additional operation authority.

## Herdr alignment boundary

| Herdr interaction idea                         | Orchbee decision                                                                                                                                               |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| One CLI and a version-matched Skill            | Adopt through the existing `apps/cli` and `orchbee --skill`                                                                                                    |
| JSON responses and exact returned identifiers  | Preserve the current versioned JSON envelope and opaque Workbench and Agent identities                                                                         |
| Server-owned live topology                     | Preserve Desktop or future Hub ownership; never reconstruct topology from renderer state                                                                       |
| Workspace, tab, pane, and agent command groups | Adapt as Project, Directory, Workspace, Tab, Terminal, and Agent nouns only when each owner publishes the required semantic read or command contract           |
| Agent detection and wait                       | Use the Agent-owned Session, Turn, Attention, Outcome, attachment, provenance, and freshness model; never copy Herdr's flat state or promote screen heuristics |
| Bounded event journal and subscriptions        | Adopt as a loss-aware canonical Agent change feed with signed cursors, explicit reset, and no cross-runtime continuity claim                                   |
| Pane output and input                          | Keep Terminal Host authority and require explicit bounded semantic contracts; do not expose its private protocol through the CLI                               |
| Prompt, key input, focus, close, and stop      | Reserve because these may submit work, answer a prompt, steal focus, terminate work, or have an unknown commit outcome                                         |

The reference scope is command organization, machine-readable output, exact targeting, contextual
safety, and release-matched Skill guidance. This proposal makes no full behavioral-parity claim.
Herdr tests may inspire scenarios, but Orchbee evidence refines Orchbee's own owner contracts.

## Delivered Slice 1: admitted Workbench facade and Skill

Slice 1 adds only these high-level commands over the five operations already present in the
production Desktop catalog:

```text
orchbee project list [--limit <n>] [--cursor <cursor>]
orchbee directory list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee workspace list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee terminal list (--standalone | --project <project-id> --workspace <workspace-id>) [--limit <n>] [--cursor <cursor>]
orchbee terminal create (--standalone | --project <project-id> --workspace <workspace-id>) [--directory <directory-id>] [--detach] [--request-id <request-id>]
orchbee --skill
```

All coordinator-backed commands retain `--target` and `--output`. Page limits remain within the
Workbench-owned bound. Workspace targeting requires both exact Project and Workspace identities.
`terminal create` creates only the owner-defined Terminal presentation and retains the existing
commit, reconciliation, retry, and no-focus semantics; it does not start or prompt an Agent.

The embedded Skill:

- activates only for explicit Orchbee inspection or control intent and requires the injected
  `ORCHBEE_CLI` path;
- starts with `"$ORCHBEE_CLI" --skill` as installed syntax authority;
- uses only exact IDs returned by JSON and never guesses a focused target;
- distinguishes read-only discovery, Terminal creation, generic operation access, execution
  observation, and business cancellation;
- states unavailable capabilities instead of substituting shell injection, private protocols, or
  screen inference; and
- requires explicit user intent before a mutation and never claims that execution cancellation
  reverses an already committed owner effect.

## Delivered Slice 2: canonical Agent reads and server-owned wait

On 2026-08-29, the requester delegated uninterrupted completion toward the already selected
Herdr-alignment direction. Acting conservatively within that authorization, Slice 2 delivers only
read-only Agent observation and no new provider, Terminal, or presentation authority:

```text
orchbee agent list [--all] [--limit <n>] [--cursor <cursor>]
orchbee agent get <session-id>
orchbee agent explain <session-id>
orchbee agent wait <session-id> [--until <condition>]... [--after-revision <revision>] [--timeout <milliseconds>] [--detach] [--request-id <request-id>]
```

- `@orchbee/agent` owns all four operation descriptors, strict input and output schemas, summary,
  detail, explanation, condition matching, and default wait conditions. The CLI accepts only an
  exact returned Session ID; provider names, presentation labels, focused state, paths, process
  identities, and screen content never resolve a target.
- `agent list` returns a revision-fenced, server-signed page of stable Session summaries, defaults
  to live Sessions, includes retained ended Sessions only with `--all`, and has a maximum page size
  of 100. `agent get` returns the exact retained canonical snapshot plus its Agent-owned semantic
  projection. Neither query returns prompts, transcripts, commands, tool content, filesystem paths,
  environment values, provider errors, or adapter-private correlation data.
- `agent explain` explains which canonical dimension selected the projected status, the declared
  capability ceiling, source, fidelity, freshness, generation and sequence, and the current
  navigate-only/content-free limits. It does not expose or recreate Herdr-style screen matcher
  evidence because screen detection is not Orchbee semantic authority.
- `agent wait` is one admitted asynchronous query owned by Desktop application composition over
  the canonical `AgentSnapshotSource`. It subscribes before its decisive recheck, pins one Session
  identity, and settles only from an exact snapshot, explicit timeout, cancellation, or coordinator
  retirement. It does not poll a renderer, hold a hidden terminal, or consume provider events
  directly.
- Wait conditions are the Agent semantic-status kinds plus `fresh`, `stale`, and `disconnected`.
  The default is `needs-attention`, all four outcome kinds, `idle`, or `ended`. Semantic-status
  conditions match only a fresh observation; degraded health is matched only when explicitly
  requested. `--after-revision` additionally requires a strictly newer runtime revision and rejects
  a baseline ahead of the current runtime instead of waiting ambiguously.
- Timeout is explicit canonical input, defaults to 600,000 milliseconds, and is bounded from 1 to
  3,600,000 milliseconds. Timeout is `agent-wait-timeout`, missing identity is
  `agent-session-not-found`, a future baseline is `agent-revision-ahead`, and execution cancellation
  is `agent-wait-cancelled`. Desktop or Hub restart changes coordinator generation, so an in-flight
  wait has an unknown outcome under the existing CLI contract; no continuity or replay is claimed.

Slice 2 adds no Agent mutation or durable state. Its asynchronous subscription, recheck, timeout,
cancellation, exact-identity, and coordinator-retirement boundary has a bounded TLA+ model in
addition to deterministic domain, Desktop operation, CLI parser, runner, and catalog-composition
tests.

## Delivered Slice 3: canonical Tab and Panel membership reads

Slice 3 adds two deterministic Workbench queries and their high-level CLI projections:

```text
orchbee tab list (--standalone | --project <project-id> --workspace <workspace-id>) [--limit <n>] [--cursor <cursor>]
orchbee panel list (--standalone | --project <project-id> --workspace <workspace-id>) --tab <tab-id> [--limit <n>] [--cursor <cursor>]
```

- `@orchbee/workbench` owns both strict version-1 operation contracts and derives both projections
  from one validated canonical shared-document revision. Desktop adds only catalog admission,
  authorization, signed pagination, and the existing bounded execution envelope; the CLI remains a
  descriptor-checked projection over those operations.
- `tab list` requires one exact standalone or Project-and-Workspace location. It preserves canonical
  Tab collection order and returns each stable Tab identity, terminal-context identity, group
  identity, pinned state, custom and fallback titles, Panel count, and Terminal-presentation count.
- `panel list` additionally requires one exact Tab identity from that location. It preserves the
  canonical depth-first, left-to-right Panel-leaf order and returns a one-based position, stable
  Panel, Tab, and terminal-context identities, and Terminal-presentation count. A missing exact Tab
  returns `tab-not-found`; no active Tab or focused Panel fallback exists.
- Both queries keep the Workbench page maximum of 200, bind every opaque cursor to the exact
  operation, target, coordinator generation, and shared revision, and return `page-cursor-stale`
  after that revision changes. The Panel cursor target includes the exact Tab identity.
- The output intentionally excludes active or selected Tabs, focused or maximized Panels, recent
  focus history, active child Terminal, split weights, pixel geometry, renderer state, screen
  content, process state, and inferred Agent state. Those are Client View or another semantic
  owner's facts, not canonical shared membership.

This slice adds no mutation, asynchronous owner transition, durable state, or new failure ordering.
Deterministic projection, schema, Desktop operation, HTTP-boundary, CLI parser, runner, catalog, and
Skill tests refine the contract; TLA+ is not selected because pagination reuses the already modeled
semantic-command ledger and existing revision-fenced cursor mechanism.

## Delivered Slice 4: loss-aware canonical Agent changes

The requester's 2026-08-29 authorization to begin Herdr-inspired peripheral observation adopts one
additional read-only command over the separately accepted Agent-owned volatile change journal:

```text
orchbee agent changes [--cursor <cursor>] [--limit <n>] [--timeout <milliseconds>] [--detach] [--request-id <request-id>]
```

- The first call without a cursor returns immediately with an empty `baseline` page and a signed
  cursor at the current runtime revision. A later call with that exact cursor returns up to 100
  strictly ordered canonical changes or long-polls when the cursor is current.
- Every item contains only its runtime revision, canonical observation-transition kind, and the
  current Agent-owned content-free Session summary. It contains no provider payload, prompt,
  transcript, command, question, tool content, path, environment value, terminal output, or screen
  matcher evidence.
- A timed-out long poll completes with a `current` page rather than fabricating an Agent change or
  failing the feed. Cancellation is `agent-changes-cancelled`. Invalid or cross-generation cursors
  are `agent-change-cursor-invalid`.
- When the caller is behind the latest 512 retained applied transitions, the operation returns a
  completed `reset-required` page with no items and a fresh cursor at the current revision. The
  Skill must obtain a new `agent list --all` snapshot before continuing; it must never silently
  bridge the gap.
- The application owns subscribe-before-recheck, timeout, cancellation, and coordinator-retirement
  behavior. The CLI remains stateless and emits one versioned JSON document per invocation; it does
  not keep a process-local event cache or bypass semantic-command admission.
- Timeout defaults to 30,000 milliseconds and is bounded from 1 to 600,000 milliseconds. The
  journal and cursor are volatile across Desktop or future Hub restart, and no durable replay,
  audit, or exactly-once delivery is claimed.

Slice 4 adds a bounded TLA+ model covering baseline, ordered batching, current waiting, retention
overflow, explicit reset, timeout, cancellation, and coordinator retirement, plus deterministic
Agent journal, Desktop operation, CLI parser, runner, catalog, HTTP-boundary, and Skill tests.

## Invariants and evidence

1. Every high-level command maps to exactly one admitted operation ID and one owner-valid bounded
   input; invocation proceeds only when the complete live public descriptor matches the bundled
   owner contract. Parsing failure connects to no coordinator and allocates no request identity.
2. High-level and generic invocation share the same request identity, describe-before-invoke,
   execution ledger, wait, cancellation, exit-code, and unknown-outcome implementation.
3. Adding a high-level CLI projection alone never adds an operation to Desktop's catalog or grants
   a caller authority; every new owner operation is separately declared, admitted, and tested.
4. `--skill` performs no discovery, connection, invocation, filesystem read, or product mutation at
   runtime.
5. Existing command arguments and output documents remain compatible.

Deterministic parser and runner tests cover every accepted shape, incompatible selector, exact
operation/input mapping, unavailable operation, lost response, output, and no-connect Skill path.
Desktop's real-endpoint test exercises Node, packaged Electron Node mode, and the exact launcher and
proves that Skill printing adds no endpoint request. Slice 1 adds no new asynchronous owner state and
therefore reuses the semantic-command ledger model. Slice 2 adds `AgentSemanticWait.tla`; domain and
Desktop operation tests refine its exact-identity, revision, freshness, subscribe-before-recheck,
timeout, cancellation, removal, and coordinator-retirement boundaries. Slice 3 adds deterministic
Workbench membership projection and exact-target cursor tests without a new state machine. Slice 4
adds `AgentSemanticChanges.tla`; domain and Desktop tests refine its bounded ordering, batching,
subscribe-before-recheck, timeout, cancellation, overflow/reset, signed cursor, and coordinator-
retirement boundaries.

## Delivery order

1. Delivered Slice 1: the Workbench facade and release-matched Skill do not change the production
   catalog.
2. Delivered Slice 2: read-only `agent list`, `agent get`, `agent wait`, and `agent explain` use the
   exact contract above.
3. Delivered Slice 3: read-only `tab list` and exact-target `panel list` use the canonical shared
   Workbench document without exposing Client View state.
4. Delivered Slice 4: read-only `agent changes` follows the volatile, loss-aware canonical
   Agent journal without claiming restart continuity.
5. Accept topology mutations such as Workspace, Tab, Panel, and Terminal creation, split, move,
   rename, focus, and close one owner operation at a time.
6. Accept Agent launch, prompt, resume, decision, raw terminal input/output, and orchestration only
   with exact provider and Terminal authority, confirmation, audit, cancellation, replay, and
   unknown-outcome contracts.
7. Move delivered behavior and evidence into current owner guides and delete resolved proposal
   sections; delete the complete proposal when no future decision remains.

## Reserved decisions

- any Agent CLI continuity beyond the selected volatile Desktop runtime and existing coordinator-
  generation failure boundary, including durable wait replay or retained state across Desktop or
  Hub restart;
- caller-context environment variables and their exact Project, Workspace, Tab, Terminal,
  presentation-attachment, and remote topology meanings;
- creation, split, move, focus, close, stop, and destructive lifecycle commands;
- Terminal output snapshots, history bounds, sensitive-content policy, input, paste, logical keys,
  and output waiting;
- Agent launch, naming, provider arguments, prompt/turn identity, submission, approval, answer,
  resume, interruption, orchestration, and external-cost policy;
- global installation, shell completions, third-party API stability, remote/SSH/Browser invocation,
  multiple users, audit retention, and durable execution recovery; and
- any full Herdr parity claim beyond the pinned interaction ideas explicitly selected above.

Implementation stops at the earliest slice affected by one of these decisions until its owner
accepts the exact contract, failure behavior, evidence limits, rollout, and recovery.
