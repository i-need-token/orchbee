# Unified Search Remaining Integrations

| Field             | Value                                                                                                                                      |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Status            | Accepted; the search surface and bounded asynchronous provider coordination are implemented; Agent and Terminal integrations remain future |
| Owner             | `@orchbee/workbench`, Agent capability, Terminal capability, and application composition maintainers                                       |
| Current authority | [`packages/workbench/docs/unified-search.md`](../../packages/workbench/docs/unified-search.md)                                             |
| Exit condition    | Integrate the remaining owned providers, update current authorities and tests, then delete this proposal and its translation               |

Simplified Chinese review translation:
[unified-search-surface.zh-CN.md](unified-search-surface.zh-CN.md). English remains
canonical.

## Current foundation

Workbench already owns one search surface. The sidebar, `Mod+K`, and `Mod+Shift+P` open that same
surface with typed filter inputs. It currently provides local Actions and Settings results and a
bounded asynchronous provider coordinator with cancellation, retry, stale-completion rejection,
deterministic ranking, accessibility projection, tests, and TLA+ evidence.

This proposal does not redefine those implemented facts. It only owns the remaining integrations
below.

## Agent provider

The Agent capability owner supplies the searchable read model and navigation Action. Workbench may
project recent and matching Agents through the existing provider contract but does not own Agent
session, composer, submission, or remote-job lifecycle.

- `Agents` shows owner-provided recent and matching results.
- Selecting a concrete Agent invokes an owner-provided prepared navigation Action.
- When no concrete result matches, an explicitly reviewed fallback may prefill an Agent-owned
  composer with the query.
- Prefill never submits, starts work, starts a process, or incurs external cost automatically.
- Provider cancellation and stale-result rejection continue to use the current Workbench
  coordinator.
- Query and result content remain mounted-memory data and are not persisted or sent to telemetry by
  Workbench.

The Agent owner must define stable result identity, availability, navigation meaning, localized
presentation, and the prefill boundary before composition.

## Terminal actions

Application composition may adapt Actions from the exact active Terminal instance into the existing
search provider and Action Runtime contracts.

- A result remains bound to the instance registration that produced its prepared invocation.
- Disposal or active-instance replacement makes the old result unavailable; it never retargets a
  different terminal.
- `@orchbee/terminal` retains terminal-specific action semantics and does not depend on Workbench
  presentation or localization.
- Workbench does not acquire Terminal process, session, attachment, or projection lifecycle.
- Application composition maps compatible definitions and outcomes without creating another search
  path.

## Exclusions

This proposal does not add file search, persistent query or action history, user-defined keybindings,
action chaining, opaque or AI-generated ranking, Workbench-owned Agent or Terminal runtime state, or
automatic remote exposure of local Actions.

Workspaces, Automations, Skills, and other feature groups require their own stable searchable read
models and real usage before admission. Their possible future presence is not an accepted filter or
delivery commitment here.

## Completion

1. Compose the Agent-owned provider and no-auto-submit fallback after its public contracts are
   accepted.
2. Compose active Terminal-instance Actions at the application boundary after that adapter contract
   is accepted.
3. Remove any duplicate callbacks or metadata exposed by those integrations.
4. Move their maintained behavior and evidence into the owning current guides and tests.
5. Delete this proposal and its translation.
