# Windows Panel Agent reports

Four commands submit explicit terminal reports to the running Site:

```text
orchbee panel report-agent <panel> --source <id> --agent <label> --state idle|working|blocked|unknown [--message <text>] [--seq <n>] [--agent-session-id <id>] [--agent-session-path <path>]
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
owner. Resolve the primary terminal once and submit the expected binding. Unbound targets fail
before submission; changed bindings receive a correlated rejection. No Site starts, focus changes,
terminal input, provider execution or automatic retries occur. After handoff, a lost, malformed,
mismatched or late response preserves the request ID and reports an unknown outcome. A late
interrupt preserves a received result and sets `interrupted`.

`orchbee.panel.agent-report` v1 contains request/Site/Panel IDs, a Server reply or closed unknown
reason, `interrupted` and `cleanupFailed`. The reply contains independent Workbench/report revisions,
binding, applied/ignored/deferred disposition, actual change flag and effective report view including
an optional external provider session reference. Applied, ignored and deferred exit 0; target,
permission and capacity refusals exit 3; unavailable exits 4; invalid request exits 5; owner
revision/clock/result bounds exit 1. Unknown after submission exits 8. Cleanup failure exits 1 unless
the submitted outcome is already unknown. Localized text reports disposition and request identity;
JSON carries the complete view without changing its strings.

Catalog v88 publishes 90 schemas and the native grammar has 104 command paths. OBWP 35/36 uses
the existing 16 KiB request and 512 KiB result limits. Deploy a matched private cohort; older peers
reject these carrier kinds. State is transient and no persistence migration is involved.

## Remaining integration and verification

The native commands consume the real Server owner. Automatic Host process production,
report-associated names, provider resume/persistence and TUI display remain separate integration obligations. In particular, a deferred lifecycle report does not
become a confirmed process merely because its CLI receipt succeeded. Panel list/get/current and Agent Get now expose source-attributed `reportedAgent` views and
apply metadata guards using their actual context. Both revisions are fenced together; see the
[metadata projection](windows-panel-metadata.md#query-observation-model).

Parser tests cover scope, source distinctions, label/reference preservation and complete encoded
request bounds. Named-pipe fault tests cover pre-submission deadlines, response correlation,
binding mismatch, loss, malformed replies, late replies and no replay. Receipt tests preserve all
three dispositions through late cancellation, both locales and the closed JSON schema. Windows
acceptance uses a matched cohort and isolated Site through these public command paths; it does not
claim an authenticated provider task or final TUI/full-regression acceptance.
