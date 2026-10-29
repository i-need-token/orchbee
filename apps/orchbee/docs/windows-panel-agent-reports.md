# Windows Panel Agent reports

## Exact member targets

Each command accepts optional `--presentation <id>`, a full nonzero 32-character lowercase
hexadecimal Presentation ID belonging to the positional Panel. Omission retains the primary
target. Explicit selection never falls back to a different member, caller or Client. The CLI
resolves canonical membership and requires a bound member before submission. The Server rechecks
the exact owning Panel and expected binding under its existing Session gate. Reports remain owned
by the binding and physical generation; source ordering and report policy are unchanged.

Exact writes use the closed `report-presentation` / `presentation-reported` variants of the
existing carrier. Positive replies must match request, Session, Panel, Presentation and binding;
legacy `report` / `reported` remain primary-only. The CLI always emits `orchbee.panel.agent-report` version 1 with `presentationId`: the requested ID for explicit member selection, or null for the default primary. Known and unknown submitted outcomes retain this member. Localized text identifies the member.
All phases retain the single five-second deadline, cancellation and no-replay rules below.

Targeted contract and owner traces cover child/primary isolation, wrong owners, unbound members,
binding changes and source ordering; CLI traces cover exact requests and correlated or unknown
receipts. Matched private cohorts roll out together, with source rollback and no data migration.
Full provider, multi-Client and visual acceptance remains part of the final gate.

## Commands

Three commands submit explicit terminal reports to the running Session:

```text
orchbee panel report-agent <panel> --source <id> --agent <label> --state idle|working|blocked|done|error|unknown [--message <text>] [--seq <n>]
orchbee panel clear-agent-authority <panel> [--source <id>] [--seq <n>]
orchbee panel release-agent <panel> --source <id> --agent <label> [--seq <n>]
```

All accept `--format text|json`, a full Panel ID or `@panel:0x<hex>`, and the Windows global
`--session` selector. Help and PowerShell completion expose their command-specific options and state
values. `clear-agent-authority` clears matching explicit report authority.

## Input and report ownership

Report/release sources are trimmed and must be nonempty; unlike
metadata sources they allow arbitrary internal characters. Clear preserves its optional source
literally: omitted means the current authority, while an empty string matches only the empty source.
The binary API itself preserves literal sources. Agent labels use the shared report normalizer:
recognized aliases become canonical report labels, unknown labels retain trimmed case and content,
and explicitly empty labels fail. Messages are literal. No display-length truncation applies.
Sequence input parses a full unsigned 64-bit integer and becomes a canonical decimal string on the
wire. Duplicate options, unsupported options, invalid states, and complete requests above 16 KiB
fail before any connection; JSON escape expansion counts toward that limit.

Every source is custom, with complete rules owned by the [Server report owner](../../orchbee-server/docs/agent-report-model.md). A custom report is explicit authority; it does not prove process existence or create a canonical Agent session. The protocol admits only state, clear and release reports; unknown report kinds or fields are rejected before owner admission.

Clear affects the current matching authority; release preserves an independently observed live process. Once a source has a sequence watermark, stale or unsequenced state changes are ignored. An admitted report that leaves the view unchanged may still be applied, so disposition and `changed` separately describe admission and actual change.

## Submission and receipt

One existing-Session connection, snapshot and submission share a five-second deadline and signal
owner. Resolve the requested member once (primary by default) and submit the expected binding. Unbound targets fail
before submission; changed bindings receive a correlated rejection. No Session starts, focus changes,
terminal input, provider execution or automatic retries occur. After handoff, a lost, malformed,
mismatched or late response preserves the request ID and reports an unknown outcome. A late
interrupt preserves a received result and sets `interrupted`.

The `orchbee.panel.agent-report` v1 carries request/Session/Panel IDs, a Server reply or closed unknown
reason, `interrupted` and `cleanupFailed`. The reply contains independent Workbench/report revisions,
binding, applied/ignored disposition, actual change flag and effective report view. Applied and ignored exit 0; target,
permission and capacity refusals exit 3; unavailable exits 4; invalid request exits 5; owner
revision/clock/result bounds exit 1. Unknown after submission exits 8. Cleanup failure exits 1 unless
the submitted outcome is already unknown. Localized text reports disposition and request identity;
JSON carries the complete view without changing its strings.

The catalog exposes the current documents and command paths. OBWP 35/36 uses
the existing 16 KiB request and 512 KiB result limits. Deploy a matched private cohort; older peers
reject these carrier kinds. State is transient and no persistence migration is involved.

## Remaining integration and verification

The native commands consume the real Server owner. The Windows TUI consumes reports and guarded metadata through [Panel report presentation](windows-panel-report-presentation.md). Panel list/get/current and Agent Get now expose source-attributed `reportedAgent` views and
apply metadata guards using their actual context. Both revisions are fenced together; see the
[metadata projection](windows-panel-metadata.md#query-observation-model).

Parser tests cover scope, source distinctions, label preservation and complete encoded
request bounds. Named-pipe fault tests cover pre-submission deadlines, response correlation,
binding mismatch, loss, malformed replies, late replies and no replay. Receipt tests preserve all
two dispositions through late cancellation, both locales and the closed JSON schema. Windows
acceptance uses a matched cohort and isolated Session through these public command paths; it does not
claim an authenticated provider task or final TUI/full-regression acceptance.

## Process confirmation through Host reads

On Windows, the Session samples Host processes in the background without a user query. Each round visits at most eight primary/child bindings in round-robin order, with a 250 ms wait between rounds and a one-second between-target budget; carrier I/O and cleanup can delay a round, so this is not a freshness guarantee. One temporary read-only viewer is released before sampling another terminal; uncertain cleanup pauses sampling and remains owned through Session shutdown. Successful `panel process-info` and the terminal observations used by Panel/Agent queries also feed the same trusted process adapter. It rechecks the exact Worker/Host generation and feeds the Server [process identity state](../../orchbee-server/docs/agent-canonical-model.md#process-identity-state): a provider appears only after its anchor settles across two complete samples at least 500 ms apart, and a later complete sample without the anchor's PID/creation identity or a same-provider hand-off clears its reports plus applicable metadata atomically. Missing/failed/ambiguous samples and opaque rows do not invent exits; unchanged identities do not churn revisions. Panel/Agent queries collect Host observations before reading report/metadata revisions, so their own admitted confirmation can appear in the same result. A later concurrent revision change returns the existing `metadata-changed` failure, never a mixed result. The periodic sampler also publishes correlated live screen fallback under the [screen detection model](../../orchbee-server/docs/agent-screen-model.md); process recognition does not authenticate a provider or create canonical Agent Sessions.
