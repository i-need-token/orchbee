# Windows Panel metadata reports

`panel report-metadata <panel> --source <id>` writes transient presentation metadata for the
Panel's current primary terminal. Select an exact Panel ID or `@panel:<suffix>`. Options are
`--title`, `--display-agent`, repeated `--state-label STATE=TEXT`, `--agent`, `--applies-to-source`,
`--clear-title`, `--clear-display-agent`, `--clear-state-labels`, repeated `--token KEY=VALUE` and
`--clear-token KEY`, `--seq`, `--ttl-ms`, and `--format text|json`. States are `idle`, `working`,
`blocked`, `done`, `unknown`. The [Server model](../../orchbee-server/docs/workbench-authority.md#panel-presentation-metadata)
owns binding lifetime, source ordering, guards and expiry.

The parser validates the entire report before connection. Source uses 1–80 ASCII letters,
digits, `:._-`; display text strips control characters, trims and retains at most 80 Unicode scalars.
An empty token value removes its key; empty optional presentation text is omitted. State labels
require nonempty normalized text. Repeated state/token options use the last value. At least one
presentation set/clear or token change is required. Setting and clearing the same field fails, including explicitly supplied empty text.
Pure presentation sets replace the source record; including any presentation clear changes to
selective patch semantics. Optional Agent/source guards filter display; they confer no authority.
An explicitly supplied `--agent` must be nonempty after trimming. Recognized report aliases and
executable paths normalize to the report label (`claude`, `agy`, `copilot`, `qwen`, for example);
unknown labels retain their trimmed case and content. The Agent guard is never truncated or
stripped of internal controls: values outside the current 80-scalar, control-free wire contract
fail before connection. This bound on custom labels remains a difference from Herdr's report
input and a dependency of complete Agent report support. Native process provider IDs keep their
separate recognition contract.

One existing-Site admission and snapshot share a cancellable five-second deadline. Resolve the
primary binding once and include it as `expectedBinding`; the Server rejects a changed binding.
An unbound Panel fails before submission. The command submits once and never starts a Site,
navigates a Client, mutates topology, changes OSC title or automatically retries. After handoff,
a late signal preserves the receipt and sets `interrupted`. A lost, malformed, mismatched or late
response preserves the request ID with an unknown outcome and exit 8. Before handoff, normal
CLI failures apply. A known applied or stale report exits 0; typed target/permission/capacity
refusals exit 3, unavailability 4, invalid request 5, revision/deadline/result bounds 1. Cleanup
failure exits 1 unless the submitted result is already unknown.

`orchbee.panel.metadata` v1 contains request/Site/Panel IDs, observation (`server` with the exact
reply, or `unknown` with a closed reason), `interrupted` and `cleanupFailed`. Catalog v89 includes
90 schemas. OBWP 33/34 carries `workbench.panel-metadata` and its result; request limit is 16 KiB,
result limit 512 KiB. Server reads use read permission and reports navigation permission under
the Site gate. Reads expose source records for up to four ordered unique Panels. Metadata and
Workbench revisions remain separate. Same-cohort deployment is required; old peers reject the
unknown carrier kind. There is no persistence migration; restarting the Site clears reports.

Protocol/owner tests cover atomic changes, replay and TTL; shared Rust/TypeScript/JSON corpus
checks bound and malformed inputs; named-pipe tests check exact correlation including binding,
response loss, deadline and no replay. Parser, receipt, localization and shell grammar checks
cover the CLI projection. TUI rendering and general events remain separate integration obligations; the query projection below consumes current Server reports. This command alone
does not establish complete Herdr Panel/Agent parity or Desktop support.

## Query observation model

Panel list/get/current and Agent Get consume Server metadata and explicit Agent reports through
one bounded query collector. Inputs are the admitted connection, original Snapshot, ordered Panel
IDs, original absolute deadline and cancellation signal. Both families read batches of four and
require their own stable revision across batches. Every row must match the original primary
binding and Workbench revision. All data reads finish before empty reads fence first metadata and
then Agent reports; their stable intervals therefore overlap. Neither family can silently change
during this join. A change rejects the query with `metadata-changed`, including report changes that
would change metadata guard context. The outer query still fences canonical and selected Client
state. No automatic retry or partial projection is published.

Each `metadata` object contains `source` (Site, Workbench and metadata revisions plus its read
request IDs), `binding`, `tokens`, raw source `presentations`, guard `context` (`agent`,
`authoritySource`) and computed `presentation` (title, displayAgent, stateLabels). Its sibling
`reportedAgent` contains its own `source` (Site, Workbench and report revisions plus report read
request IDs), the same logical `binding`, and `view` (Agent label, state, authority source, message
and optional external provider session reference). An unbound row has null binding/view; a bound
terminal with no report has an explicit unknown view. The metadata guard context uses the actual
Server report view. Missing facts remain null and never satisfy a guard. Generic observation
categories, user-assigned names and canonical provider IDs are never substituted for that context.
Native session-identity reporting alone does not fabricate process or report authority.

The collector is a read workflow, not another report owner. Server metadata and reports remain
available when a Host sample is unavailable. Agent Get uses a unique live primary binding; ended,
unbound, ambiguous or subsequently changed Agent membership discards both alongside the terminal
sample. Reports never replace independently observed OSC title, canonical label, status or
readiness. External provider references remain separate from canonical observation Session IDs
and are not executed by these queries. Automatic Host process production, associated names,
provider resume/persistence and TUI publication remain separate integration obligations.

Windows Panel list/get emit v9, current emits v6, and Agent Get emits v11 (v12 with `--client`).
Every Panel row and non-null Agent terminal requires metadata and reportedAgent. Earlier metadata
versions retain null guard context and reject reportedAgent; still older versions reject metadata
as well. Their closed previous shapes remain accepted. Localized text escapes effective metadata,
report details, messages and external references. Catalog v89 contains 90 schemas and 104 command
paths; this query change adds no command or schema identity.

Admission caps combined encoded metadata and report rows at the existing 8 MiB CLI output budget.
After metadata admission, reports consume only the remaining row budget; each batch is validated
before extending state. The final envelope remains independently bounded. Oversize or inconsistent
reads fail without partial output. Tests cover both revision fences, exact batches, wrong/missing
rows and bindings, permissions, transport loss, original shared deadline, closed legacy/new schema
shapes, guarded presentation, global tokens and escaped text. Matched Windows cohort acceptance
exercises the actual public reports and queries; it is not final TUI or authenticated provider proof.

The Windows TUI now consumes a separately revision-fenced report/metadata round for primary Panels in its active Tab. Custom titles retain precedence; effective metadata, literal tokens and localized Agent status paint in the existing single, split or zoomed header. See [Panel report presentation](windows-panel-report-presentation.md) for scope, failure and shutdown rules.
