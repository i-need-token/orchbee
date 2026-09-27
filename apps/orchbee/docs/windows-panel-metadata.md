# Windows Panel metadata reports

`panel report-metadata <panel> --source <id>` writes transient presentation metadata for the
Panel's current primary terminal. Select an exact Panel ID or `@panel:<suffix>`. Options are
`--title`, `--display-agent`, repeated `--state-label STATE=TEXT`, `--agent`, `--applies-to-source`,
`--clear-title`, `--clear-display-agent`, `--clear-state-labels`, repeated `--token KEY=VALUE` and
`--clear-token KEY`, `--seq`, `--ttl-ms`, and `--format text|json`. States are `idle`, `working`,
`blocked`, `done`, `unknown`. The [Server model](../../orchbee-server/docs/workbench-authority.md#panel-presentation-metadata)
owns binding lifetime, source ordering, guards and expiry.

The parser validates the entire report before connection. Source uses 1–80 ASCII letters,
digits, `:._-`; text strips control characters, trims and retains at most 80 Unicode scalars.
An empty token value removes its key; empty optional presentation text is omitted. State labels
require nonempty normalized text. Repeated state/token options use the last value. At least one
presentation set/clear or token change is required. Setting and clearing the same field fails, including explicitly supplied empty text.
Pure presentation sets replace the source record; including any presentation clear changes to
selective patch semantics. Optional Agent/source guards filter display; they confer no authority.

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
reply, or `unknown` with a closed reason), `interrupted` and `cleanupFailed`. Catalog v86 includes
89 schemas. OBWP 33/34 carries `workbench.panel-metadata` and its result; request limit is 16 KiB,
result limit 512 KiB. Server reads use read permission and reports navigation permission under
the Site gate. Reads expose source records for up to four ordered unique Panels. Metadata and
Workbench revisions remain separate. Same-cohort deployment is required; old peers reject the
unknown carrier kind. There is no persistence migration; restarting the Site clears reports.

Protocol/owner tests cover atomic changes, replay and TTL; shared Rust/TypeScript/JSON corpus
checks bound and malformed inputs; named-pipe tests check exact correlation including binding,
response loss, deadline and no replay. Parser, receipt, localization and shell grammar checks
cover the CLI projection. TUI rendering, general events and provider authority/session reporting remain separate
integration obligations; the query projection below consumes current Server reports. This command alone
does not establish complete Herdr Panel/Agent parity or Desktop support.

## Query observation model

Panel list/get/current and Agent Get consume the Server's metadata through one bounded query
collector. Its inputs are the already admitted connection, original Snapshot, ordered Panel IDs,
absolute deadline and cancellation signal. It reads batches of four, requires one metadata
revision across batches, checks every returned binding against the original primary membership,
and ends with an empty read at the same revision. The outer query still fences canonical and
selected Client state. Failure never publishes a mixed or partial metadata projection.

Each projected `metadata` object contains `source` (Site, Workbench and metadata revisions plus
all read request IDs), current `binding`, `tokens`, raw source `presentations`, guard `context`
(`agent`, `authoritySource`) and computed `presentation` (title, displayAgent, stateLabels).
Missing context is explicit null and never satisfies a guard. Until actual Agent authority is
integrated, both context facts are absent; guarded records remain visible as raw reports but do
not override display. Provider IDs, user names and generic observation categories are not
substituted for the missing authority. This dependency remains part of full parity acceptance.

The collector is a read workflow, not a second metadata owner. No Host availability is required
for Server metadata. Agent Get uses only a unique live primary binding; ended, unbound, ambiguous
or subsequently changed Agent membership discards that metadata with its terminal sample. Report
metadata never replaces independently observed OSC title, canonical label, status or readiness.
The collector's tests must cover batch and final-fence changes, missing/mismatched rows/bindings,
nonmatching guards and retained global tokens, cancellation, original deadline and no replay.

Windows Panel list/get emit v8, current emits v5, and Agent Get emits v9 (v10 with `--client`).
The metadata object is required on each Panel row or non-null Agent terminal. Legacy query
versions reject it; their previous shapes remain accepted. Text prints escaped effective fields
and global tokens. Catalog v86 contains 89 schemas and 100 command paths.

Admission caps the sum of encoded metadata rows at the existing 8 MiB CLI output budget before
extending the collector; the final envelope retains its separate preparation bound. A batch
that exceeds the budget fails atomically with output-too-large, without partial publication.
