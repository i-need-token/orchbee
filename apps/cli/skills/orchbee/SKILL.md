---
name: orchbee
description: Inspect or control the current Orchbee application through its injected CLI. Use when the user explicitly asks to inspect Orchbee Projects, Directories, Workspaces, Tabs, Panels, Terminals, or observed Agent Sessions; wait for an Agent state; create an Orchbee Terminal; or invoke an admitted Orchbee operation. Requires ORCHBEE_CLI.
---

# Orchbee

Orchbee exposes server-owned product state through the exact CLI path injected into a
Desktop-created Terminal. The installed CLI and its structured responses are authoritative.

Before using Orchbee, verify the caller context in the current shell:

```sh
test -n "${ORCHBEE_CLI:-}"
```

```powershell
if (-not $env:ORCHBEE_CLI) { throw 'Orchbee CLI context is unavailable.' }
```

If the check fails, say that this shell has no Orchbee CLI context and stop. Do not search `PATH`,
guess an application runtime directory, inspect private locator files, or connect to Terminal Host.
If these instructions were copied instead of printed by the current binary, refresh them with
`"$ORCHBEE_CLI" --skill`; in PowerShell use `& $env:ORCHBEE_CLI --skill`. The examples below use
POSIX syntax; use the same PowerShell call operator for each command on Windows.

## Discover product state

These commands are read-only:

```sh
"$ORCHBEE_CLI" status
"$ORCHBEE_CLI" project list
"$ORCHBEE_CLI" directory list --project <project-id>
"$ORCHBEE_CLI" workspace list --project <project-id>
"$ORCHBEE_CLI" tab list --standalone
"$ORCHBEE_CLI" tab list --project <project-id> --workspace <workspace-id>
"$ORCHBEE_CLI" panel list --standalone --tab <tab-id>
"$ORCHBEE_CLI" panel list --project <project-id> --workspace <workspace-id> --tab <tab-id>
"$ORCHBEE_CLI" terminal list --standalone
"$ORCHBEE_CLI" terminal list --project <project-id> --workspace <workspace-id>
"$ORCHBEE_CLI" agent list
"$ORCHBEE_CLI" agent get <session-id>
"$ORCHBEE_CLI" agent explain <session-id>
```

Use `--limit <1-200>` for Workbench lists and `--limit <1-100>` for Agent lists, then pass the
returned `nextCursor` to continue the same query. `agent list` returns live Sessions by default;
`--all` also includes retained ended Sessions. Use `--target <endpoint-id>` only when discovery
reports multiple coordinators and an exact trusted endpoint ID is already known.

`tab list` returns canonical shared Tab membership for one exact location. Use only a returned
`tabId` with `panel list`; that command returns canonical depth-first, left-to-right Panel leaves for
the same location and Tab. Panel `position` is descriptive for that revision, not a durable target.
Neither command reports the active or selected Tab, focused or maximized Panel, active child,
split weights, pixel geometry, renderer state, or screen content.

Default output is one compact, versioned JSON document. For a completed high-level command, the
owner result is under `.result.execution.result.output`. Read IDs from that output; never derive
them from names, list order, paths, examples, or another client's focused UI. `--output text` is
pretty JSON for humans and is not a stable automation format.

## Observe an Agent

Select only an exact `sessionId` returned by `agent list` or another canonical Agent result. Never
resolve a Session from a provider name, presentation label, path, process identity, screen text,
list position, or focused UI. Inspect one retained Session or its status basis with:

```sh
"$ORCHBEE_CLI" agent get <session-id>
"$ORCHBEE_CLI" agent explain <session-id>
```

`agent get` returns the canonical bounded snapshot and Agent-owned status projection. `agent
explain` reports the source, fidelity, freshness, generation, sequence, capability ceiling, and
canonical status basis. Neither command returns prompts, transcripts, commands, tool content,
filesystem paths, environment values, provider errors, or screen-matcher evidence.

Wait for a canonical condition on that same Session with:

```sh
"$ORCHBEE_CLI" agent wait <session-id>
"$ORCHBEE_CLI" agent wait <session-id> --until needs-attention --until failed
"$ORCHBEE_CLI" agent wait <session-id> --after-revision <revision> --timeout <milliseconds>
```

The default conditions are `needs-attention`, `failed`, `succeeded`, `canceled`, `interrupted`,
`idle`, and `ended`. Explicit conditions may also include `working`, `unknown`, `fresh`, `stale`, or
`disconnected`. Semantic-status conditions match only while the observation is fresh; request a
health condition explicitly when degraded state is the result you need. `--after-revision` requires
a strictly newer global Agent runtime revision. The timeout defaults to 600,000 milliseconds and is
bounded from 1 to 3,600,000.

This wait is a server-owned semantic execution. `SIGINT` or `SIGTERM` stops only this CLI process's
local observation and does not cancel the waiter or the Agent. If later cancellation may be needed,
start with `--detach`, retain the returned request or execution ID, and use `execution cancel` on
that exact ID. Cancelling the wait still does not cancel Agent work. A coordinator restart makes an
in-flight wait outcome unknown; report that uncertainty and do not replay it.

## Create a Terminal

Terminal creation is a product mutation. Use it only when the user requested a new Terminal or a
workflow explicitly requires one within the authorized task:

```sh
"$ORCHBEE_CLI" terminal create --standalone
"$ORCHBEE_CLI" terminal create \
  --project <project-id> \
  --workspace <workspace-id> \
  [--directory <directory-id>]
```

Creation returns exact `tabId`, `panelId`, `presentationId`, and `terminalContextId` values. It does
not start an Agent, submit a prompt, send terminal input, or grant access to Terminal Host.

Use `--detach` only when the caller will retain and observe the returned request or execution
identity. A lost or unknown mutation outcome must not be retried with a new request identity.

## Use the generic operation surface

Prefer the high-level commands above. When the user requests an admitted capability that has no
high-level command, inspect the live catalog before invoking it:

```sh
"$ORCHBEE_CLI" operation list
"$ORCHBEE_CLI" operation describe <operation-id>
"$ORCHBEE_CLI" operation invoke <operation-id> --input-file <json-file|->
```

Use only an exact returned operation ID and input matching its owner contract. Catalog presence does
not override user intent, authorization, confirmation, or task scope. Never substitute an arbitrary
shell, filesystem, renderer Action, private socket, or Terminal Host request.

## Observe an execution

```sh
"$ORCHBEE_CLI" execution get <request-or-execution-id>
"$ORCHBEE_CLI" execution wait <request-or-execution-id>
"$ORCHBEE_CLI" execution cancel <request-or-execution-id>
```

`execution cancel` requests cooperative cancellation of an active uncommitted semantic execution.
It does not stop a Terminal process, cancel Agent work, or reverse an owner effect that already
committed. If the CLI reports `outcome-unknown-after-restart` or an unavailable receipt, report the
uncertainty and do not replay the mutation.

## Current limits

The installed command surface does not yet provide Agent prompt, launch, approve, answer, resume,
interrupt, stop, orchestration, or key input; Terminal output reading or input; Tab or Panel split,
move, focus, or close; or remote control. Agent observation remains volatile across Desktop restart.
Do not simulate missing capabilities through screen inference or private protocols.
