# Windows Panel Agent reports

## Exact member targets

Each command accepts optional `--presentation <id>`, a full nonzero 32-character lowercase
hexadecimal Presentation ID belonging to the positional Panel. Omission retains the primary
target. Explicit selection never falls back to a different member, caller or Client. The CLI
resolves canonical membership and requires a bound member before submission. The Server rechecks
the exact owning Panel and expected binding under its existing Site gate. Reports remain owned
by the binding and physical generation; source ordering and report policy are unchanged.

Exact writes use the closed `report-presentation` / `presentation-reported` variants of the
existing carrier. Positive replies must match request, Site, Panel, Presentation and binding;
legacy `report` / `reported` remain primary-only. The CLI emits `orchbee.panel.agent-report` v4
only for explicit member selection, including the requested `presentationId` for known and
unknown submitted outcomes. Earlier v1/v2 remain closed; primary reports use v3. Localized text identifies the member.
All phases retain the single five-second deadline, cancellation and no-replay rules below.

Targeted contract and owner traces cover child/primary isolation, wrong owners, unbound members,
binding changes and source ordering; CLI traces cover exact requests and correlated or unknown
receipts. Matched private cohorts roll out together, with source rollback and no data migration.
Full provider, multi-Client and visual acceptance remains part of the final gate.

## Commands

Four commands submit explicit terminal reports to the running Site:

```text
orchbee panel report-agent <panel> --source <id> --agent <label> --state idle|working|blocked|done|error|unknown [--message <text>] [--seq <n>] [--agent-session-id <id>] [--agent-session-path <path>]
orchbee panel report-agent-session <panel> --source <id> --agent <label> [--seq <n>] [--agent-session-id <id>] [--agent-session-path <path>] [--session-start-source <reason>]
orchbee panel clear-agent-authority <panel> [--source <id>] [--seq <n>]
orchbee panel release-agent <panel> --source <id> --agent <label> [--seq <n>]
```

All accept `--format text|json`, a full Panel ID or `@panel:<suffix>`, and the Windows global
`--site` selector. Help and PowerShell completion expose their command-specific options and state
values. `clear-agent-authority` provides a native command for Herdr's API-only operation.

## Input and report ownership

Report/release sources are trimmed and must be nonempty, matching the reference CLI; unlike
metadata sources they allow arbitrary internal characters. Clear preserves its optional source
literally: omitted means the current authority, while an empty string matches only the empty source.
The binary API itself preserves literal sources. Agent labels use the shared report normalizer:
recognized aliases become canonical report labels, unknown labels retain trimmed case and content,
and explicitly empty labels fail. Messages are literal. No display-length truncation applies.
Sequence input parses a full unsigned 64-bit integer and becomes a canonical decimal string on the
wire. Duplicate options, unsupported options, invalid states, and complete requests above 16 KiB
fail before any connection; JSON escape expansion counts toward that limit.

Provider IDs, paths and start reasons are passed to the [Server report owner](../../orchbee-server/docs/agent-report-model.md).
Invalid references or unknown start reasons normalize to absent rather than creating invented
sessions. Only official source/provider pairs retain resumable identity. Native official state
reports route to session identity; full lifecycle reports need actual matching process evidence.
A custom report is explicit authority, not proof of process existence or a canonical Agent session.
Clear affects the current matching authority; release preserves independent live processes and
foreign provider references. Official releases intentionally do nothing. A stale or unsequenced
report after that source's sequence watermark is ignored. State changes and reference acceptance
can have different outcomes, so the returned disposition and `changed` flag are both meaningful.

## Submission and receipt

One existing-Site connection, snapshot and submission share a five-second deadline and signal
owner. Resolve the requested member once (primary by default) and submit the expected binding. Unbound targets fail
before submission; changed bindings receive a correlated rejection. No Site starts, focus changes,
terminal input, provider execution or automatic retries occur. After handoff, a lost, malformed,
mismatched or late response preserves the request ID and reports an unknown outcome. A late
interrupt preserves a received result and sets `interrupted`.

The original `orchbee.panel.agent-report` v1 introduced request/Site/Panel IDs, a Server reply or closed unknown
reason, `interrupted` and `cleanupFailed`. The reply contains independent Workbench/report revisions,
binding, applied/ignored/deferred disposition, actual change flag and effective report view including
an optional external provider session reference. Applied, ignored and deferred exit 0; target,
permission and capacity refusals exit 3; unavailable exits 4; invalid request exits 5; owner
revision/clock/result bounds exit 1. Unknown after submission exits 8. Cleanup failure exits 1 unless
the submitted outcome is already unknown. Localized text reports disposition and request identity;
JSON carries the complete view without changing its strings.

The initial catalog v90 published 90 schemas with 104 native command paths. OBWP 35/36 uses
the existing 16 KiB request and 512 KiB result limits. Deploy a matched private cohort; older peers
reject these carrier kinds. State is transient and no persistence migration is involved.

## Remaining integration and verification

The native commands consume the real Server owner. Report-associated names,
provider resume/persistence remain separate integration obligations. The Windows TUI consumes
reports and guarded metadata through [Panel report presentation](windows-panel-report-presentation.md).
In particular, a deferred lifecycle report does not
become a confirmed process merely because its CLI receipt succeeded. Panel list/get/current and Agent Get now expose source-attributed `reportedAgent` views and
apply metadata guards using their actual context. Both revisions are fenced together; see the
[metadata projection](windows-panel-metadata.md#query-observation-model).

Parser tests cover scope, source distinctions, label/reference preservation and complete encoded
request bounds. Named-pipe fault tests cover pre-submission deadlines, response correlation,
binding mismatch, loss, malformed replies, late replies and no replay. Receipt tests preserve all
three dispositions through late cancellation, both locales and the closed JSON schema. Windows
acceptance uses a matched cohort and isolated Site through these public command paths; it does not
claim an authenticated provider task or final TUI/full-regression acceptance.

## Process confirmation through Host reads

On Windows, the Site samples Host processes in the background without a user query. Each round visits at most eight primary/child bindings in round-robin order, with a 250 ms wait between rounds and a one-second between-target budget; carrier I/O and cleanup can delay a round, so this is not a freshness guarantee. One temporary read-only viewer is released before sampling another terminal; uncertain cleanup pauses sampling and remains owned through Site shutdown. Successful `panel process-info` and the terminal observations used by Panel/Agent queries also feed the same trusted process adapter. It rechecks the exact Worker/Host generation, recognizes a unique process chain, and can activate matching deferred lifecycle reports. A later complete sample can prove departure of the retained PID/creation identity and clear its reports plus applicable metadata atomically. Missing/failed/ambiguous samples do not invent exits; unchanged identities do not churn revisions. Panel/Agent queries collect Host observations before reading report/metadata revisions, so their own admitted confirmation can appear in the same result. A later concurrent revision change returns the existing `metadata-changed` failure, never a mixed result. The periodic sampler also publishes correlated live screen fallback under the [screen detection model](../../orchbee-server/docs/agent-screen-model.md); process recognition does not authenticate a provider or create canonical Agent Sessions.
