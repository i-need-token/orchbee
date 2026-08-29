# CLI Agent Control Surface

| Field             | Value                                                                                                                                                                                                                             |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | The existing TypeScript CLI direction and Slice 1 are implemented; later Agent and terminal-control slices remain reserved                                                                                                        |
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
| Tier          | Consequential because this is an Agent-facing process contract and will grow cross-workspace product operations; Modeled when a later slice introduces Agent waits, input, launch, cancellation, or another owned asynchronous transition                                                                                                                                                                  |
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

## Invariants and evidence

1. Every high-level command maps to exactly one admitted operation ID and one owner-valid bounded
   input; invocation proceeds only when the complete live public descriptor matches the bundled
   owner contract. Parsing failure connects to no coordinator and allocates no request identity.
2. High-level and generic invocation share the same request identity, describe-before-invoke,
   execution ledger, wait, cancellation, exit-code, and unknown-outcome implementation.
3. Adding a command never adds an operation to Desktop's catalog or grants a caller authority.
4. `--skill` performs no discovery, connection, invocation, filesystem read, or product mutation at
   runtime.
5. Existing command arguments and output documents remain compatible.

Deterministic parser and runner tests cover every accepted shape, incompatible selector, exact
operation/input mapping, unavailable operation, lost response, output, and no-connect Skill path.
Desktop's real-endpoint test exercises Node, packaged Electron Node mode, and the exact launcher and
proves that Skill printing adds no endpoint request. Slice 1 adds no new asynchronous owner state
and therefore reuses the existing semantic-command ledger model rather than adding a second TLA+
model.

## Delivery order

1. Delivered Slice 1: the Workbench facade and release-matched Skill do not change the production
   catalog.
2. Accept and deliver read-only `agent list`, `agent get`, `agent wait`, and `agent explain` from the
   canonical Agent read model, including subscription or polling semantics, freshness, visibility,
   privacy, compatibility, and application-runtime ownership.
3. Accept topology mutations such as Workspace, Tab, Panel, and Terminal creation, split, move,
   rename, focus, and close one owner operation at a time.
4. Accept Agent launch, prompt, resume, decision, raw terminal input/output, and orchestration only
   with exact provider and Terminal authority, confirmation, audit, cancellation, replay, and
   unknown-outcome contracts.
5. Move delivered behavior and evidence into current owner guides and delete resolved proposal
   sections; delete the complete proposal when no future decision remains.

## Reserved decisions

- the Agent CLI read schema, runtime endpoint, wait/event protocol, retention, and continuity across
  Desktop or Hub restart;
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
