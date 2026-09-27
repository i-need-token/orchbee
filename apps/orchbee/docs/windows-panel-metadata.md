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
presentation set/clear or token change is required. Setting and clearing the same field fails.
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
reply, or `unknown` with a closed reason), `interrupted` and `cleanupFailed`. Catalog v85 includes
89 schemas. OBWP 33/34 carries `workbench.panel-metadata` and its result; request limit is 16 KiB,
result limit 512 KiB. Server reads use read permission and reports navigation permission under
the Site gate. Reads expose source records for up to four ordered unique Panels. Metadata and
Workbench revisions remain separate. Same-cohort deployment is required; old peers reject the
unknown carrier kind. There is no persistence migration; restarting the Site clears reports.

Protocol/owner tests cover atomic changes, replay and TTL; shared Rust/TypeScript/JSON corpus
checks bound and malformed inputs; named-pipe tests check exact correlation including binding,
response loss, deadline and no replay. Parser, receipt, localization and shell grammar checks
cover the CLI projection. Full Panel/Agent query projection, TUI rendering, general events and
provider authority/session reporting are separate integration obligations. This command alone
does not establish complete Herdr Panel/Agent parity or Desktop support.
