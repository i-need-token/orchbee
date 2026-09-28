# Agent screen detection model

The Server owns fallback screen interpretation separately from explicit Agent reports and process identity. A successful bounded terminal sample can describe working, idle or blocked UI; it cannot prove which provider owns a process, authenticate it or create a canonical Agent Session. The reference is Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, its 22 bundled detection manifests and `pane/agent_detection.rs`. The Windows periodic sampler compiles the deterministic engine once before startup and publishes admitted screen fallback. Physical explain consumes the same engine through the bounded observer queue. Startup now loads bounded local overrides and eligible remote cache with typed provenance warnings. Manifest queries and retained live reload now use the observer owner; remote updates remain an integration obligation.

## Inputs and rule ownership

The private engine accepts a known provider or absence, recent live terminal text, OSC title and OSC progress as separate fields. Screen input is limited to 64 KiB and each OSC field to 4 KiB; over-limit input is rejected whole, never truncated into misleading evidence. Text is already decoded terminal content, not raw escape sequences. Unknown providers remain unknown even when text resembles another provider. Known providers with no matching rule use idle fallback, with no visible confidence flag. Omp and Mastracode have no bundled screen manifest in the reference and use this known-provider fallback.

Bundled TOML files are retained under `resources/agent-detection` with their Apache-2.0 license and source attribution. Server maintainers own review and pinned updates. Parsing rejects unknown fields, invalid states/regions, empty rule identifiers, unsupported engine versions, invalid skip-state combinations and excessive complexity before compiling any executable rule set. Each manifest allows at most 128 rules, 512 gates, depth 8 and 1,024 matchers; each gate allows 32 direct matchers of at most 512 Unicode characters. Regex compilation has explicit memory limits. The already locked `regex` and `toml` packages supply syntax that the standard library cannot implement; no new versions or runtime framework are required.

## Deterministic decisions

| Input or match                          | Result                                                                                                    |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| No known provider                       | Unknown, no visible evidence                                                                              |
| Multiple matching rules                 | Highest priority; first declared rule wins equal priority                                                 |
| Contains                                | All needles, Unicode lowercase comparison                                                                 |
| Regex / line regex                      | All patterns; case-sensitive unless the expression changes flags; line patterns may match different lines |
| Nested all / any / not                  | All children / at least one child if present / no child matches                                           |
| No matching rule for known provider     | Idle without visible idle evidence                                                                        |
| Transcript/history viewer rule          | Skip publication, preserve existing fallback and clear pending idle confirmation                          |
| Visibility flag inconsistent with state | Flag is false in the derived result                                                                       |

Regions follow the reference's live prompt, response markers, prompt boxes, top/bottom nonempty lines and separate OSC fields. Line boundaries retain correct UTF-8 offsets for LF and CRLF, including blank lines and a final unterminated line; no byte offset may split a Unicode character. All matching is pure and does not access files, processes, clocks or mutable global state. Compiled rules may be retained by the sampling owner and reused.

## Publication integration contract

The Windows Host adapter validates the exact attachment, Worker generation and Host identity and associates screen evidence with the same observed PID/creation identity. It may not reinterpret a failed/missing frame as an empty screen or derive idle from unavailable data. Existing process exits retain precedence. Full-lifecycle report authority remains stronger than fallback; visible blockers only override weaker matching authority through the existing report aggregate.

Each owned read-only viewer consumes its initial full frame, requests a fresh snapshot with StoreReset for that viewer alone, then reads the bottom live-height rows at that exact frame revision and generation. UI scroll position does not choose detection text. Both surrounding complete process samples must select the same PID, creation time and provider. A failed or stale frame/row read supplies no screen evidence; the successful complete process sample can still establish presence or exit. Transport rows are bounded to 8 MiB and decoded text to 64 KiB, preserving physical newlines and trimming trailing whitespace/blank tail rows. Title and progress come from the same frame; typed progress projects the current OSC 9;4 state, not raw escape history. No Client display frame is acknowledged.

The coordinator retains screen stability and a three-second startup grace per physical process identity. Process-only observations preserve screen timing; identity departure/replacement resets it. Changed explicit-report input permits fresh matching screen evidence to republish the settled fallback without bypassing idle confirmation. Prepared timing and fallback commit only after report/metadata admission succeeds. Bundled rule license and attribution ship in the Windows cohort under `licenses/agent-detection`.

Working-to-plain-idle transitions hold for three further confirmations or 700 ms; explicit visible idle/blockers, provider change or exit clear that hold. Stable visible blockers refresh at 800 ms so newer weak reports can be challenged without publishing every sampling tick. Startup grace and sampling cadence are adapter responsibilities. Screen and process sampling must retain the existing required-completion and uncertain-viewer-cleanup ownership. No controller/input/focus effect is admitted.

## Evidence and delivery

The accepted rule-maintenance model gives the Server one immutable compiled ruleset and its
provenance. Windows startup resolves the shared configuration-file parent without rewriting user
settings. For each bundled provider, `<config>/agent-detection/<report-label>.toml` overrides
`<config>/agent-detection/remote/<report-label>.toml`, then bundled rules. Reads admit regular UTF-8
files of at most 256 KiB with the same provider, engine and complexity validation as bundled rules.
Present versions must be dotted numeric; remote caches must also declare an engine version and
be no older than bundled versions. Trailing zero segments compare equally. Normalized manifest
identifiers or aliases may match the provider. Invalid override/cache inputs produce typed warnings and fall back to the
next valid source. Providers without bundled rules retain known-idle fallback. The sampler and
Explain consume the same installed ruleset. Explain provenance identifies the actual path, cached
version, local shadowing and warnings; it does not manufacture update status. Canonical CLI v1 and
closed physical v2 remain compatible; enriched physical explanations use v3. The full maintenance
workflow includes current-manifest queries, retained bounded reload and stabilization reset, but still requires an
owned real updater with atomic cache commits, truthful origin/status and cancellation/cleanup.

Physical `agent explain` is a read, owned by the same detector and temporary-viewer lifecycle.
An exact binding and current Workbench/report context are prepared under the Worker/Site fences.
Live full-lifecycle authority produces an explicit screen-skipped explanation without reading Host
content. Otherwise the effective known provider, or the detected provider behind a custom label,
selects the rules. A fresh frame/row/OSC capture is bracketed by stable complete process samples;
the same binding and report context must still hold before publication. A changed context refuses
without retry. Raw rule prediction is distinct from the effective, stabilized reported state.

The process-observer thread owns at most one executing and one queued diagnostic request, together
with its existing sole pending viewer cleanup. CLI cancellation ends the caller wait; it is not a wire cancellation request. The Server gives
each diagnostic one four-second queue/read budget; timeout and Site shutdown stop new work, while
admitted Required exchanges still settle. Unconfirmed Detach keeps ownership and blocks
new attachments. Queued expired work cannot attach. Diagnostics never send input, acquire control,
change Client focus or publish a new Agent state. Periodic observations keep their existing owner.

Explain exposes the actual manifest source/version, every evaluated rule's identifier, priority,
region, state, match and top-level matcher evidence, gate counts and region byte count. Region
previews take 240 Unicode scalars and append `...` when longer. First-declared highest-priority
matching rule wins; fallback and skip reasons describe the same deterministic evaluation. The
existing input/manifest/complexity limits and 512 KiB PanelAgent result bound apply; over-limit
results refuse whole. Rule maintenance remains an integration obligation, not invented update
metadata. Pure rule witnesses, exact-context refusals, queue cancellation/cleanup traces and CLI
transport/schema checks precede final Windows Host/provider acceptance.

Rule, region, bounds, priority and malformed-manifest tests exercise the engine before transport integration. Pinned-reference cases and differential evaluation must cover every bundled provider, nested gates, transcript skip and OSC precedence; a passing static engine does not close the live CLI/TUI requirement. Later Host acceptance must prove real Windows screen changes and report arbitration without user query effects. Source/cohort revert is rollback; no durable state or wire migration is involved. The final full gate remains deferred until all parity implementation is complete.

## Installed manifest maintenance

The Windows process observer owns the installed detector and a Site-lifetime revision starting at 1.
`workbench.agent-manifests` (OBWP 37/38) reads this snapshot, submits a reload identified by an
operation ID, or reads that retained operation. Queries never reread files. Reload reads the same
configuration parent resolved at startup. It runs between observer turns; sampling and Explain
switch together. Under the maintenance lock and Site gate, installation clears screen stabilization
without discarding process identity or explicit reports, then publishes the new engine and revision.

Only one reload may be queued or running. Up to 1,024 compact operation receipts are retained for
the Site lifetime without eviction; duplicates return the existing receipt, and busy/capacity
refusals occur before admission. Operations transition pending -> applied(revision) or failed(reason).
Stopping cancels pending operations before installation. A failed load retains the prior engine.
Read/status use read policy; reload uses navigation-management policy. CLI uses one bounded budget,
submits once, and retains the operation ID with pending/unknown results after lost acknowledgement
or interruption. Status recovery never resubmits a mutation. Receipts end with the Site generation;
this is not durable replay across Server restart. Remote download and automatic-update policy are
separate remaining capabilities and are not represented as successful updates by this interface.
