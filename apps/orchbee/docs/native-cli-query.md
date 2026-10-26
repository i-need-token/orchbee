# Native CLI Query

## Canonical grammar boundary

Panel-family commands use a positional Panel or `--current`. Agent targets additionally accept `--terminal` and `@terminal:` for exact child or primary terminals. Startup, integration and doctor use `--provider antigravity`; `antigravity-cli` stays a private adapter identity. CLI event names use `panel.*` in event/filter input and v1 receipts. Terminal and Agent reads use `--source`/`--lines`; the read/history projection descriptions below cover internal content evidence. Source waits default to 30000 ms, with `--forever` as the explicit unlimited form.

The parser in `src/invocation.rs` is the grammar authority: it admits command paths, per-command option scopes and values before any runtime access. `src/cli/completion.rs` declares the projection that help and every completion script consume, including command paths, `COMMAND_FLAGS` option scopes and value domains. `src/cli/text/examples.rs` supplies one parser-checked example per command. Tests keep them in agreement: help and completion list the same command paths, every help example parses, `--format` admission matches help, and on Windows each option is admitted where help and completion declare it and rejected elsewhere. Options whose admission depends on another option are listed with that reason in the test.

## Sessions and background services

An **Orchbee Session** is one independent work environment containing Workspaces, Tabs,
Panels, terminals and Agents. Its background service owns the shared state and processes;
CLI and TUI Clients connect to it. Leaving a Client keeps those programs running.
Start with the default Session and use Workspaces to organize ordinary work. Windows named
Sessions provide separate runtime endpoints and persisted state for independent sets of terminal programs.

`--session <name>` selects that environment; `session list/attach/stop/delete` manages named
Sessions, while `server start/diagnose/stop/config/reload-config` manages the selected Session's
background service. An **Agent session** identifies one Agent conversation/runtime record and
uses `--agent-session <id>` or an ordinary Agent target. A protocol **Client Connection** identifies
one admitted connection; it is not a separate work environment.

Use `orchbee server -h` to list service commands and `orchbee server stop -h` for confirmed
shutdown and recovery options. In PowerShell, use `.\orchbee.exe server -h` from the binary's
directory, or invoke its full path when it is not on `PATH`.

| Previous spelling          | Canonical spelling               |
| -------------------------- | -------------------------------- |
| `--site <name>`            | `--session <name>`               |
| `ORCHBEE_SITE`             | `ORCHBEE_SESSION`                |
| `site <command>`           | `server <command>`               |
| `agent ... --session <id>` | `agent ... --agent-session <id>` |

Only canonical CLI spellings are accepted. `ORCHBEE_SITE` remains a legacy environment fallback
below `ORCHBEE_SESSION`, preserving existing routes. Existing runtime names, scope hashes, catalogs,
checkpoints and JSON schemas remain unchanged; machine fields such as `siteId` and schema IDs
such as `orchbee.site.start` retain their existing protocol meaning. This terminology change
does not make the Electron Desktop runtime a consumer of the Session service.

## Schema versions for independent Agent status

Every CLI machine document, including the offline API catalog, has one current contract and version `1`. Platform capabilities and options never select a document version or remove its members. A member unsupported on the platform is `{kind:"unsupported"}`; `null` means the source said none and `"unknown"` means the source cannot say. Whole-command platform refusal uses `stage: "platform"`, `reason: "unsupported-platform"`, and exit `4`. The [CLI output standard](../../../docs/cli/output.md#machine-documents) owns these rules.

Agent List uses rows with separate canonical, terminal, placement and Client facets; Agent Get uses `{kind:"projected",agent,inspection}`. Windows joins physical and canonical observations. Other supported platforms retain canonical observations and mark unavailable facets unsupported. `--client` selects evidence in existing members. Canonical and terminal subjects remain distinct tagged business variants.

## Current CLI discovery and platform contracts

Completion keeps local `--session` selection separate from Agent session selectors. On Windows, get/read/prompt/send-keys/wait/focus offer static `@agent:<kind>` provider aliases before a target; they never query runtime state or invent ordinal suggestions. In PowerShell, start an alias with a quote (for example, `'@agent:c`); completion inserts the full target as a safely single-quoted handle because an unquoted `@agent` token is parsed as splatting syntax. Windows POSIX-shell adapters filter remote `--machine` paths using the same static grammar as PowerShell and Elvish, including nested families and root data commands. Known options and operands keep suffix `--help` discoverable; unsupported routing, duplicates and terminated options do not regain candidates.

Snapshot v1 and Tab Get v1 admit the canonical optional split `ratio` (0.1–0.9); omitted means 0.5. Windows Agent Doctor v1 covers all 20 providers.

Target help states ID/selector precedence: a display label shaped like an ID or a recognized `@kind:` selector is addressed through the resource’s real ID. Creation and rename keep the literal display label. `panel resize --amount` is a split-ratio step (default 0.05, absolute value capped at 0.5).

Agent startup help maps `--provider antigravity` to integration/doctor `--provider antigravity`. A missing startup Panel identifies the target failure and directs users to `orchbee panel list`. A local manifest read failure directs users to the path and read permissions configured by `ORCHBEE_UPDATE_MANIFEST`; its machine reason remains `download` at the manifest phase.

Launch Profile list/get use version 1 and admit POSIX, drive-absolute and verbatim drive executable paths. Linux and Windows diagnosis use `orchbee.site.diagnose` version 1 for their supported scopes; unsupported facts are explicit members. Integration lists always include `outdatedOnly`, inspection counts and provider ID lists, with bounds covering all 20 providers. Generic failures select closed details by semantic stage, including local plugin preparation at `stage: plugin`. Session catalog and startup environment failures preserve their failed runtime stage. Client selection diagnostics identify Clients and the applicable `--client` remedy.

The event stream schema exposes typed filter input at `$defs/filter`; plugin runtime exposes
context input at `$defs/context`. Help includes valid JSON examples and schema lookup hints.
Schema validation describes structural input; CLI decoding also enforces semantic combinations,
Unicode scalar decoding and UTF-8 byte bounds. Wide human tables use labeled records when
their measured width exceeds 80 cells. Field blocks always separate labels and values, and
full IDs remain available for copying; JSON projections are unchanged by text layout.

## Workspaces and optional Projects

Project listing follows the independent Navigation `projectOrder`, with canonical ID order when
omitted or empty. ID/name lookup stays independent of display position; Workspace listing keeps
using its independent `workspaceOrder`.

Navigation has global `directories` and `workspaces` arrays; `projects` contain only ID/label. Workspace `projectId` is required-nullable. Project list/get v1 derive directories from member Workspaces; Workspace list/get v1 resolve globally and display ungrouped membership. `api snapshot` v1, Worktree query/list v1, Workspace close-status v1 and catalog v1 publish these shapes. Semantic event stream/wait envelopes are v1: Workspace data carries grouping only on the nested Workspace, and Worktree operation metadata is nullable.

`workspace set-project <workspace> --project <project>` changes grouping; `workspace create-project <workspace> --label <label>` atomically creates a Project and moves the Workspace; `workspace ungroup <workspace>` removes grouping; `project dissolve <project>` ungroups all members and removes the Project. These change organization metadata only, retaining exact IDs, cwd, PTYs, directories and order. `workspace move`/`move-block` remain separate reorder commands. Projects can organize multiple repositories. Ordinary close affects only the selected Workspace; batch closure uses the explicit Project commands below.

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` pastes one command and sends Enter in one input request. Supply one quoted string; text is preserved, and targets are exact IDs or `@terminal:0x<hex>`. The receipt confirms input submission, not Shell readiness, command completion or exit status; never replay partial or unknown delivery. [Command submission contract](native-terminal-run.md).

Status: current implementation and correctness contract for the native `orchbee`
command line, covering queries, Workbench mutations, terminal observation and input,
Agent names, and local provider onboarding. Queries observe an existing Session; Workbench
mutations follow the mutation contract below. Only explicit `server start` and the TUI
startup path may activate a Session. Agent state comes from the Server's canonical model,
never terminal-text inference; the Server owns names, while `agent integration` manages
only the invoking user's local provider configuration. Each command's effects, budgets,
and release boundaries are specified in its section. CLI commands need no TTY; the
interactive TUI has separate terminal admission requirements. The native CLI remains
a native development surface.

Authority: this record owns the public grammar, result envelopes, failure matrix,
budgets, exchange state machines and output effects implemented under `src/invocation.rs`
and `src/cli/**`. Prose and implementation must converge in the same change.

## Clear terminal output

Windows `terminal clear <terminal>` clears one exact bound primary or child terminal
in an existing Session. It preserves the cursor's complete soft-wrapped prompt line,
removes preceding history and other output, clears selection and returns the viewport
to the active screen. An alternate screen is refused without changing content.
Terminal modes, title, working directory and child process remain intact. This follows
the pinned Herdr `pane.clear` implementation, rather than injecting terminal input.

The public terminal control action `clear` names an admitted read viewer. The Server
checks its current Session exposure, exact physical binding and Worker generation,
then allocates an independent transient input attachment identity. The Worker uses
that viewer's exact Host target, attaches input-only without a resize lease, submits
one Host `ClearTerminal` with `preservePrompt: true`, validates attachment/terminal
and positive generation/revision acknowledgement, and detaches. Existing controller
ownership and resize geometry are unchanged. Every ClearTerminal request includes `preservePrompt`; explicit false selects screen erase/home.

One cancellable five-second CLI budget covers discovery and submission. Host work
has a three-second effect budget and a separate one-second cleanup allowance.
Pre-submission failures cannot clear content; a missing or malformed acknowledgement
after submission is unknown and never replayed. A known clear with unconfirmed
cleanup retains the confirmed effect and exits 8. Closing a failed Host carrier
releases its transient attachment instead of stacking uncertain attachments.
The CLI releases its viewer after the last consumer on every outcome. Cancellation
retains an already submitted receipt and exits 130. No Session startup, Client focus,
controller takeover, PTY input, process reset or browser effect is part of clear.

Closed `orchbee.terminal.clear` v1 carries the exact target, operation identity and
typed outcome, including the new content revision/generation and cleanup confirmation
for a known clear. Success exits 0, typed refusal 3, unavailable 4 and unknown effect 8.
The existing controller codecs own the additive clear request/result; Host protocol
peers own the optional prompt-preserving field. Scoped contract, Host runtime and
Windows pipe tests establish these boundaries; deployed Host/TUI acceptance and the
whole repository gate remain final-stage work under the User's standing authorization.

The operation identity is a request correlation, not a retained idempotent receipt; no status lookup or automatic retry is advertised.

## Read a terminal selection

Windows `terminal selection-read <terminal> --anchor ROW:COL --cursor ROW:COL`
reads an inclusive, nonrectangular range from an existing terminal. Coordinates
are zero-based physical rows in the complete retained screen buffer, independent
of any Client viewport; rows are unsigned 32-bit and columns unsigned 16-bit.
Reversed endpoints are ordered before reading. The selected span admits at most
4096 rows and one existing 8 MiB row-window budget. Invalid points, duplicate or
foreign options, and a larger span fail before Session discovery.

Optional `--content-revision <u64>` pins the expected content revision;
`--generation <positive-u64>` additionally pins its geometry/store generation.
Without these, the command captures a fresh frame. Every row read still uses the
same exact frame revision, generation, attachment, Host epoch, dimensions and
retained row count. Stale or evicted data fails whole without retry or partial text.
Rows/columns outside that frame fail rather than selecting a nearby cell.

The command owns one cancellable five-second existing-Session budget and one
temporary read-only viewer, released after the last consumer on every outcome.
It never takes a controller, changes selection/viewport, sends input, writes the
clipboard, acknowledges a user's attention, starts a Session or navigates a Client.
Cell byte mappings preserve complete wide and combining glyphs. Hard-line padding
is trimmed; adjacent verified soft-wrap rows join without a newline. The result
contains the original endpoints, exact terminal binding and captured content
revision/generation/Host epoch. `orchbee.terminal.selection` v1 is a closed JSON
receipt; text output escapes controls through the existing CLI presentation owner.
Existing typed viewer/read-history failures retain stale-revision,
stale-generation and history-evicted reasons. Cancellation exits 130; no replay
or durable state is introduced.

Evidence covers reversed/single-cell and blank ranges, wide/combining mappings,
soft/hard wraps, geometry and span limits, parse/help/completion/schema admission,
and real Windows CLI pipe traces for a successful read, stale expected content,
wrong row routing and typed Host refusal with deterministic viewer release.
External scrollback editing and final Host/emulator acceptance remain separate
obligations; copy motion/search are specified below.

## Resolve a terminal link

Windows `terminal link-resolve <terminal> --cursor ROW:COL` resolves the cell in
an explicit native viewport without opening a browser or invoking a plugin.
Viewport coordinates are unsigned 16-bit, zero-based; optional
`--offset-from-bottom <u64>` defaults to zero and defines the viewport's first
physical row as captured scrollback rows minus offset. An offset past retained
history or a cursor outside captured dimensions fails. This CLI-owned viewport
is independent of a TUI Client's focus and scroll; no Client viewport is changed.
Exact bound primary/child terminal targets, optional full-u64 `--content-revision`
and positive-u64 `--generation` use the same existing-Session read-only five-second
viewer and full frame/Host/binding fences as selection and copy.

The resolver first reads one strictly validated render row to inspect the clicked
whole glyph's OSC 8 target, including wide tails. An explicit hyperlink returns
its complete URI and an empty regions list, matching the reference's separation
between frame-owned OSC 8 highlighting and server-resolved plain-text regions.
Without OSC 8, complete mapped text rows are read only as needed to prove the
whitespace-delimited token across verified soft wraps, including token ends
outside the requested viewport. Missing neighboring context triggers a bounded
extension; hard row boundaries and Unicode whitespace stop the token. Verified
wide spacer heads add no text. Plain detection follows pinned Herdr: lowercase
http/https prefixes, trailing quotes/punctuation trimmed, balanced round/square/
curly closers retained, and a click outside the resulting URL yields no link.
The reference shared8192-cell inspection budget includes selected physical
cells, wide spacer/tail cells and boundary probes in both directions; exhausted
inspection yields no link rather than a truncated URL. Native
encoded-byte/glyph/65536-row scan limits still fail explicitly rather than
returning an unproven empty result. Visible inclusive regions preserve full
wide/combining cells and are clipped to the requested viewport.

Closed `orchbee.terminal.link-resolve` v1 includes the complete URL or null,
`plain-text`/`explicit`/`none` kind, visible regions, viewport origin/offset and
captured content revision/generation/Host/terminal binding. Empty is a successful
no-link result under the reference's bounded token policy, including exhaustion
of its inspection budget. Malformed/duplicate/foreign options fail before
runtime discovery; stale/evicted/misrouted/incomplete records fail whole, with no
retry or partial regions. This operation never obtains a controller, sends input,
acknowledges user attention, changes selection/clipboard/focus, starts a Session or
writes durable state. Schema/help/scoped Windows completion and machine read
forwarding share the typed request. TUI hover/click wiring
and physical Host/emulator acceptance remain separately scoped consumers.

## Activate a terminal link

Windows `terminal link-activate <terminal> --cursor ROW:COL` accepts the same
offset, revision and generation options and explicit native viewport as resolve.
It resolves the full clicked URI/URL before choosing an enabled, available Windows
plugin handler. Plugin IDs are ordered lexically; handlers retain manifest order.
Both handler and referenced action must support Windows. Invalid regex handlers
are unavailable; matching uses the existing linear-time regex engine with a1MiB
compiled-pattern ceiling and deadline/cancellation checks between handlers.
No link or no matching handler returns `handled=false` without invoking a plugin.

A match collects display context for the exact clicked primary or child terminal,
including its canonical Panel/Tab/Workspace and corroborated cwd/Agent/worktree
facts. Unrelated caller environment and Client focus do not select this source;
unknown facts remain absent. `invocationSource=link_click`, the full clicked URL,
handler ID and operation correlation are fixed by this workflow. Existing plugin
context ceilings apply: URL4096 UTF-8 bytes, aggregate context16KiB and request32KiB.
A matched URL that cannot be admitted fails before submission, without truncation.
After context collection, a strict same-revision render-row read fences the clicked
content, and the selected catalog entry is checked again before one existing
plugin invocation. These are pre-submit observations, not an atomic catalog/content
transaction or a content lock after submission.

The Server alone resolves the current action executable, plugin-root cwd and
protected environment. Existing Worker Jobs own launch, output logs and cancellation;
display data cannot supply argv, directory policy or Worker authority. This command
does not open a browser, send terminal input, navigate/focus, change clipboard or
start a Session. One existing viewer and the entire preflight/submission share five
seconds and cancellation. A matched effect path requires one correlated Detach
acknowledgement after its final content/member fences and before plugin submission;
a rejected or uncertain Detach submits no plugin and is never retried. Read-only
and no-effect outcomes retain best-effort Detach plus session EOF cleanup.
An admitted plugin Job outlives that temporary viewer under its existing Job owner.

Closed `orchbee.terminal.link-activate` v1 retains the resolved link, viewport and
content/binding receipt, plus activation state and operation identity. `handled=true`
requires a correlated known Started answer; it means Job admission, not command
success. Known refusal exits3. A lost post-submit answer or unknown/malformed outcome
exits8, preserves the operation identity and never retries; cancellation exits130.
No-handler is a successful no-op. Existing plugin logs expose admitted Jobs; existing
read receipts remain unchanged. Bilingual help/schema/scoped completion and machine
relay share the typed command. Focused evidence covers ordered/platform matching,
context bounds, exact target context, pre-submit stale data, start/refusal/loss and
viewer release through actual Windows pipes; deployed Host/TUI acceptance remains.

## Terminal copy motion and search

Windows `terminal copy-motion <terminal> --cursor ROW:COL --motion <motion>`
computes a new screen-buffer cursor without changing Client selection or sending
input. Motion names are `line-end`, `first-non-blank`, `next-word-start`,
`previous-word-start`, `next-word-end`, `next-big-word-start`,
`previous-big-word-start`, `next-big-word-end`, `previous-paragraph` and
`next-paragraph`. Both word classes cross verified soft wraps; hard rows insert
whitespace. Ordinary words distinguish whitespace, tmux ASCII punctuation
(excluding underscore), and other characters; big words distinguish only
whitespace. Results remain at whole glyph heads, including wide tails and
combining text. Line end is the last character after hard-row padding trim;
first nonblank is the first non-whitespace glyph. Kitty Unicode placeholder
cells are whitespace for copy motion/search, matching the pinned reference. Paragraph motion finds the
nearest blank physical row in the requested direction, preserving column and
checking at most 999 rows, as the pinned reference does. No target returns the
original cursor. Out-of-buffer points fail.

Windows `terminal copy-search <terminal> --cursor ROW:COL --query <literal>`
`--search-direction forward|backward --content-revision <u64>` searches complete retained
logical text. The query is preserved, admits at most 4096 UTF-8 bytes, and is
literal with Unicode smart case: any uppercase character makes it case-sensitive.
An empty query yields no matches after the content fence. Optional
`--previous ROW:COL-ROW:COL` is an ordered inclusive prior match; forward origin
is its end, backward origin its start. Without it the supplied cursor is the
origin. The next match starts strictly after a forward origin, or ends strictly
before a backward origin; absent one, search wraps to first/last. Match boundaries
must coincide with complete glyph boundaries. Padding is trimmed at hard logical
line endings; a verified wide spacer head adds neither text nor a word boundary.
The result preserves the exact total and global current index, while returning
at most 1024 matches centered around the current match and its local index.
Zero matches have null current indexes. Index/count u64 values use decimal strings.

Both commands share selection-read's existing-Session, five-second cancellation,
exact terminal/frame revision/generation/Host/geometry and viewer release owner.
Motion optionally pins `--content-revision`; search requires it. Both additionally
accept `--generation <positive-u64>`. Each rows request has its own increasing
sequence and uses the existing per-window byte/cell limits; adjacent windows
must agree on soft-wrap continuation. A complete admitted scan precedes output:
at most 65536 rows, 8 MiB of aggregate encoded row windows/logical text and 1048576 stored
glyphs. Exceeding bounds or stale/evicted/misrouted data fails whole without
partial matches, retry or an invented no-op. Line motions read one row;
paragraph and word motion begin with 64 rows and extend only when needed.
Paragraphs stop at the nearest blank or their 999-row distance; word starts/ends
are returned only after their boundaries are proven, extending backward through
soft continuations or forward through unfinished word ends. Adjacent chunks keep
the same frame and global byte/glyph budgets. Search covers the complete retained buffer.

`orchbee.terminal.copy-motion` and `orchbee.terminal.copy-search` v1 are separate
closed receipts, retaining original request data, exact terminal binding and
content identity. Default text localizes labels and escapes query/control text.
The deterministic `terminal_copy` domain owns cell/word/logical-line decisions;
the CLI read adapter alone owns discovery, exchange, deadline and cleanup.
Evidence covers all ten motions, Unicode and whole glyphs, row/window boundaries,
literal/smart-case/empty search, previous origins, wrapping and exact totals beyond
1024, parse-before-effect limits, and actual Windows CLI success/stale/forged
reply/release traces. Final deployed Host/emulator acceptance remains separate.

## Workbench status

Bare `status` reads one Workbench snapshot and reports reachability and resource counts. Text
includes the selected Session name only when `--session` was explicit. It always includes the
executing Client's package version. On Windows, `status server` reports the Server's actual
build metadata.

## Workbench topology queries

Windows `workspace list/get` return `number`, `tabCount`, and `panelCount`;
`tab list/get` return `number` and `panelCount`. Workspace numbers are one-based
positions in the complete canonical navigation order, before `--project` filtering.
Tab numbers are one-based positions within their terminal context, independent of
manual or automatic labels. Reordering changes numbers without changing identities.
Empty contexts have zero Tabs and Panels. A Panel is one leaf of a Tab's split tree;
child presentations and terminal bindings never increase its count.

Human `project list`, `workspace list` and `tab list` print a `HANDLE` column: the
stable `@<kind>:0x<hex>` spelling of the exact identity, omitting only leading zero digits.
Filtering, creation and deletion of other members never change these handles. Deleted exact
identities return not-found; compact identities retain every significant digit, up to 32. `workspace list` uses the localized `HANDLE`, `NAME`, and `PROJECT`
columns; `tab list` uses `HANDLE`, `NAME`, `PANELS`, and `WORKSPACE`. An ambiguous target's
text diagnostic lists up to eight candidates as these handles with their quoted labels,
plus the true total; JSON keeps the identifier `candidates` and `totalCount` details.
Project and Workspace ownership cells show their canonical names. Name cells are quoted, control-safe, and clipped
to keep each table within 80 terminal cells. These lists do not treat a Workspace's
primary directory as a process CWD or expose internal context IDs as names.

`panel list` includes `workspaceId`, plus the primary
presentation's `label`, nullable `customLabel`, and `displayLabel`. These are the
same canonical values returned by `panel get`, not inferred process or Agent titles.
JSON preserves labels exactly; text escapes them and bounds list display width.

These projections consume one validated Workbench snapshot and retain its source
revision, existing-only admission, bounded output, and headless availability. They
perform no navigation or mutation. Runtime observations are described below.

Human `workspace list` and `tab list` output uses snapshot-only Text tables, with no
optional `Agent`, `worktree`, `metadata` or `viewport` joins. With `--client <id>`,
Text still validates that exact ready Client's current selection and membership but
adds no columns. The joins and final consistency checks described below still apply
to JSON for these two lists and to other existing queries that use them.

Under JSON, Windows accepts optional `--client <id>` on these six list/get
queries to add global focus and the Client's remembered active Tab in each
Workspace. The [Client query contract](windows-client-control.md#client-relative-workbench-queries)
owns this bounded, read-only join and its provenance and consistency checks.
`orchbee.workspace.list/get`, `orchbee.tab.list`, `orchbee.tab.get` and `orchbee.panel.list/get` all use version 1; discover the complete embedded catalog
with `api schema`. Retained selection-read controls use `orchbee.client.control` v1;
viewport reads use v1; other controls retain v1. `orchbee.cli.failure.client-query` v1 identifies selected
Client read failures; existing target and admission diagnostics remain intact.
Process and worktree observations use their respective owners as described below;
reported metadata remains a separate owner and is not implied by topology fields.

Verification covers global reorder and filtering without renumbering, empty and
nested trees, child presentations and unbound leaves, list/get agreement, automatic
labels, escaped text and schema validation of real Windows query documents. A composed
`WorkbenchQuery` witness checks table-only Text output in both locales and confirms the
JSON path does not enter that shortcut.

## Workbench Agent summaries

Windows Workspace/Tab/Panel list/get and Panel current queries add one root `agents` observation with
its canonical Agent revision, List request IDs and matched live session summaries.
Rows carry `agentSessionIds` and nullable `agentStatus`. Only navigable primary
Presentation bindings contribute; child presentations and ended sessions do not.
A Session counts once per row even when it has several matching bindings. Multiple
matching Sessions remain explicit. Missing bindings produce an empty set only after
a complete successful canonical list, never after a failed or truncated read.

Agent pages contain at most 100 summaries; the composition admits at most 512 live
Sessions. All pages must share one revision and contain unique IDs. Repeated cursors,
empty continuation pages, overflow and revision drift fail without replay or partial
output. The CLI uses one admitted connection, signal owner and five-second total,
and verifies the original Workbench revision before publication. An Agent observation
and a Workbench snapshot retain separate provenance; this is not a distributed atomic
snapshot. There is no process scan, text inference, startup, input, navigation or
acknowledgement effect.

With `--client`, up to 100 matched Sessions per attention batch use the existing
[authenticated Client join contract](native-client-attention.md#joining-canonical-agent-reads).
When there are matching Sessions, a later complete Agent list supplies the output; journal continuity, generation and
sequence corroborate each Client projection. A new Session is explicitly missing,
not presumed seen. With no initial match, the first complete list is the observation. Workbench and Client view revisions must remain unchanged.
The root preserves canonical summaries and optional per-Session Client projections.

Aggregate priority follows the canonical attention/error/outcome/working/idle/unknown order: blocked, error, done, working, idle, unknown. Idle remains idle, succeeded maps to done, and failed/canceled/interrupted map to error, independently of Client acknowledgement. Stale/disconnected observations contribute unknown and remain explicit in session facts. No matching Sessions yields unknown. The no-Client path never invents seen. Text localizes aggregate status and escapes display data; JSON is locale independent.

`orchbee.cli.failure.workbench-query` v1 reports bounded composition failures with
`stage: workbench-query` and a closed reason (agent-capacity, agent-page-invalid,
agent-revision-changed, workbench-changed, client-changed or metadata-changed). Existing Agent and
admission errors keep their owning schemas and diagnostics. Selected Client routing
errors keep the Client query contract. Verification covers priority and uncertainty,
primary/child membership, duplicate/mixed pages, stale acknowledgements, schema/text,
headless reads and real Windows two-Client observations. Panel process/title fields
use the terminal observation owner below; Client scroll and Workspace worktree observations
use the owners below. Workspace tokens use the separate metadata owner below.

## Workspace metadata reports and queries

Windows `workspace report-metadata <workspace> --source <id> --token <key=value>`
submits one normalized patch. Repeated `--token` / `--clear-token` flags use argv order;
optional `--seq` fences each source and `--ttl-ms` expires patched values. The
[Workspace metadata contract](windows-workspace-metadata.md) owns bounds, stale no-ops,
expiry, unknown-outcome receipts and the separate Server revision. Workspace list/get v1
includes `tokens` and root `metadataSource`, with bounded batches and a final revision
fence inside the original five-second deadline. Reads do not activate or navigate a Session.

## Workspace worktree observations

Windows Workspace list/get joins each canonical primary Directory to the existing
[read-only Windows Worktree query](windows-worktrees.md#source-based-discovery).
It returns nullable `worktree` and required `worktreeObservation`. An observed
summary contains `workerId`, `repositoryRoot`, `commonDirectory`, `repositoryName`,
`checkoutPath` and `linked`; the pair of Worker ID and common Git directory is the
repository grouping identity. The name is the final nonempty component of the
observed repository root, not the Workspace label or invoking CLI directory.

| Input or result                                                                           | Published observation                                     | Effects                               |
| ----------------------------------------------------------------------------------------- | --------------------------------------------------------- | ------------------------------------- |
| No primary Directory                                                                      | `no-directory`, null worktree                             | No Git request                        |
| Valid source discovery                                                                    | `observed`, exact query source and summary                | Read-only Worker Git query            |
| Git/Worker unavailable, unsupported, spawn/I/O/size failure                               | `unavailable`, request ID and typed reason, null worktree | No fallback, replay or trust override |
| Denied, missing/changed source, malformed result, transport loss, timeout or cancellation | Whole command fails, without partial success              | Existing query cleanup                |

Each request uses the exact Workspace and `trustRepository: false`. A successful
reply must agree with the original Session, Workbench revision, primary Directory path
and Worker, and include that Workspace at the source checkout. One admitted
connection, signal owner and original five-second total cover all rows, Agent/Client
joins and the final canonical/Client consistency checks. Queries never activate a
Session, change associations, navigate, launch a terminal or write Git configuration.
Git cleanup remains owned by the existing Worker read task. External filesystem
changes are not fenced: these are sequential live observations, not an atomic Git
snapshot or Herdr's cached membership. `unavailable` never means “not a repository.”

Workspace list/get use schema version 1. The catalog exposes the current documents and command paths. Other query versions retain their existing contracts. Verification
covers main and linked checkouts, Workspace directories inside a checkout, missing
directories, unavailable Git, mismatched source facts, total deadlines, safe text
and real Windows reads without topology or Client-view changes.

## Panel terminal observations

Windows `panel list/get/current` also returns nullable `cwd`, `cwdSource`, `foregroundCwd`,
`terminalTitle` and `terminalTitleStripped`, plus a required `terminalObservation`.
The observation is `unbound`, `unavailable` with `reason: terminal-unavailable`, or
`observed` with a validated frame and an observed/unavailable process sample. A stopped
terminal does not remove its Panel from the list. Only typed terminal/process service
unavailability becomes unavailable data; protocol, permission, transport, cancellation
and deadline errors fail the query without a partial result.

One read-only viewer per primary terminal supplies both observations. Frame facts retain
Host epoch, attachment, sequence, generation, revision, geometry, screen, scrollback count,
raw title and reported directory. Full cell data is discarded before the summary is retained.
The process sample uses the existing Windows Job read and its explicit root/Agent-chain/
ambiguous selection policy; it does not claim OS foreground authority. Each observed sample
retains its own source and the normal `panel process-info` result. Unavailable process sampling
does not erase successfully observed frame metadata.

`cwd` uses an absolute Windows path decoded from the frame's reported directory, falling back to the sampled root-process directory;
file URIs decode percent escapes once as strict UTF-8 (drive paths or UNC authority), without
query/fragment/control characters or filesystem access; invalid/non-Windows reports remain raw
in the frame evidence and do not become a filesystem directory.
`cwdSource` is `terminal-frame`, `root-process` or null. `foregroundCwd` uses the process selected
by the disclosed process policy and remains null when unavailable or ambiguous. No invoking-CLI
directory or launch-directory guess is substituted. Raw title is retained unchanged. The stripped
title removes an exact Windows `Administrator: ` prefix, surrounding whitespace, and at most one
leading whitespace-delimited Braille or known activity glyph (`·✢✳✶✻✽◐◓◑◒`); empty becomes null.
Text output escapes all terminal/process-controlled strings.

The existing five-second total covers all Panel observations, Agent joins, and final canonical/
Client checks. Viewers detach on every outcome, with admitted-session teardown as fallback.
There is no input, resize, focus, history-demand or Client acknowledgement effect. These independent
frame/process observations are not an atomic snapshot. A fresh viewer's live frame never claims
the selected Client's scroll offset: the Client viewport observation below supplies it.
Windows Panel list/get schemas are version 1 . The catalog exposes the current documents and command paths.

## Panel Client scroll queries

Windows Panel list/get/current returns nullable `scroll` and required `scrollObservation`.
Without a selected Client, the observation is `no-client`; a new Host viewer cannot establish
the user's scroll position. With an exact Client, bounded read-only viewport batches
return its installed live/history metrics or explicit unavailable/pending state, as
defined by [Client viewport observations](windows-client-control.md#client-viewport-observations).
The root Client source retains each operation ID. Requested Panel IDs, primary bindings,
canonical revision and the unchanged selected view must agree before publication.

Panel reads accept current Ready or Busy Clients, including history browsing. They
corroborate selection membership directly against the canonical snapshot; they do not
request unrelated active-Tab memory. Workspace/Tab queries keep their existing gates.
All observations share the original admitted connection and five-second total, followed
by the final canonical and Client revision checks. No query changes the viewport, focus,
selection, input lease or seen acknowledgement. Independent Host/process and Client
observations are not an atomic snapshot. Windows Panel list/get and retained viewport controls both use version 1, including
`orchbee.client.control`. The catalog exposes the current documents and command paths.

## Windows named Session selection

The global `--session <name>` / `--session=<name>` selects the local Windows Session for the entire
invocation, including `server start`, `server stop`, queries and `tui`. It does not change the
Agent `--agent-session` selector. Names are case-sensitive, contain 1–64 ASCII letters, digits,
`.` / `_` / `-`, and cannot be `.` or `..`. `default` selects the existing default Session.
The complete argument grammar is validated before selection; offline help, completion,
version and schema operations do not discover a runtime because a name was supplied.

Selection precedence is explicit `--session`, the managed terminal's `ORCHBEE_RUNTIME_SCOPE`,
`ORCHBEE_SESSION`, legacy `ORCHBEE_SITE`, then `default`. `ORCHBEE_RUNTIME_BASE` still selects the existing absolute
base. The Worker supplies the admitted base and scope to child terminals, so nested CLI
commands keep their owning Session even if a different name was inherited by the Server.
An explicit `--session default` overrides that inherited scope. Invalid selected environment
values fail discovery; they never silently select the default Session.

Named runtime leaves hash the current SID and exact name; names are never filesystem path
components. Activation registers a bounded private identity under its existing activation
lock before committing the startup marker. Read-only discovery validates that identity
without creating it. Checkpoint storage is already isolated by the admitted runtime path.
`session list [--format text|json]` lists default followed by exact named sessions in the
selected runtime base. It reads at most 256 sessions under one five-second deadline, creates
no runtime state and reports running, stopping, stopped, absent, starting, deleting, recoverable or recovery-required
from authenticated metadata and admitted lifecycle facts. Unknown/unsafe inventory fails explicitly.

Check and list text views lead with their conclusion and keep raw reason structures in JSON.
`config check` prints a verdict line, one aligned row per check with its path, locale or failure
reason, any settings warnings, the file in use and a `--format json` pointer. `agent doctor` prints
its readiness summary and next command first, then a provider table. `session list` has a heading
row and native paths, and explains a `not created` session. A closed `workspace close-status` shows
its members as closed. An empty `session list`, `command list` or `launch-profile list` prints one
line saying how to create the first entry; the launch-profile catalog revision is JSON-only.

`session attach <name> [--mouse on|off]` activates or reconnects the real TUI. `session stop
<name> --confirm` reuses `server stop`, including its timeout, detach, request-id and retry options
and its existing result schema. `session delete <name> --confirm` removes only that stopped
named session's retained checkpoint and owned runtime files; it refuses default, running or
uncertain state and never deletes Project/worktree/user directories. A name operand and explicit
`--session` must agree. Delete is idempotent when absent. A partial removal remains fenced by a
deletion intent and an explicit failure; repeat delete to finish, never start to repair it.
Deletion checks Ctrl+C before each filesystem phase and exits 130 on interruption. Completed removals are not rolled back; a pending intent remains resumable. The `interrupted` result reports whether an intent remains, including cancellation after cleanup completed.

List emits `orchbee.session.list` v1. `recoverable` means
a guarded observation found complete current registration, all exact identities gone and idle resources;
it carries `running=false` and `pid=null`, and does not promise a later start. Delete retains
`orchbee.session.delete` v1. The embedded schema catalog is v1.

The durable catalog preserves exact default/named identities and stopped receipts in LocalAppData.
With the same admitted runtime base, `session list` still discovers a stopped session after its
runtime leaf is removed; `session attach`/`server start` recreates its route and restores the checkpoint.
`server start` and TUI startup automatically recover a complete, proved-dead current Active generation,
including after runtime-leaf loss, while preserving the last checkpoint. The first stage refuses
live Host, unknown or incomplete identity and unreadable records without deleting data. No old process
is terminated and no clean-stop receipt is invented. Run `orchbee server diagnose` with the same
`--session` selection to inspect a refusal, or `orchbee --session recovery-work server start` to use another
name. A new activation publishes the current version 1 catalog before marker, Server or storage
publication. Stopped records admit fixed slots and exact owned corrupt originals. Other record
versions/shapes remain unreadable without migration. `session list` exposes an admitted named record
as `unreadable` with unknown running state; confirmed `session delete` can remove it after all existing
owner/storage guards pass. See the
[recovery contract](../../../crates/local-site-runtime/docs/windows-crash-recovery.md).
Pending deletion also survives loss of temporary metadata and remains resumable by the same delete
command. Listing remains read-only. The runtime base itself is not implicitly recreated.

Background and clean-stop checkpoints preserve layouts, Launch Profiles, sampled per-terminal cwd
and unique provider resume references. Startup selects the newest valid copy, retains corrupt bytes,
and warns when it falls back or opens an empty workspace. Live Agent state, terminal history and
screen contents are excluded. Live Host adoption remains separate work.

## Agent session names

`agent names` lists all live aliases in the running Session. `agent resolve <target>`
returns a canonical session ID or an exact terminal target, testing exact session ID existence before aliases. `agent rename
<target> <new-name>` assigns or atomically replaces a name; `agent rename <target>
--clear` releases it without stopping the session. Replace the target operand with
`--agent-session <id>`, `--name <alias>` or `--panel <id|@panel:0x<hex>>` for explicit selection; exactly one selector is
required. Names match `[a-z][a-z0-9_-]{0,31}` and are case-sensitive. They are unique,
sharing a 512-entry cap across canonical names, terminal names and pending launches, and released when the session ends, is removed, or the
Session runtime is disposed. Only admitted completed managed names can return after Session restart.

Windows terminal-backed names require a reported, detected or retained provider identity; an
ordinary unnamed shell does not qualify. Their target carries presentation ID, creation generation,
Host epoch and terminal ID. A first provider reference upgrades ownership; a changed provider or
known reference, proven release/exit, removal or physical replacement clears the name. Intermediate
report changes are reconciled at commit, so A → B → A cannot revive an old alias. Missing process
observations alone do not clear it. Hook arrival does not silently convert a terminal alias into a
canonical session alias.

Terminal-target resolve/list use version 1, rename/clear use version 1, and mixed-target name
collisions use failure version 1 with `conflictTarget`. Canonical receipts use the same version and target shape. Physical `agent get` includes
`terminal-found` in its inspection with its exact target, actual frame/process/primary-panel report fields and a separate management source/state read; this terminal detail now remains under v1 `inspection`.
Management is unmanaged or active with the restored provider; it does not assert current readiness.
It contains no invented
session ID. Child terminal reads never borrow the primary panel's report. Named `read`, `send-keys` and
`attach` route through the exact physical binding and existing terminal owners. Physical
`explain` returns v1 fresh screen diagnostics or v1 with active rule provenance; semantic `wait`/`prompt`, focus and `get --client`
use their physical observation/Client contracts without inventing canonical sessions;
completed managed names restore onto fresh bindings after enabled provider resume. Ordinary manual names
and pending reservations do not survive a cold restart. Same-provider reference changes and renaming
preserve restored management. Clearing its name removes only its matching owned persisted resume seed.

All three commands admit `--format text|json`. The embedded success schemas are
`orchbee.agent.names` version 1 (with separate pending reservations), and
`orchbee.agent.resolve` and `orchbee.agent.rename`, version 1;
post-admission failures use `orchbee.cli.failure.agent-names`, version 1. Presentation resolution and ambiguous-target failures use version 1; launch-pending failures use version 1; all outcomes use version 1. Correlated
semantic answers carry an `agent-names` source with the OBAP request ID. These four documents are included in schema catalog version 1, alongside the prompt contracts. Invalid names or conflicting selectors
fail with exit 2 before any runtime access. A collision is exit 3 and identifies the
incumbent session; it never overwrites that session's name.

Each command owns one five-second deadline spanning cohort validation, existing-only
Session discovery, admission, optional Panel resolution and the final OBAP exchange. Signals participate in transport
waits; the session closes before signal arbitration and output. A confirmed reply
remains authoritative if a signal races its completion. A submitted mutation without
a trustworthy matching reply returns outcome-unknown (exit 8), including cancellation,
and is never replayed. A known pre-submission cancellation is exit 130. No alias
cache, provider process, new Session, or canonical-session mutation is involved.

`get`, `explain`, and `wait` accept the same positional target or explicit
`--agent-session`/`--name`/`--panel` selector. Exact retained IDs take precedence over names; explicit
IDs and opaque tokens that cannot name aliases or Panels go directly to the read. Other targets first send
Resolve on the same admitted Client Connection. Resolution and reads share the original
deadline and signal owner, without a second admission or renewed budget. A wait bounds
initial resolution by `min(remaining_ms, 5000)` and pins the resolved ID before
polling; releasing, renaming or reusing an alias never retargets it. Success publishes
only the actual read's `agent-read` source. Resolution refusals use their own
`agent-names` source and failure schema, never the identity of an unexecuted read.
Windows process evidence covers real pipe admission, resolved reads, pinned polling,
the shared five-second deadline, explicit-ID reads, schema-valid failures,
lost/mismatched mutation replies and parse-before-effect.

Windows physical Agent Get v1 includes `semantic: {source, observation}` in the current
inspection shape. Its source recorded Session, Workbench/report revisions and request ID; the observation
contains the exact binding, effective report view, terminal-local sequence and an empty change list.
Primary and child presentations read their own report resources; primary metadata/report projections
agree with the semantic report revision. Text renders the effective report in the selected locale.
`clientView` is always present, null without `--client`; when selected it contains: selected Client/operation/view, report
revision, nullable sequence/seen/status and outer focus. Only matching full binding and semantic
sequence under a covering Server report revision corroborate seen. State remains idle/working/blocked/done/error/unknown independently; missing or stale acknowledgement leaves only seen unknown. No canonical Session, turn or
readiness is invented. `agent attention` routes physical targets through
the same Client owner and emits the v1 Client control receipt without marking them seen.

### Physical Agent wait and prompt

Windows terminal-backed names use the report owner's exact physical binding and semantic history.
The CLI holds one Client Connection through admission, every bounded read and private interruptible
backoff, dropping it before signal arbitration. Each round refreshes Workbench membership, reads
contiguous changes and revalidates the original selector and provider. Replacement, history loss,
future cursors, transport loss and provider ABA fail explicitly; there is no reconnect or polling
fallback after lost evidence. Total deadlines never renew. No late observation produces success.

Ordinary waits may match a retained transient; current observation remains separately reported. `--after-revision` on this target means its report-source revision, not a canonical Agent revision. A future fence refuses against the first observation. History consumed after that checkpoint is necessarily newer than its report revision. Working, idle, blocked, unknown and the needs-attention alias use report facts. Explicit done/error reports support succeeded/failed; the existing done wait alias matches either settled outcome. Freshness, cancellation and interruption conditions still require canonical observations. `--client` optionally pins one Client and exposes its separately corroborated acknowledgement; a done-only request no longer selects a Client implicitly. Missing/stale acknowledgement cannot prevent a state match, while an explicitly selected Client loss or read failure ends the observation.

Prompt preflight rejects blocked state, verifies a known provider against the actual foreground
process chain and rereads the exact semantic/physical target before submitting. A known live provider
with unknown report state may receive input, matching fixed Herdr's prompt admission. Existing send
owns paste/Enter, the Windows Codex boundary, input cleanup and delivery evidence. It is invoked once.
For prompt waits, the pre-send sequence checkpoints the activity gate. Prior working or a later
working/blocked change opens it within five seconds (or the shorter total); message/reference changes
and preexisting idle never do. A transient gate-opening working/blocked may match those conditions;
resting conditions require the current state after activity. Lost history fails without resending.

Wait v1 returns `terminal-matched` with physical observation, optional matched change, conditions and
accounting. Prompt v1 returns a terminal target, original submission and optional semantic wait.
Selected-Client matches use Wait/Prompt v1 with required `clientView` on the match; Prompt v1 requires
a completed wait. Preflight checks the selected Client before input. All reads share the existing
five-second evaluation and absolute total deadline, without acknowledging or resending.
Agent read failure v1 and prompt failure v1 carry physical failure details.
Cancellation or failure after accepted input retains its submission; partial/unknown delivery remains
the original send result. Tests cover state gates, quiet updates, transient versus current matching,
provider/physical changes, fences, bounded timeouts, receipt preservation and real Windows input.
Generated tests cover 32 seeds and 6,400 status pages, shrinking failures to replayable traces;
this projection checks retained status and activity gating, excluding release events and Client attention.
The scope follows fixed Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` wait and prompt handlers;
Client attention, authenticated-provider behavior and final full acceptance remain separate evidence.

## Agent terminal operations

`agent read <target>` and `agent send-keys <target> <key>...` accept the same
Session ID, runtime name or Panel selectors as `agent get`, including explicit `--agent-session`,
`--name`, `--panel` and `--terminal`. Read admits `--source` and `--lines <n>`
with the same bounds and meaning as `terminal read`. Send admits at most 64 semantic
keys or modifier combinations and validates the entire sequence before any effects.

On Windows, these commands use the shared projection resolver described in
[Windows unified Agent projection](#windows-unified-agent-projection). It also
accepts `@agent:<kind>` and complete-list `@agent:<n>` selectors. Bare names keep
Server alias precedence; physical targets retain and revalidate their full binding.
The canonical-session flow below describes that target path and other platforms.

The CLI composes existing owners: one existing-only Client Connection reads a Workbench
snapshot, optionally resolves an alias, and gets the canonical Agent session. Only
an active session's navigable, live presentation attachments may select a terminal.
The join compares full presentation IDs against bound terminals in the snapshot;
a Panel ID is never treated as a presentation ID. An explicit or fallback Panel restricts the join to its primary presentation.
Zero matches is not-found, multiple matches is ambiguous, and neither case attaches a viewer or submits input.
One five-second deadline covers all exchanges. Resolution is pinned for the operation;
the Server still authorizes the current terminal binding at input admission.

| State                      | Allowed next operation                        | Failure effect                           |
| -------------------------- | --------------------------------------------- | ---------------------------------------- |
| Parsed                     | Snapshot admission/read                       | No terminal effects                      |
| Snapshot held              | Resolve and canonical Get on the same session | No terminal effects                      |
| Exactly one bound terminal | Existing terminal read or send exchange       | Existing viewer/input cleanup applies    |
| Reply or failure           | Drop session, project output                  | No replay; uncertain send remains exit 8 |

Success uses the existing `orchbee.terminal.read`, `orchbee.terminal.history`, or
`orchbee.terminal.send` schema and its actual terminal source. Agent resolution/read
failures retain their own correlated source and failure schema. No provider command,
Agent lifecycle transition, new Session, alias cache, input ownership, or per-turn causal
promise is introduced. Signals retain the existing terminal operation behavior; this
composition does not claim a new cancellable viewer or input transport. Evidence must
cover distinct presentation/Panel IDs, inactive and non-navigable attachments, ambiguous
bindings, parse-before-effect, a single admitted session, and non-replayed send failures.

The embedded send schema validates the implemented sequence receipt: payload kind,
record count, confirmed prefix, stopped index, encoded byte count, partial acceptance,
and cleanup status. Generic ambiguity failures include Panel and terminal candidates.
These repair stale schema projections; wire and receipt versions remain unchanged.

## Agent prompt submission

`agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms>]`
uses the same Session/name/Panel selectors as other Agent commands. Windows also accepts
`@agent:<kind>` and complete-list `@agent:<n>` through the shared projection resolver above.
Text is non-empty UTF-8,
bounded by the 256 KiB paste limit and the command's 64 KiB argv envelope. `--until`
and `--timeout` require `--wait`; Windows wait defaults to idle/done/blocked without a total
timeout, and explicit finite timeouts are at most one hour. Without waiting, the budget is five seconds.

One existing-only Client Connection owns snapshot lookup, one resolved Agent identity,
a canonical preflight Get, one paste-plus-Enter send, and any subsequent Gets. Preflight
requires a live, fresh session with a known status and exactly one navigable terminal.
Pending requests/needs-attention refuse with `agent-blocked` before any input. This is
a sampled preflight, not an atomic lock on future provider state or foreground ownership.
The Server still authorizes the terminal binding when admitting input.

| State          | Transition and evidence                                                                                             | Retained effect                             |
| -------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| Parsed         | Admit and resolve under min(total deadline, five seconds)                                                           | None                                        |
| Preflight      | Fresh, live, known, unblocked and uniquely bound                                                                    | None                                        |
| Send           | One ordered paste and Enter sequence, no replay                                                                     | Exact terminal receipt                      |
| Activity gate  | Already working at preflight, or newer observed working/attention/turn outcome within five seconds after acceptance | Accepted receipt survives every later error |
| Condition wait | Sample fresh status/freshness conditions on the pinned session under the original total deadline                    | No turn-causality claim                     |
| Done           | Drop session, arbitrate cancellation, emit once                                                                     | No input replay or provider cancellation    |

Waiting from a non-working state first requires evidence of activity. A newer turn
outcome also qualifies when a short working interval was missed between polls. Old idle
or an unchanged completed outcome cannot satisfy this gate. Gate expiry is
`prompt-stalled`; an earlier caller deadline is `wait-deadline`. Subsequent read errors,
end, timeout or cancellation do not erase the confirmed submission. An already-working
Agent may complete its existing turn; a matching result does not identify the prompt's
own turn. Polling can miss transitions and does not claim Herdr event replay.

On Windows, canonical provider `codex` uses one ordered paste, Right, Enter sequence. Right is a non-character boundary that flushes Codex's paste-burst buffer and clears its Enter suppression window. It neither embeds escape bytes in prompt text nor guesses delivery speed with a fixed delay. Other providers retain paste, Enter; ordinary `terminal send --paste --enter` adds no boundary key. Receipts report the actual three or two records. Stopping before Right or Enter reports only the accepted prefix, never retries the suffix and never starts observation. This boundary does not establish an empty composer, foreground ownership or provider readiness.

Schema The catalog exposes the current documents and command paths.

`orchbee.agent.prompt` returns the session ID, the full terminal submission receipt,
and optional matched observation/wait facts. Refused/partial/unknown send outcomes retain
the existing terminal send/failure contract and never start a wait. Prompt preflight and
post-submission failures use `orchbee.cli.failure.agent-prompt`; post-submission failures
include the receipt so callers cannot mistake a wait failure for unsubmitted input.
A confirmed send without `--wait` remains authoritative if cancellation races its reply.
During a wait, cancellation is exit 130 with the accepted receipt. Ambiguous submitted
input stays exit 8 even under cancellation. All sessions drop before signal finalization
and output. No prompt text is included in receipts.

Evidence covers blocked/not-ready refusals without terminal effects, exact literal paste
plus Enter, one admission and no replay, active/idle activity gates, newer completed
outcomes, pinned-session reads, total/activity deadlines, partial and unknown sends,
cancellation settlement, and schema validation. Native canonical observations remain the
only Agent status authority; this command does not add screen inference or a provider API.

## Agent wait policy

Windows `agent wait` and `agent prompt --wait` default to no total timeout and the ordered conditions `idle`, `done`, `blocked`. `--forever` remains an explicit synonym and conflicts with `--timeout`; prompt requires `--wait`. Finite timeouts remain 1–3600000 ms. Admission, resolution and each read remain bounded to five seconds; interruption stops the wait and never reconnects. Existing Linux polling defaults are unchanged. Prompt retains its five-second non-working activity gate.

Windows wait establishes an authenticated changes checkpoint before its first canonical Get, then consumes contiguous retained pages before subsequent Gets. The single admitted session, optional monotonic total deadline and cancellation owner cover all steps. Each evaluation drains at most 512 changes, 100 per page, retaining at most one matching change. No cursor reset or foreign boot is silently recovered: lost history is an explicit rejection. A partial/failed read cannot become a matching observation.

Wait version 1 on Windows adds nullable `matchedChange`. Null means the current sample matched; otherwise the historical change is the matching evidence, while `revision` and `session` remain the separate latest Get sample. The historical record never overwrites current status. `source.requestId` identifies the matching Get or changes read. Requested condition order is preserved. `--after-revision` remains a global revision fence, not turn causality. Linux wait and prompt success retain version 1. Finite/infinite failures keep their existing version and null timeout distinction.

`blocked` matches fresh needs-attention; `done` currently matches fresh succeeded/failed/canceled/interrupted. Client-relative unseen enrichment is a separate dependent consumer and is not implied by the change feed. Evidence covers transient states, unrelated sessions, ordered pages, reset refusal, retained identity, deadline and cancellation cleanup.

For canonical session targets, Windows `agent wait --client <id>` pins one exact Client as well as the resolved Agent. Every condition uses canonical current/change-page matching independently of Client acknowledgement. A missing/stale Client version exposes null seen without suppressing idle or an explicit outcome. Historical canonical events never invent a historical Client acknowledgement. Loss of the selected Client ends the wait rather than choosing another.

Selected-Client wait uses v1 with a required `client` projection and existing nullable `matchedChange`; unselected Windows wait remains v1. Each Client read, cursor proof and canonical sample shares one five-second evaluation bound within the original total deadline. Reads remain valid under Busy without acknowledging or navigating. Finite expiry and cancellation preserve the existing wait failure/finalization contract. Explicit canonical status conditions may return historical evidence even when current Client status differs.

Selected-Client composition keeps missing/stale Client selection failures in `orchbee.cli.failure.client-control` v1. Other Client read failures use that envelope v1 with required `stage: client-agent-read`; their kind distinguishes denial, rejection, lost transport, invalid reply, deadline and cancellation. Existing Agent and admission failures keep their original contracts.

## Inputs and trust boundary

- `argv`: bounded hostile input. Total UTF-8 bytes across arguments may not exceed 64 KiB
  (65,536). Any non-UTF-8 argument, or any overflow, is an `invalid-usage` failure with empty
  stdout even when another argument spells `--format=json`. No stdin or file input is read
  implicitly — the only exceptions are the explicitly declared `terminal send --file`/`--stdin`
  sources under [Send input sources](#send-input-sources), materialized once before any runtime
  effect.
- Environment: `ORCHBEE_RUNTIME_BASE` (optional; defaults to `/tmp`) selects the runtime base;
  `LC_ALL`/`LC_MESSAGES`/`LANG` select the diagnostic locale. `agent doctor` additionally reads
  `ORCHBEE_AGENT_OBSERVATION_SOCKET` (the observation-endpoint override its endpoint fact
  reports verbatim), the inherited `PATH` for its bounded executable scan, and `HOME` for the
  provider target prefixes on Linux. Windows uses the native user profile and an optional
  non-empty `CODEX_HOME` for Codex configuration. `panel current` in caller mode additionally reads
  `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID` as described below. Nothing else is consulted.
- Filesystem: the resolved runtime base, the Session scope directory
  `orchbee-native-<euid>`, its `client.sock`/`worker.sock`, the activation marker, and the
  current executable's sibling `cohort.json` plus four artifacts are observed but never written.
  The manifest also declares the `agent-observation-adapters` sidecar digest — startup never
  opens or traverses that directory. All directory traversal uses the existing no-follow,
  owner/mode-checked rendezvous owner
  (`ModeCheckedLocalSiteRendezvous::validate_mode_checked`, `ExistingOnly` policy only).
  `agent doctor` inspects a disjoint declared set — the ten provider hook targets under the
  resolved home, the supplied `--asset-root` paths, the recorded roots those provider files
  themselves name, the observation socket path's final component, and each PATH directory's
  candidate executables — all under its own per-file/count/aggregate byte and item caps with
  no-follow `lstat` admission. It never enumerates ambient user directories.
- Socket peer: `client.sock` may belong to a real Server, a zombie socket, or a scripted peer.
  The CLI trusts the typed transport and protocol layers (OBLC admission, OBWP correlation and
  contract validation) and never re-derives verdicts from `Display` text, `ErrorKind` guesses, or
  byte counts.
- Snapshot data: user-controlled labels and directory paths are arbitrary Unicode and are treated
  as hostile text for terminal output (escaping below); JSON output preserves them losslessly.

## Public grammar

Option scan runs left to right over tokens before `--`; `--` terminates option recognition only.

- `--format text|json` or `--format=text|json`: at most one selection, any position before `--`.
  `--format` with a missing, empty, or unknown value, or a repeated selection, is a format
  failure: `invalid-usage`, exit 2, empty stdout. The complete pre-`--` format envelope is
  scanned before ordinary grammar validation: a format defect anywhere in that envelope answers
  with empty stdout, and one valid unique selection shapes every later grammar failure's
  envelope regardless of where it sits.
- `--project <target>`, `--workspace <target>`, `--tab <target>`: named value filters and
  context operands, single use each. The next token is consumed as the value even when it
  begins with a dash; a bare `--` is never consumed as a value (missing value →
  `invalid-usage`). A consumed filter value is not a format selector. Each flag is valid only
  on a command that declares it (`--project` on `workspace list`/`workspace create`/
  `worktree list`/`workspace close-status`/`workspace close-resume`,
  `--workspace` on `tab list`/`tab create`/`workspace close-status`/`workspace close-resume`,
  `--tab` on `panel list`); anywhere else,
  including `get` commands, is `invalid-usage`. Filters are never silently dropped: a filter
  flag on a command that does not declare it is `invalid-usage` before any runtime access, so
  `workspace list` rejects `--workspace`, `--tab`, and `--standalone`. On `workspace
close-status` and `workspace close-resume` both flags are strict stable IDs instead of
  targets: a complete nonzero 32-lowercase-hex Workbench identifier only — never a label or
  selector — because the retained close record outlives the Workspace's active-projection
  membership and must be re-proved by stable identity.
- `--directory <directory-id>`: single-use value flag for the registered-directory form of `worktree list`, valid nowhere else. Windows also has a source-based form without this flag; see [Windows Worktrees](windows-worktrees.md#source-based-discovery). The value is a `WorkbenchDirectoryId` only — a strict nonzero 32-lowercase-hex
  identifier, never a filesystem path, label, or prefix. A missing, malformed, uppercase,
  zero-valued, or wrong-width value is `invalid-usage` before any runtime access; the CLI
  performs no directory-membership precheck — membership is the running Session's authority, and
  a well-formed but absent directory answers `not-found` with `{resource:"directory"}` under
  the worktree failure envelope.
- `--label <label>`: single-use value flag, required on `project create`, `project rename`, non-Windows `workspace create`, `workspace rename` and `tab rename`. Windows `tab create` and `panel split` allow omission, using the preflight Tab/Panel count plus one. Explicit values are nonempty protocol-bounded Workbench labels; empty or oversized values are `invalid-usage`.
- `--path <path>`: single-use value flag. On `workspace accept-directory` it is required and accepts a `WorkbenchDirectoryPath`: a literal absolute local-platform path
  (POSIX `/` or an absolute Windows drive path) of at most 4,096 UTF-8 bytes without NUL, without `~` or variable expansion. A missing, relative, NUL-bearing or overlong value is `invalid-usage`.
  Windows Worktree Create/Open instead admit literal absolute, relative and leading-`~` operands under the [Worktree path policy](windows-worktrees.md#commands-and-defaults); the final executing CLI resolves them after complete grammar and machine routing, before configuration or runtime access. Neither form creates, repairs or probes the directory; filesystem facts remain the Server's authority after submission.
- `--direction left|right|up|down` is the cardinal vocabulary. `tab move` uses
  `--order previous|next|first|last`, exclusive with `--insert-index`. `terminal copy-search`
  uses `--search-direction forward|backward`. Unknown values fail before runtime access.
- `--cols <n>`, `--rows <n>`, `--cell-width <n>`, `--cell-height <n>`: the terminal
  geometry group on `tab create` and `panel split`, each taking one decimal `u16`. The
  group is all-or-none: with all four absent the command requests the documented default
  (80x24 logical grid, paired-zero unknown cell pixels); supplying any of the four requires
  all four. The combined grid cells and pixel extents must satisfy the
  protocol `WorkbenchTerminalGeometry` bounds; a partial, repeated, non-numeric, or
  out-of-range set is `invalid-usage` before any runtime access.
- `--exec <path>` / `--arg <value>`: the ephemeral launch selection, admitted only on
  `tab create`, `panel split`, and `launch-profile create` (required there, optional on
  the launch-bearing mutations). `--exec` is single-use and takes a nonempty absolute
  UTF-8 executable path without NUL. `--arg` is repeatable and each value is appended
  verbatim, in order, as `argv[1..]` — empty and dash-leading values are consumed as
  values under the standard value rule, and `argv[0]` is synthesized from the
  executable path. At most 128 `--arg` values and at most 30,720 UTF-8 bytes across the
  executable plus all argument values; `--arg` without `--exec`, a repeated `--exec`,
  or a violated bound is `invalid-usage` before any runtime access. Omitting `--exec`
  keeps the resolved default shell; a selection is part of request identity and is
  never persisted as a profile.
- `--name <name>`: single-use value flag, required on `launch-profile create` and valid
  nowhere else. The value is a `LaunchProfileName` — a nonempty UTF-8 literal of at
  most 512 bytes; uniqueness is not required (the minted profile ID, not the name,
  identifies the entry). A missing, empty, or overlong value is `invalid-usage`.
- `--profile <selector>`: single-use value flag, admitted only on `tab create` and
  `panel split`, mutually exclusive with the explicit `--exec`/`--arg` selection —
  either combination is `invalid-usage` naming the conflicting options. The value is a
  `LaunchProfileSelector`: a nonempty UTF-8 literal of at most 512 bytes passed
  verbatim to the Server, which resolves it under the catalog's ID-first/name-cardinality
  rule — the CLI never reads the catalog or guesses a match. A resolved profile supplies
  the launch for that one request; it is not persisted anywhere else.
- `--request <close-id>`: single-use value flag, required on `workspace close-resume` and
  valid nowhere else. The value is the close operation's own `WorkbenchRequestId` — a strict
  nonzero 32-lowercase-hex identifier, the operation identity the Server re-verifies, never
  the resume request's fresh identity. A missing, malformed, uppercase, or zero-valued value
  is `invalid-usage` before any runtime access.
- `--generation <n>`: single-use value flag, required on `workspace close-resume` and valid
  nowhere else. The value is the explicit attempt-generation fence: a nonzero decimal `u64`
  without a leading zero — the protocol's `WorkbenchTabCloseAttemptGeneration` spelling. A
  stale-but-valid generation is a semantic request the Server answers `stale`, never a
  grammar concern; a missing, empty, zero, non-numeric, or overflowing value is
  `invalid-usage` before any runtime access.
- `--confirm`: valueless boolean, required on `tab close`, `panel close`,
  `workspace close`, and `workspace close-resume`; absent there or
  present on any other command is `invalid-usage`. A `tab close` or `workspace close` with
  `--dry-run` needs no `--confirm`.
- `--match <text>`: single-use value flag on `terminal wait-output`, one of the two search
  inputs — exactly one of `--match` or `--regex` is required. A nonempty UTF-8 literal up to
  4096 bytes, matched case-sensitively. A missing, empty, non-UTF-8, or oversized value —
  or supplying both search flags — is `invalid-usage`.
- `--regex <pattern>`: single-use value flag, the alternate `terminal wait-output` search
  input; a nonempty UTF-8 regular expression of at most 1024 bytes compiled once before
  any runtime access under a 1 MiB program-size limit and a 1 MiB lazy-DFA cache limit.
  A missing, empty, oversized, or non-compiling value — or coexistence with `--match` — is
  `invalid-usage`.
- `--timeout <ms>`: single-use value flag, optional on `terminal wait-output` (defaults to `30000` on every platform) and on `agent wait` (Windows defaults to no total timeout, Linux to `600000`); one decimal `u32` in `1..=3600000`.
  A repeated, non-numeric, or out-of-range value is `invalid-usage`.

- `--target-panel <target>`: optional single-use value flag with `panel move --into-tab`;
  admits a complete Panel ID or exact compact handle and resolves inside the destination Tab.
- `--source visible|recent|recent-unwrapped|detection` and `--lines <n>` select output on
  terminal/Agent reads and terminal waits. The default is recent, 80 physical rows; line counts
  are unsigned 32-bit, clamp to 1000 and permit zero. Geometry commands use `--rows` as a dimension.
- `--text <utf8>`, `--key <name>`, `--file <path>`, `--stdin`: the four mutually
  exclusive `terminal send` input sources, valid on no other
  command. `--text` and `--file` consume the next token as their value under the
  standard value rule (a dash-leading token is consumed, a bare `--` is not);
  `--key` takes one closed key name per occurrence and can repeat up to 64 times; the other sources are single-use. `--stdin` is valueless. `--text` must be a
  nonempty UTF-8 literal whose byte length plus the optional folded CR stays
  within the 4,096-byte record bound; `--file` stores its operand untouched —
  the parser performs no filesystem validation, and every path, type, and
  content check is the materializer's job under [Send input
  sources](#send-input-sources). `--enter` is the text-bearing modifier: legal
  on `--text`, `--file`, and `--stdin`, never on `--key` or alone. `--paste` is valid only with `--file` or `--stdin`; it produces a paste record and an optional trailing Enter record. Zero source kinds
  or more than one is `invalid-usage`, with conflicts named in the fixed order
  `--text`, `--key`, `--file`, `--stdin` regardless of argv order.
- `--mouse off|on`: single-use value flag on explicit `tui`, `terminal attach` and
  `agent attach` routes; host pointer capture defaults on. The attach interfaces use
  the [Windows stream contract](windows-terminal-streams.md), including interactive
  output with either global format. Both declared spellings
  (`--mouse on` and `--mouse=on`) carry the same closed `off|on` vocabulary; a missing
  or unknown value, a repeat under either spelling, or the flag on any other command
  route, or the bare invocation, is `invalid-usage`.
- `--include-ended`: valueless boolean flag, single use, admitted only on `agent list`;
  includes sessions whose lifecycle is `ended` in the requested page.
- `--limit <n>`: single-use value flag, admitted only on `agent list`; the page size as
  one decimal `u64` in `1..=100`, defaulting to the wire contract's 50. A repeated,
  non-numeric, or out-of-range value is `invalid-usage`.
- `--cursor <c>`: single-use value flag, admitted only on `agent list`. Windows uses
  a structured `u1.<base64url>` cursor capped at 1024 characters and bound to the Site,
  source revisions, filters, Client choice and page position/fingerprint. Malformed cursors
  fail as invalid input before live source reads. Determining whether a cursor is stale requires reading current sources, so stale structured
  cursors are rejected only after those reads. Other platforms retain their canonical
  Server cursor contract.
- `--until <condition>`: repeatable value flag, admitted on `agent wait` and `agent prompt --wait`; one
  condition name per occurrence drawn from the fourteen-name vocabulary
  `blocked`, `done`, `needs-attention`, `failed`, `succeeded`, `canceled`, `interrupted`, `working`,
  `idle`, `unknown`, `ended`, `fresh`, `stale`, `disconnected`. Repeated values
  stay in requested order for matching and the emitted echo; a duplicate value or
  an unknown name is `invalid-usage`. Absent `--until` entirely, the effective set
  on Windows is `idle`, `done`, `blocked`; Linux retains `needs-attention`, `failed`,
  `succeeded`, `canceled`, `interrupted`, `idle`, `ended`, each in that order.
- `--after-revision <n>`: single-use value flag, admitted only on `agent wait`;
  the optional global-revision fence as one decimal `u64` in the safe-integer
  range `0..=9007199254740991`. A repeated, non-numeric, or out-of-range value is
  `invalid-usage`.
- `--provider <id>`: repeatable value flag, admitted only on `agent doctor`; each
  value is one canonical provider id (`claude-code`, `codex`, `gemini`,
  `github-copilot`, `grok`, `cline`, `opencode`, `pi`, `omp`, `qwen-code`), selected
  in flag order and deduplicated under the fixed canonical row order — an unknown id
  is `invalid-usage` before any runtime access. No flag selects all ten.
- `--asset-root <path>`: repeatable value flag, admitted only on `agent doctor`;
  each value is a nonempty absolute NUL-free path declared as a capsule candidate
  root, kept in flag order. Relative spellings, `~`/`$` forms, empty values, and a
  fifth value are `invalid-usage` before any runtime access — at most four supplied
  roots are admitted and none is ever expanded or guessed.
- `--provider <id>`: single-use value flag, admitted only on the `agent integration`
  commands; one of the closed supported-provider ids (ten members, reported by
  `agent integration list`). Required on `explain`/`enable`/`disable`, an optional
  filter on `list`. Any other value is `invalid-usage`.
- `--adapter-source <abs>`: single-use value flag, admitted only on the
  `agent integration` commands; a literal absolute path to a development/test
  adapter payload that replaces the packaged cohort source for that invocation.
  A relative path or a repeat is `invalid-usage`.
- `-v`, `-V`, `--version`: equivalent version options, legal only as the sole root invocation;
  each prints the same `orchbee <version>` line and exits `0` offline. Combining them or adding
  a command, help flag or output format is usage. Option values and tokens after `--` stay data.
- `-h`/`--help`: one flag anywhere before `--`; selects an exact family or the longest
  recognized command path, ignoring operands and business requirements without effects.
- `--` positionals: tokens after `--` are positional only, so `orchbee -- status` queries
  status and `orchbee project get -- --help` resolves the literal label `--help`.

Command paths (fixed words, no abbreviations or aliases):

```
orchbee                         # bare → TUI (unchanged)
orchbee tui [--mouse off|on]    # TUI
orchbee -v|-V|--version         # version line
orchbee -h|--help | orchbee help   # root help
orchbee status
orchbee status server | status client
orchbee project list | project get <target>
orchbee project create --label <label>
orchbee project rename <target> --label <label>
orchbee workspace list [--project <target>] | workspace get <target>
orchbee workspace create --project <target> --label <label>
orchbee workspace rename <target> --label <label>
orchbee workspace accept-directory <target> --path <path>
orchbee workspace close <target> (--confirm [--detach | --timeout <ms>] | --dry-run)
orchbee workspace close-status --workspace <id>
orchbee workspace close-resume --workspace <id> --request <close-id> --generation <n> --confirm
orchbee worktree list --directory <directory-id>
orchbee tab list [--workspace <target>] | tab get <target>
orchbee tab create [--workspace <target>] [--label <label>] [<geometry>] [--exec <path> [--arg <value>]... | --profile <selector>] [--focus | --no-focus] [--client <id>] [--timeout <ms>]
orchbee tab rename <target> --label <label>
orchbee tab move <target> (--order previous|next|first|last | --insert-index N)
orchbee tab close <target> (--confirm [--detach | --timeout <ms>] | --dry-run)
orchbee panel list [--tab <target> | --workspace <target>] | panel get <target>
orchbee panel rename <target> (--label <text> | --clear)
orchbee panel split [<target> | --current] --direction left|right|up|down [--label <label>] [--ratio <number>] [<geometry>] [--exec <path> [--arg <value>]... | --profile <selector>] [--focus | --no-focus] [--client <id>] [--timeout <ms>]
orchbee panel close <target> --confirm
orchbee panel move <target> --into-tab <target> [--target-panel <target>] --direction left|right|up|down
orchbee terminal list [--workspace <target> | --tab <target> | --panel <target>] | terminal get <target> | terminal read <target> [--source visible|recent|recent-unwrapped|detection] [--lines <n>]
orchbee terminal wait-output <target> (--match <text> | --regex <pattern>) [--source <source>] [--lines <n>] [--forever] [--timeout <1-3600000>]
orchbee terminal send <target> (--text <utf8> | --file <path> | --stdin) [--enter] | --key <name>
orchbee launch-profile list | launch-profile get <selector>
orchbee launch-profile create --name <name> --exec <path> [--arg <value>]...
orchbee launch-profile delete <selector>
orchbee agent list [--include-ended] [--limit <1-100>] [--cursor <c>]
orchbee agent get <target> | agent explain <target>
orchbee agent read <target> [--source visible|recent|recent-unwrapped|detection] [--lines <n>]
orchbee agent send-keys <target> <key>...
orchbee agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms>] [--forever]
orchbee agent wait <target> [--until <condition>]... [--after-revision <0-9007199254740991>] [--timeout <1-3600000>] [--forever]
orchbee agent doctor [--provider <id>]... [--asset-root <path>]...
orchbee agent integration list [--provider <id>]
orchbee agent integration explain --provider <id>
orchbee agent integration enable --provider <id> [--adapter-source <abs>]
orchbee agent integration disable --provider <id> [--adapter-source <abs>]
orchbee client list | client get <client-id> | client control-status <operation-id>
orchbee workspace focus <target> [--client <id>]
orchbee tab focus <target> [--client <id>]
orchbee panel focus [<target> | --current] [--client <id>]
orchbee panel zoom [<target> | --current] [--toggle | --on | --off] [--client <id>]
orchbee api snapshot
orchbee api schema [<schema-id>] [--output <path>]
orchbee server start
orchbee server stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>]
orchbee server stop-status <request-id>
orchbee server diagnose
orchbee config check
orchbee completion bash|zsh|fish|powershell|elvish
orchbee --default-config  # Windows
orchbee --skill
orchbee help <recognized path>
orchbee <recognized path> -h|--help
```

Help topics include the root, every build-admitted family and complete command path including
`tui`, and nested families such as `agent integration`, `terminal session`, and Windows `panel graphics`.
`help` takes only a recognized path (no filters, no format); `help -h` and `help --help` show root
help, and `help <path> -h|--help` preserves that exact path and option scope.
A single `-h` or `--help` before `--` is the same request on the same path:
`orchbee --help`, `project --help`, and `project get --help` emit exactly what `help`,
`help project`, and `help project get` emit. Flag help also accepts supplied operands and
recognized options (`project get x --format json --help` describes `project get` as text).
It never applies those options or reads runtime state. Unknown options, missing values,
duplicate options and unknown family verbs remain usage failures. Unlisted
targets such as `help help`, `help version`, `help --version` are unknown paths → usage. Help,
`-v`/`-V`/`--version`, `--skill`, `completion`, `api schema` without `--output`, and Windows `--default-config` never touch platform, runtime, cohort, or Session state. Explicit Windows schema export touches only the caller-selected file destination and its owned sibling temporary.
Help has three information levels, in both `en-US` and `zh-CN`:

- Root help introduces Orchbee, shows a small set of common examples, groups each admitted
  top-level command once, and explains global options and how to descend.
- Family help describes that resource and lists each direct child with its purpose. Nested
  families such as `agent view` and `panel graphics` have their own directories. A Synopsis
  section repeats each direct child command's own usage line, without the leading `orchbee` and
  the shared `--format` choice, so it follows the grammar; full option descriptions stay on the
  child page. `completion` lists its supported shells. Every family and command
  page displays `-h, --help`; the `server` directory includes `stop` and `stop-status`, with
  `--confirm` identified on `stop`.
- Command help contains complete syntax, positional arguments, admitted options, an example,
  and the command's defaults, effects and constraints. `status` also links its child commands.

The help-path catalog in `src/invocation/help.rs` drives both path admission and directories.
Business option membership uses the completion command table; root-mode options follow their
existing grammar. Localized syntax and constraints are
separate fields, and one pure renderer owns spacing and wrapping. Output is unstyled UTF-8
with a trailing newline, aligned by display cells and wrapped without splitting grapheme
clusters at 78 cells. Long labels use their own row. Examples identify their Shell, keep quoted operands intact, and use PowerShell or POSIX Shell continuation when a command spans rows. This width is deterministic in terminals
and redirected output: help does not query a terminal, inspect `COLUMNS`, or read configuration
or runtime state. Hosts narrower than 78 columns may wrap the text further themselves.
Tests cover both locales, both help flags at every position of every admitted path, completion/help
command agreement, directory coverage, 40/78/120-cell renderer inputs, and parsing illustrative
examples without executing their effects. `cli_windows_site_stop` additionally exercises the real
binary's version aliases, nested help and missing stop confirmation with an unusable runtime base;
the existing stop receipt witnesses retain the lifecycle safety checks.

On Windows, `--default-config` must be the sole root argument and emits canonical UTF-8 TOML from the build without reading user configuration or runtime-selection environment.
`--skill` must be the sole root argument and emits the localized, embedded CLI usage guide.
PowerShell completion is a static grammar projection: it never executes user expressions or
queries a Session; loading it is explicit and does not modify a shell profile.
PowerShell 7 invokes it for a bare `-` and offers the root `-v`/`-V` aliases. Windows PowerShell
5.1 does not call native completers for that unfinished token. The Windows witness checks every
shared grammar case against the unchanged callback in both engines, checks the real completion
pipeline, and explicitly records the two bare-dash short-alias cases as engine non-invocation
boundaries in 5.1. Version argument parsing remains available in both shells.

Elvish completion uses the same Rust-owned JSON grammar as PowerShell. Its callback receives
literal words (command first, unfinished word last) from the interactive editor. Per invocation,
the pure walker records completed positionals, seen flags, literal values and a pending value
owner. `--` and terminal options stop completion; invalid scope, duplicates or conflicts leave
no viable command. Viable paths determine the next command word, unused compatible flags,
operand budget and closed enum values; a joined value keeps its `--flag=` prefix. Freeform
values have no candidates. The callback emits each candidate once, leaving matching and quoting
to Elvish. It evaluates no supplied word and starts no process. Registration covers `orchbee`
and `orchbee.exe` only, without changing profiles or other completers. The generated script is
bounded by the shared 8 MiB output limit. A separate fixed-output test keeps every completion
script and help body below 512 KiB (16 times under the runtime limit) to flag unexpected grammar growth. Load explicitly in interactive Elvish 0.21 with
`eval (orchbee completion elvish | slurp)`.

The shared completion fixtures check accepted and dead prefixes, help depth, target conflicts,
enum ownership and joined spelling. The Elvish interpreter test is explicitly ignored by the
portable gate and must be run with `ORCHBEE_TEST_ELVISH` pointing to an interpreter; absence
fails that requested test. It supplies a registration namespace to exercise the unmodified script
without an editor. Windows ConPTY evidence separately checks real interactive registration and
loading. This proves static completion, not runtime target discovery or other operating systems.

`[<geometry>]` stands for the optional all-or-none group `--cols <n> --rows <n>
--cell-width <n> --cell-height <n>`; omitted, the command requests the default 80x24
grid with unknown cell pixels (0,0).

`help`, `completion`, `tui`, `-v`/`-V`/`--version`, `--skill`, the bare invocation, and Windows `--default-config` are fixed text outputs that do
not accept `--format`; a single valid `--format=json` turns the resulting syntax failure into a
JSON `invalid-usage` failure document, while a text/default failure keeps stdout empty. Data
commands (`status`, all `list`/`get`, `workspace close-status`, `agent explain`,
`agent doctor`, `agent wait`, `api snapshot`, `api schema`, `server start`, `server stop`, `server stop-status`,
`server diagnose`, `config check`, and all four `agent integration` commands) and every
mutation command accept `--format`. The `api` commands are the machine surface: a successful `api snapshot` or
`api schema` emits the identical JSON document under both formats, while an `api` failure
follows the selected format like any other command — under text/default it reports on stderr
with empty stdout, and only `--format json` produces a failure envelope.

`orchbee completion` prints a static projection of this grammar: verbs, complete help paths,
each command's declared flags in both the separate and the joined `--flag=value` spelling,
both `--format` spellings and its `text`/`json` values, the closed value vocabularies under
either spelling (`--mouse`, `--direction`, `--key`, `--until`, `--provider`), `--`
termination, and flag-value consumption contexts. It enumerates no Session state, resource
names, or runtime data, and never guesses a freeform value.

### Targets and filters

`<target>` operands accept stable compact identities `@<kind>:0x<hex>`: 1–32 lowercase
hex digits, nonzero and without leading zeros, expanded to an exact 32-digit typed identity.
Lists and text diagnostics print these handles; JSON receipts retain canonical IDs. Completion
accepts them as opaque operands and does not enumerate runtime identities. In PowerShell, quote
the handle, for example `panel get '@panel:0x1'`. Their scope and lifetime equal the full ID;
rediscover runtime terminal IDs after Server restart. Other resource changes cannot retarget them.
Complete typed IDs contain exactly 32 lowercase hexadecimal characters and are nonzero.
A recognized selector prefix requires its declared resource kind and an exact compact identity;
malformed input is `invalid-identifier` (exit 2) before runtime access. Other `@` tokens keep
label interpretation. Other tokens are an exact, case-sensitive label candidate requiring 1–512 Unicode scalars when the resource
admits labels; empty or overlong tokens and the all-zero ID shape are `invalid-identifier`
(exit 2). A label that itself has ID or selector shape can never be reached by label or selector
matching — the entity remains reachable through its real ID. `panel` and `terminal` accept
complete typed IDs and their own `@panel:`/`@terminal:` selectors only; anything else is
`invalid-identifier`. Filters (`--project`, `--workspace`, `--tab`) use the same per-resource
target rules, and so do every mutation operand and `--target-panel` (the `panel` rule, resolved
inside the destination tab). `api schema` takes a nonempty schema ID; an empty one is
`invalid-identifier` with `resource="schema"`, an unmatched one is `not-found`.
`agent get`, `agent explain`, and `agent wait` take one positional token of 1–128
characters drawn from `[A-Za-z0-9._:-]` (or an explicit Panel prefix), resolved by the ID-first rules above, or an
explicit `--agent-session`/`--name`/`--panel` selector. Malformed tokens are `invalid-usage` before runtime access.
`launch-profile get` and `launch-profile delete` take one positional `<selector>`, and
`--profile` carries the same token: a `LaunchProfileSelector` — a nonempty UTF-8 literal
of at most 512 bytes (the name bound, so every admitted selector can reach either rule).
An empty or overlong token is `invalid-usage`; the CLI never splits it into an id/label
case — the Server resolves it under the catalog lock: an exact profile ID first, then a
name whose match cardinality must be exactly one. Zero matches answer `not-found`, a
name matching more than one entry answers `ambiguous` with at most the first 8 candidate
profile IDs plus the true `totalCount` — both under the dedicated launch-profile failure
envelope.

Resolution after a successful read: zero matches → `not-found` (3); multiple exact label
matches → `ambiguous` (3), reporting at most the first 8 candidate typed IDs in canonical
traversal order plus the true `totalCount`. Resolution uses the command's complete declared
snapshot scope without an extra fetch. A filter naming an absent entity is `not-found`;
an existing entity with an empty result set is a successful empty list.

## Query state machine and budgets

Every network query (`status`, `list`/`get`, `api snapshot`) performs exactly this ordered
sequence on Linux; the first failing stage wins and determines the failure:

1. **Cohort** — `NativeCohort::current` under one checked 5-second monotonic deadline
   (`Instant::now().checked_add(5s)`; a failed `checked_add` is `internal-failure`, exit 1).
   Any cohort error (missing, invalid, unsafe, hash mismatch, cooperative deadline) is
   `check-failed` stage `cohort`. Localized stderr gives repair guidance without raw I/O
   details. It names an exact missing executable only when that sibling's open returned
   `NotFound`; otherwise it never guesses which file is missing. Windows
   `status client --format json` v1 carries bounded `ioKind`/`osError` detail and, when
   proven, the exact missing sibling. The failure envelope and exit code are unchanged.
2. **Discovery** — resolve `ORCHBEE_RUNTIME_BASE`/default, then
   `ModeCheckedLocalSiteRendezvous::validate_mode_checked` (`ExistingOnly`, no mkdir, no lock).
   Classification uses the typed `RendezvousError` path compared with the resolved
   `runtime_base`/`runtime_base.join(scope)`: a `System` error whose source is `NotFound` on the
   scope directory is `service-unavailable` stage `discovery`; every other rendezvous or
   filesystem validation failure is `check-failed` with stage `runtime-base` when the typed path
   is the base or a base component and `runtime-directory` when it is the scope directory.
   The pre-classification observation is not claimed atomic with the later open.
3. **Socket** — `client_socket_exists()` reuses the existing type+owner+`0600`+no-follow
   validation: absent → `service-unavailable` stage `discovery`; invalid/error → `check-failed`
   stage `socket`.
4. **Admission** — one new checked 5-second monotonic deadline is created immediately before
   `LocalClient::connect_before`; the same `Instant` is reused unchanged for the snapshot read
   and is never renewed, never falling back to the 50-second request default.
5. **Read** — exactly one `submit_workbench_snapshot_read_before` with an original
   `WorkbenchRequestId`, bounded by the same deadline. One OBLC admission plus one OBWP request;
   no reconnect, retry, publication catch-up, or second read.
6. **Release** — all owned `LocalClient`/socket handles are dropped (RAII) before any mapping,
   formatting, or output; a slow stdout therefore never retains a Session. No shutdown exchange
   exists or is claimed; the Server may observe EOF asynchronously.
7. **Map** — pure deterministic projection of the validated snapshot into the command result.

`config check` is purely local: no socket connect, no lock, no mkdir/chmod, one separate checked
5-second cohort budget for its cohort check. `server diagnose` is the same kind of purely local
command — descriptor-relative path and lock observation only; it never connects, never creates
a lock or a marker, and never repairs — over one cooperative 5-second budget checked between
bounded filesystem operations. Platforms other than Linux and Windows produce `service-unavailable`
`{stage:"platform",reason:"unsupported-platform"}` for network queries and mutations and
`service-unavailable` with the same details for `config check`; there is no fallback to unbounded
connection APIs. `server start` outside Linux/Windows and `server diagnose` outside Linux/Windows answer
`service-unavailable` with the same platform details.

`agent doctor` is the offline provider-hook diagnostic: it explains, from bounded filesystem
facts alone, why Agent observations may not arrive. It performs no provider execution, no
socket connect, no Session discovery, no lock, and no write of any kind — the inspection set is
the declared one above, and every lstat/read/list/PATH scan runs under one 5-second
cooperative budget, ≤64 probe items, ≤16 MiB aggregate read, and per-file/count caps, with
cancellation checkpoints between items. It answers `service-unavailable` with
`{stage:"platform",reason:"unsupported-platform",command:"agent doctor"}` on platforms other than Linux and Windows. A run that meets a bound still produces its document with `complete:false` —
partial facts are explicit, never silently dropped — and `providers[]` always reports every
selected provider in canonical order. `hints[]` are observation-only: each `{id,evidence,text}`
names the concrete fact rows it cites and carries no causal, remedy, or trust claim.

Capsule manifests use contract 1 with exactly `contractVersion` and `payloadSha256`.
The integration result's `capsule.payloadSha256` replaces payload-version fields;
its root is `~/.orchbee/agent-observation-adapters/<payloadSha256>`. Marketplace
inspection uses `absent|current|update-required`; old-root aliases are not admitted.
Doctor recomputes installed bytes before comparing with the current sidecar identity.
A self-consistent different payload is an asset-root `update-required`, carrying
`detail.payloadSha256` and `detail.expectedPayloadSha256`. Recorded enabled hooks
then use the existing update summary and explicit `agent integration enable --provider <id>`
command. A safely identified non-current guarded or Cline contract also requires reinstall;
guarded files must still match their declared fingerprints. Non-current capsule metadata
requires a matching actual digest, required runtime files and named Orchbee package marker
before it can be diagnosed as outdated. Unknown ownership, unsafe paths, unreadable bytes
and corrupt installed trees retain their protected outcomes. Only the strict current source
is ready for mutation. Diagnosis neither repairs setup nor deletes old directories.

### Windows Agent diagnostic refinement

Windows `agent doctor` owns the same bounded offline observation workflow. It emits
`orchbee.agent.doctor` version 1 with the same members as Linux. Capsule 0.4.16 supports native Windows Codex hook
transport, but doctor never connects to a named pipe or writes console correlation hints.
Its machine endpoint probe is `{kind:"unsupported"}`; the human hint retains `windows-endpoint-not-inspected`;
configuration and capsule integrity are not delivery or Session-liveness evidence. `complete`
describes probe coverage. Windows managed installation is owned by the [Windows integration contract](windows-agent-integrations.md); Windows Agent startup is specified in [Windows Agent startup](windows-agent-start.md); prompt composition is specified above.

| Field               | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Inputs and owner    | Native doctor exclusively owns read-only filesystem/PATH/environment observations. Windows home uses the native user profile; a non-empty absolute `CODEX_HOME` naming an existing, non-reparse directory selects Codex configuration. Empty `CODEX_HOME` uses the default; invalid non-empty roots are uninspectable, never silently replaced. This read-only admission is stricter than Codex canonicalizing a relative or symlinked override. |
| Admission           | Regular files are opened without following the final reparse point, then checked through the opened handle before bounded reads. Device namespaces and relative paths are rejected. Reparse entries are uninspected. Ancestor components and synchronous filesystem latency remain outside a hard deadline guarantee.                                                                                                                            |
| Decisions           | Provider configuration absent/disabled/conflicting/unreadable, native executable or script visible/absent, Node visible/absent, capsule consistent/corrupt/over-budget, and an uninspected Windows endpoint are separate facts. PATH search never executes an entry or infers plugin command support.                                                                                                                                            |
| Compatibility       | Windows accepts native absolute paths and single-line TOML literal strings emitted for Windows paths. Existing Linux scanner semantics and output version remain unchanged. Windows endpoint observations use the current embedded schema.                                                                                                                                                                                                       |
| Effects and bounds  | No provider execution, socket connection, Session activation, lock or write. Existing 5-second cooperative budget, 64 items, 16 MiB aggregate reads and per-probe caps apply. Cancellation returns exit 130; produced complete or partial diagnostic documents return exit 0.                                                                                                                                                                    |
| Evidence and limits | Windows isolated process tests cover localized/schema-valid output, path/configuration isolation, missing and installed declarations, corrupt assets and zero setup effects. Filesystem tests cover reparse refusal and read budgets. Doctor evidence does not prove hook delivery; no independent review is claimed; TLA execution is excluded by user instruction.                                                                             |

`terminal read`, `agent read` and `terminal wait-output` share the current source-selection
and read-only viewer lifecycle in [native terminal output](native-terminal-output.md). The
source defaults to recent (80 physical rows); `--lines` controls its bounded range and
`--source recent-unwrapped` joins verified soft wraps. Internal read/history content projections have current schemas but are not CLI
modes. Finite waits default to 30000 ms; `--forever` explicitly removes that deadline. `--after`
pins fresh-output boundaries and refuses reset/eviction. Interruption, known-effect retention
and detach evidence retain the [wait contract](native-cli-wait-model.md).

`terminal send` samples one absolute 5-second deadline at command entry and shares it across
everything below — source materialization for a declared `--file`/`--stdin` (the first
effectful step, before the cohort check), the cohort check under the earlier of its own
budget and that ceiling, discovery, admission, and the snapshot read consuming only what
remains — then uses the same admitted session for exactly one `terminal.send` exchange
under the same deadline. A `--text`/`--key` invocation has no materialization step at all:

0. **Materialize** (declared sources only) — `--file`/`--stdin` is read in full into the
   bounded byte or paste content described below (small UTF-8 input retains the text-record path), before any runtime access; see [Send input
   sources](#send-input-sources). A source failure owns no request identity and never
   reaches the Server.
1. **Resolve** — the typed terminal ID is looked up in the validated snapshot's presentation
   bindings; absent → `not-found` (3) with `{resource:"terminal",target}` and the session
   drops. Labels are never consulted.
2. **Send** — one `terminal.send` request carries an ordered record sequence under a
   fresh request identity using the snapshot's Session, Worker, and terminal facts. `--text`
   produces one UTF-8 record (4096 bytes including optional CR); `--file`/`--stdin`
   produce one opaque byte record (1 MiB including optional CR), retaining the text path
   for small UTF-8 content. `--paste` instead produces one paste record (256 KiB) with
   an optional separate Enter record. Repeated `--key` produces up to 64 semantic-key
   records in argument order; keys are protocol names, never canned escape strings.
   The Client Connection is fully released before the typed result is mapped or output
   prepared. The request is never retried, including after partial acceptance.
3. **Answer** — the Server's `terminal.send.result` splits two axes the envelope reports
   verbatim: `inputStatus` (`accepted` means the Host queued the input record — never that a
   child process read or executed it; `rejected`/`not-submitted` mean the record provably never
   reached the queue; `unknown` means it may already have landed) and `cleanupStatus`
   (`not-needed`/`confirmed`/`unconfirmed`, where `unconfirmed` means the temporary input-only
   attachment may persist until carrier teardown or Host-epoch end — never a claim of
   release). The send rides the shared Worker↔Host link: when that connection fails, or when
   its cleanup cannot be confirmed, other viewers on the same Host may be interrupted — a
   transport-level side effect of the underlying connection, not a normal send seizing
   control. The result's physical `target` (`workerGeneration`, `hostEpoch`,
   `hostTerminalId`) is the incarnation the Server actually admitted, projected verbatim;
   pre-admission refusals carry none. Exit mapping: `accepted`+`confirmed` → 0;
   `accepted`+`unconfirmed` → 5 with a do-not-resend warning (the input may already have
   taken effect); `rejected` → 3; `not-submitted` → 4, except the protocol-fault reason
   `host-protocol-fault` which is a transport-class defect → 5; `unknown` → 8 with a
   never-retried warning. Every nonzero answer keeps `inputStatus` truthful — an accepted
   input is never rewritten as unsent.

`worktree list` shares steps 1–4 of the network preamble plus one snapshot read, all under
one absolute 5-second deadline sampled once at command entry (`snapshot_session_until` —
cohort, discovery, socket, admission, snapshot, and the worktree-list exchange each consume
only what remains; there is no per-stage renewal). It then:

1. **Resolve** — `--directory` is a strict globally stable Directory ID; no Project or local filesystem probe is needed.
2. **Read** — submit one `workbench.worktree-list.read {directoryId}` on the admitted session and total deadline. Server validates the global Directory and dispatches one bounded Worker Git discovery job sharing a five-second TTL and 64 KiB stdout budget. CLI never retries, replays or reconnects.
3. **Map** — release the session before projecting `orchbee.worktree.list` v1: `{directoryId,path,entries}`, without Project identity. Entries retain verbatim Git facts and byte alternates. Exchange failures use `orchbee.cli.failure.worktree`; missing Directory returns `not-found`, never partial entries.

`launch-profile list` and `launch-profile get <selector>` share steps 1–4 of the network
preamble plus one snapshot read, all under the same single 5-second deadline, then:

1. **Read** — exactly one `workbench.launch-profile-catalog.read` request
   `{selector|null}` under a fresh `WorkbenchRequestId`, bounded by the same deadline, on
   the same admitted session. The selector passes verbatim; the Server resolves it under
   the catalog lock (exact profile ID first, then name cardinality). The read is never
   retried, replayed, or reconnected; an undetermined exchange maps through the failure
   matrix.
2. **Map** — the session is fully dropped before the typed `launch-profile-catalog.result`
   is projected. `Catalog` yields the `orchbee.launch-profile.list` document
   `{catalogRevision,profiles:[{profileId,name,launch}]}` (at most 64 entries, verbatim);
   `Profile` yields the `orchbee.launch-profile.get` document
   `{catalogRevision,profile:{profileId,name,launch}}`. Every failure the exchange
   produces — including transport and protocol defects — wears the dedicated
   `orchbee.cli.failure.launch-profile` envelope; `not-found{resource:"launch-profile"}`
   is the unmatched-selector answer and `ambiguous` carries the bounded candidate IDs.
   Both success envelopes carry `source:{kind:"launch-profile-catalog",siteId,
catalogRevision}` — the catalog-private revision, distinct from the Workbench
   navigation revision.

### Send input sources

Windows pipe acquisition owns one hidden, noninteractive Windows PowerShell reader
resolved from the OS system directory. Only the inherited stdin handle is passed;
source bytes never become script or argument text. The fixed script copies at most
the source limit plus one byte into a private output pipe. The CLI polls under the
existing deadline and interrupt guard, and kills/reaps the child on success and handled failures.
It does not change the caller's pipe mode or leave a blocking reader thread behind.
Regular-file stdin retains direct reads. Process startup and termination remain OS
scheduling boundaries, not hard real-time guarantees.

The adapter uses the already admitted `winsafe` system-directory and `winapi-util`
handle-type bindings at their existing locked versions, with no authored unsafe code.
States are admitted -> reading -> complete/failed/timed-out/cancelled -> child-reaped;
only complete can reach submission. `source/input.rs` owns opaque-byte/paste routing:
file/stdin is not restricted to UTF-8. Windows tests exercise real reader processes
for EOF, limits, and timeout, and inject a signal observation to test cancellation;
they do not simulate physical Ctrl+C. This workflow uses implementation tests.

`--file` and `--stdin` are declared local sources. Grammar validation precedes
source acquisition; cohort discovery, admission, snapshot and submission happen
only after the complete bounded source exists. Small valid UTF-8 content retains
the text-record path; other content becomes opaque bytes. Paste content becomes
one paste record and, with `--enter`, a separate trailing Enter record.

- **Pure input boundary.** The four source families are mutually exclusive;
  `--key` alone is repeatable, up to 64 records. `--enter` cannot accompany keys,
  and `--paste` requires file/stdin. Pre-admission source failures submit nothing
  and own no request identity.
- **One shared budget.** The absolute 5-second deadline sampled at command entry covers
  source setup, materialization, and the entire send lifecycle; it is never reset or
  renewed. A deadline check precedes every source wait and every read — including a
  read whose wait ended at the deadline — and also arbitrates the completion boundary
  itself: an end of input observed after the budget closed is `timeout`, never a clean
  record. No runtime admission runs after it expires.
- **Linux file admission.** `--file` opens `O_RDONLY|O_NONBLOCK|O_CLOEXEC` — the open itself
  cannot block on a FIFO or device — then `fstat` on the already-open descriptor must
  report a regular file, which also closes the path-replacement race: a swapped path
  still reports its true type. An unopenable/unreadable path, a non-regular result
  (directory, FIFO, device, or a swapped replacement), an empty file, or a read error is
  a source-stage `invalid-usage`.
- **Linux stdin admission.** `--stdin` refuses a terminal descriptor, then dispatches on the
  inherited descriptor's exact `S_IFMT` type: a regular file is read in place,
  unbuffered, on the borrowed descriptor — the shared file position advances by exactly
  the bytes returned, at most the declared source bound plus one probe byte (that is the inherited-position
  contract); a FIFO/pipe is reopened through `/proc/self/fd/0` into a private
  nonblocking file description, so the caller's descriptor flags are never mutated and
  nothing needs restoring; the reopened description must verify as a FIFO, and every
  other kind (socket, device, …) is rejected.
- **Bounded record.** File/stdin acquisition reads at most the 1 MiB byte bound
  plus one probe byte, or the 256 KiB paste-content bound plus one. Opaque bytes,
  including NUL and non-UTF-8 data, are preserved without trimming, BOM removal or
  CRLF normalization. Empty input is refused. The optional folded CR counts toward
  the non-paste bound; paste uses a separate Enter record. Literal `--text` retains
  its 4096-byte UTF-8 bound including CR. Payload is never included in diagnostics.
- **Source signal guard.** On Linux the source stage owns process signal capture for its
  duration — single-threaded, with no other signal owner and no child spawned before
  handoff. The eligible set is {SIGHUP, SIGINT, SIGQUIT, SIGTERM} minus every signal the
  process already blocks or ignores at source entry: the ignored set comes from a
  strictly bounded `/proc/self/status` `SigIgn` field (at most a 64 KiB read yielding
  exactly one well-formed hexadecimal field — missing, malformed, duplicated, or
  oversized is an internal failure before any source I/O) and the blocked set from the
  current thread mask. Setup creates a `NONBLOCK|CLOEXEC` signalfd for the eligible set
  first — while inherited dispositions still apply, so a setup fault there needs nothing
  restored — then swaps the thread mask atomically and captures the old mask; the capture
  interval begins at the swap, before any source read. Dispositions are never touched and
  inherited blocked/ignored state is preserved exactly — an empty eligible set is a real
  armed policy (nothing to capture), never a no-op fallback, and inherited pending
  signals stay pending under their original mask.
- **Arbitration and finalization.** The read loop polls the signal descriptor beside the
  source descriptor and drains it before processing ready data, before judging a
  timeout, and once more at the completion boundary; the first consumed eligible signal
  selects source cancellation, which wins over deadline and source I/O error.
  Materialization finish linearizes on a final nonblocking signal read: a consumed
  signal means cancel; `EAGAIN` means the source no longer owns cancellation — the
  signal descriptor closes and the exact old mask is restored before runtime admission.
  A signal arriving after that read stays pending and is delivered to its original
  disposition at unmask — the guard never swallows it — so a late signal can still
  terminate the process by default before admission. Every watched syscall is checked:
  descriptor creation, mask swap, each signal read, and the restore; an error is an
  internal failure, never treated as "no signal", and a restore failure is terminal
  with no runtime admission. On cancellation the eligible mask stays blocked while the
  common `main` emit attempts the output, then the old mask is restored on that same
  path — a later signal may still terminate the process after output completes, and no
  exit-status claim is made against it.
- **Failure compatibility.** Validation and local causes stay on the standard
  `invalid-usage` path (no details). Source timeout and cancellation carry the
  `orchbee.cli.failure.source` v1 envelope — `{stage:"input", inputSource:"file"|"stdin"}`
  plus `timeoutMs` (the configured shared budget, never a measured duration) on timeout
  only — with no authority `source` and no request identity, because nothing was
  admitted. Source cancellation exits conventionally `128 + signo` (129/130/131/143)
  through the same common emit finalizer as every other output — the status override
  exists only for this envelope — while source timeout keeps exit 7.
- **Stated OS limits.** The deadline is a logical bound around each syscall, not inside
  it: a kernel-stalled regular-file read is not preemptible, so no prompt-termination
  promise exists there. The pre-arm window before the mask swap runs under inherited
  dispositions — a default death there is honest and effect-free, not a guarded-window
  hole, and neither arming order closes every signal-arrival race. Inherited `SIG_IGN`
  dispositions are respected, not overridden. These are Linux-only claims backed by
  procfs, consistent with the procfd input admission.

`agent list`, `agent get`, and `agent explain` run the same cohort → discovery → socket →
admission preamble as every network query under one absolute 5-second command budget
sampled once at invocation entry: the preamble, admission, optional name resolution
and OBAP read all share that ceiling, which is never refreshed between stages — a stage with
no budget left never starts, and a step completing at or past the ceiling settles as the
command's own timeout. They skip the Workbench snapshot read entirely. On the single
admitted Client Connection they resolve the target when necessary as described above,
then perform one Agent Observation Protocol (OBAP) read exchange:

Apple Silicon macOS development builds currently expose this path for `agent list` without
`--client`; `get`, `explain`, `wait`, `changes`, and the Client projection retain their documented
platform limits.

1. **Request** — one OBAP frame (`OBAP` magic, version 1) under a freshly minted
   `ObapRequestId`: `agent.session.list.request` (kind 1) carrying `includeEnded`,
   `limit`, and an optional `cursor` (omitted on the first page — never serialized as
   `null`, and an explicit `null` on the wire is rejected); `agent.session.get.request`
   (kind 3) or `agent.session.explain.request` (kind 5) carrying the parsed `sessionId`.
   The encoded request is bounded at 16 KiB.
2. **Result** — exactly one matching result frame (kinds 2/4/6), bounded at 1 MiB for
   list and 128 KiB for get/explain; a small refusal rides the ≤1 KiB bound. The frame's
   `requestId` must equal the issued identity and the result kind must match the issued
   operation — any mismatch is `protocol-invalid` stage `agent-read`.
3. **Release** — the Client Connection is fully dropped before the outcome is mapped or
   output is prepared. An ambiguous post-submission outcome retires the session and is
   never replayed: no reconnect, no second admission, no retry.

On Linux the Agent read commands own a separate, narrower signal watch for the whole
armed span — {SIGINT, SIGTERM} only, armed before any runtime contact through the same
private arming mechanics the source stage uses: the eligible set excludes every signal
the process already blocks or ignores, the borrowed signal descriptor joins every
admission and exchange wait, and the exact old mask stays blocked through output and is
restored afterward by the same noncloneable finalization token. Before the arming runs,
process signal behavior stays inherited; SIGHUP and SIGQUIT are never masked or captured
at all — they keep their inherited disposition for the whole invocation, so a default
raw death there is honest and unreported by the CLI, and an inherited ignored or blocked
member simply leaves the eligible set (an empty set is still a real armed policy).
Arming itself establishes cancellation ownership — independent of the admission race —
so a captured signal settles the invocation as `cancelled` on the
`orchbee.cli.failure.agent` document whether it landed during admission or during the
read: `{stage:"agent-read", operation}` with no correlated source, at exit status 130
for either member. 130 is deliberate for both SIGINT and SIGTERM — it is not
`128 + signo`, and the signal's identity is not exposed. The signal wins over deadline,
transport fault, and admission outcomes at the single finish arbitration, which still
runs only after the admitted session is dropped. A provisional wake observation
surviving a clear arbitration is an `internal-failure`, never a transport or timeout
verdict.

Semantic answers map to fixed failure kinds under the `orchbee.cli.failure.agent`
document: `listed`/`found` are the only success outcomes; `rejected{read-denied}` →
`denied`; `rejected{not-found}` → `not-found` (get/explain only, naming the session);
`rejected{cursor-invalid|cursor-stale}` → `rejected` (list only); `unavailable` →
`service-unavailable` with `reason:"runtime-unavailable"`. A deadline elapsed inside the
exchange is `timeout` with `timeoutMs:5000`; a transport fault is `transport-loss`; a
malformed, mismatched, or miscorrelated frame is `protocol-invalid`. Failures from these read exchanges carry `{stage:"agent-read",operation:"list"|"get"|"explain"}` (`"wait"` on the wait command's arms); the correlated
`agent-read` source is attached only to trustworthy semantic answers — transport,
timeout, protocol-invalid, and cancelled failures carry none. Agent reads observe the Server's
canonical Agent sessions only; an `ended` session is reported, never resurrected, and
`--include-ended` widens the page filter rather than the lifecycle itself.

`agent wait` holds one admitted Client Connection. Windows follows canonical changes as described
in the Agent wait policy above and also reads current detail; Linux retains Get polling.
Every exchange has a fresh request identity, and no historical fields overwrite current detail. One
optional absolute monotonic deadline sampled at command start covers admission, every read, and
every backoff; `--forever` omits only that total ceiling; each poll runs under `min(5 s, remaining total)`. Between non-matching
observations the loop sleeps an interruptible private backoff starting at 50 ms,
doubling per quiet poll, capped at 500 ms and by the remaining budget — the signal wake
descriptor joins the sleep, and the held session is dropped before the single finish
arbitration exactly like a single read. There is no re-admission, replay, or new
session after a retired or ambiguous exchange.

Each returned document is evaluated client-side in requested order. The three freshness
conditions (`fresh`, `stale`, `disconnected`) match the observation's freshness
directly; every status condition additionally requires `fresh` freshness. Native status
conditions match the status `kind`; `blocked` and `done` use the compositions above. Multiple conditions may match one document —
`matchedConditions` echoes them in the order `--until` requested them. There is no
baseline or poll-0 change semantics: the first read may match immediately.
`--after-revision` is a caller-supplied global-revision fence — it is not a turn-scoped
or event-causal identity. When the fence is greater than the first returned revision
the command refuses at exit 3 (`rejected`, `revisionAheadDetails`) after exactly one
observation; revision is monotonic so the refusal can never fire later. A read that
returns `rejected{not-found}` at any poll — initial or mid-wait — is `not-found` (3);
`read-denied` is `denied`; `unavailable` is `service-unavailable`; each carries the
correlated `agent-read` source of its answering poll.

Two distinct deadline classes both exit 7 as `timeout` under
`{stage:"agent-read", operation:"wait"}`. `wait-deadline` is the shared absolute budget
expiring — post-step settlement wins over any late-arriving match, so a condition that
lands after the budget is never published. `read-deadline` is one poll's own
`min(5 s, remaining)` bound firing while the total still had room: the held session is
retired by the exchange and is never re-admitted, and the details record the admitted
`readBudgetMs` — the document does not claim the requested conditions were absent for
the whole wait. Timeout details carry the configured `timeoutMs`, monotonic
`elapsedMs`, `completedObservations`, and the last completed observation's
`lastStatusKind`/`lastFreshness`/`lastRevision` — all omitted when zero observations
completed. Cancellation inside the armed span is the same `cancelled` 130 with no
source; admission failures keep the generic failure family.

The existing Linux wait samples the current projected state at each completed poll — it is not a
subscription and provides no change notification. A transition that begins and ends
between two completed observations is missed: pending requests may clear, turn
outcomes are replaced by later turns, and freshness moves between polls. The miss
window is not bounded by the poll interval alone — Server read duration and scheduling
also widen it — and no sticky-state guarantee exists even for conditions that stay
true once reached. On a match the emitted `orchbee.agent.wait` envelope (version 1)
carries `matchedConditions`, the satisfying `revision`, the full `session` detail
verbatim, and a `wait` echo `{until, afterRevision, timeoutMs, observations,
elapsedMs}`; its `source` is the decisive poll's `agent-read` requestId only — failed,
cancelled, and untrusted exchanges publish none. Text output is a single line:
`session <id> matched <conditions> at revision <n> after <ms>ms`.

## Session activation

`server start` is the headless form of the TUI's local startup: both call the single shared
connect-or-activate core (`src/local_activation.rs`) — the sequence is identical and only the
failure projection differs. One absolute 20-second deadline starts before cohort validation
and is never reset per stage:

1. **Cohort** — `NativeCohort::current(deadline)` first, before any runtime-directory work.
   Its typed io kind classifies: `InvalidData` → `incompatible`, `NotFound` →
   `service-unavailable`, `PermissionDenied` → `denied`, `TimedOut` → `timeout`, every other
   kind → `internal-failure` — all with `{stage:"cohort"}`.
2. **Prepare** — resolve `ORCHBEE_RUNTIME_BASE`/default, derive `orchbee-native-<euid>`, then
   `prepare_mode_checked` in create-if-missing mode: the runtime directory may be created even
   on an invocation that later refuses. Rendezvous errors classify exactly like a query's
   discovery stage.
3. **Socket budget** — every derived socket path must fit this platform's `sockaddr_un`;
   an over-budget path is `service-unavailable`
   `{stage:"runtime-directory",reason:"address-too-long"}` before any lock or admission attempt.
4. **Connect or activate** — try an admitted `LocalClient` (a missing or refusing socket is
   retryable inside the deadline; every other connect failure classifies through the query
   `admission` mapping verbatim); take the activation lock, re-check the socket, read the
   marker, take the Session lifetime lock, refuse `rejected` with
   `{stage:"activation",reason:"recovery-required"}` when the marker or sockets show no live
   owner, commit the marker,
   spawn `orchbee-server --local-site <runtime-directory>` from the validated cohort, then
   poll connect plus `try_wait` until readiness. A spawn or `try_wait` failure is the same
   `recovery-required` refusal — the marker may already be committed, so the answer is never
   a clean-timeout claim.
5. **Probe** — the admitted probe `LocalClient` is dropped before output; a success implies
   no held session, grant, or Session identity receipt.

`result.outcome` reports the branch this invocation actually took: `started` only when it
spawned the Server child, `already-running` when an existing Session admitted the probe. A
On Linux, a `timeout` answer means readiness stayed unconfirmed — before marker commit nothing was left
behind, between commit and spawn a marker without a live owner may persist (the next
invocation then answers `recovery-required`), and after spawn a Server may still be running;
the command never claims a timeout proves no Server started, never retries `recovery-required`,
and never force-cleans uncertain runtime state. Windows may have published a generation before marker
commit; a later invocation rechecks exact process identities, registration and all recovery guards.
Only a proved-dead generation is automatically reconciled, preserving its checkpoint. Signals stay inherited: no capture stage exists,
so an interruption is process death under the conventional `128 + signo`, never a `cancelled`
envelope. Windows explicit stop and subsequent start are described below; there is no repair verb.

## Session diagnostics

`server diagnose` emits the same closed `orchbee.site.diagnose` v1 schema on Linux and Windows.
The current catalog is listed above.
The same existing-only runtime path/SID scope is used without cohort admission,
activation, endpoint connection or process discovery. Ordered rows describe the
selected base, admitted runtime directory, marker, activation/lifetime locks and
five named-pipe addresses. Windows reports the filesystem Host socket scope and POSIX mode/UID members as unsupported. `private` means current-user ownership and
the runtime DACL allowlist were checked, not a fresh security guarantee.

The current schema distinguishes a present marker's `activation: active|stopped`; Stopped
also carries `stopRequestId`. It is never rendered as absent. Only Active adds the
activation-recorded advisory. This is the retained completion record's observation,
not a new process-health check; a later start consumes the old stop receipt.

Each row reports known/absent/uninspectable, with free/held/unsafe also available
for the lifetime lock. A denied/invalid/missing parent prevents all descendant
inspection. The activation lock is never acquired. The lifetime probe opens only
an existing admitted file and briefly tries an exclusive lock; it may contend
with startup. Pipe names are unauthenticated facts from at most 2048 namespace
entries per endpoint. Exhaustion is `observation-limit`, never absent. A five-second
cooperative deadline produces partial coverage; filesystem stalls are excluded.
`complete` means every row was attempted or parent-derived, not healthy. All
handles are RAII-owned, including process termination by Ctrl+C. No repair, file
creation/deletion, process launch/stop or retry occurs. Marker and pipe advisories
describe current recovery policy only. Public Windows tests cover absent/held/free,
invalid marker, Unicode paths, schema and localized text, and zero creation;
runtime and IPC tests cover admission, real listeners and bounded observation.

`server diagnose` is the read-only counterpart of activation: it reports bounded facts about the
implicit per-euid local Session runtime without connecting to anything, creating anything, or
repairing anything. The runtime base resolves exactly like `server start` (`ORCHBEE_RUNTIME_BASE`
or `/tmp`, scope `orchbee-native-<euid>`), then the command inspects a finite set of known
paths through the rendezvous' own validated directory descriptor — `AT_SYMLINK_NOFOLLOW`
descriptor-relative `fstatat` plus, for the Session lifetime lock only, an open-existing-file
probe whose `try_lock` result is immediately released on the same descriptor. The activation
lock is metadata only (presence/kind/owner/mode/link count) and is never acquired by the
command. No endpoint is connected — `client.sock` least of all, where a connect would consume
an admission slot — and no process is enumerated, signaled, or hashed.

Every row is a closed-state fact document, never a health verdict: `runtimeBase` and
`runtimeDirectory` carry `path`/`pathBytesHex`; `marker`, `locks:{activation,siteLifetime}`,
`endpoints:{client,worker,clientPending,agentObservation,hostEndpoint}`, and `hostScope` each
report `state` in `known|absent|unsafe|uninspectable` (the lifetime lock also reports `free` and
`held`), plus a concrete `reason` when not fully known — `symlink`, `invalid-kind`,
`foreign-owner`, `invalid-mode`, `invalid-size`, `invalid-link-count`, `invalid-lock`,
`denied`, `io-error`, `invalid-path`, `identity-changed`, `unsafe-parent`, `parent-unavailable`,
or `deadline`. `Present` rows carry the raw facts (`kind`, `ownerMatch`, `mode`, `size`,
`linkCount`) the verdict was computed from. A `held` lifetime lock means only that some owner
held the lock at that observation instant — never that a Session, a Worker, or any process is
alive — and `absent` is a pathname fact, never proof that an execution ended. The Host
directory is admitted before the Host endpoint name is inspected; an unadmitted parent closes
the endpoint row `uninspectable`/`unsafe-parent` without traversal. `complete` is the
attempted-or-classified coverage fact — `false` only when the cooperative 5-second budget was
spent between operations — never a runtime-health claim.

`result.advisories` carries the conditional advisories the current activation predicate's own
facts justify: `activation-recorded` when a fully valid marker was observed, `socket-residue`
when a fully valid client or worker socket leaf was observed — each glossed in text as
"activation recorded" or "socket residue observed; if no existing Session can be joined, fresh
activation is blocked by current recovery policy". The advisory quotes current policy over
observed facts; it never predicts the result of a nonatomic future `server start`, which
attempts an authenticated join before it reads any residue.

The command exits 0 with the complete or partial fact document — unsafe and uninspectable
rows are facts, not command failures. Invocation, engine, and output-preparation failures
answer through the normal failure surface (`invalid-usage`, `service-unavailable`
`{stage:"platform",reason:"unsupported-platform"}` on non-Linux, `output-too-large`,
`output-failed`); an interruption is process death under the conventional `128 + signo`
(`130` for SIGINT). A temporary diagnostic lifetime-lock probe can contend with a concurrent
activation's own lock attempt — a bounded transient effect the caller should expect; the
command documents that effect instead of claiming zero side effects. It never acquires the
activation lock, never creates or deletes a path, never repairs, and never manufactures
liveness or termination witnesses.

## Canonical data and results

Project queries follow `navigation.projects` order. Workspace queries follow `workspaceOrder`,
falling back to global `navigation.workspaces` order when absent or empty. Other traversal follows
`terminalContexts` order then `tabs` order; panels depth-first with `children[0]` before
`children[1]`; terminal bindings in that panel order with `primaryPresentation` before
`childPresentations`. No sorting is added; filtered lists and `ambiguous` candidates preserve it.

Snapshot and one-effect command envelopes use `{schema, version, source, result}`; staged workflows have their own embedded schemas. All machine documents and the catalog use only version `1`. `api schema <id>` retrieves the complete current contract offline. Adding fields or schema IDs updates this current contract directly. The same command keeps the same members across platform and option choices, with unsupported capabilities explicit. Sources retain their owner's meaning:

- `source.kind="workbench-snapshot"` with `siteId` and decimal-string `revision` from the same
  validated snapshot — present only after a complete valid read.
- `source.kind="workbench-command"` with `siteId`, `requestId`, and decimal-string
  `expectedRevision` — produced mutation results, and command-stage failures after a valid
  read.
- `source.kind="embedded"` — the compiled-in schema dictionary (`api schema` results and its
  `not-found` failure).
- `source.kind="local-runtime"` — produced `config check` results, `agent doctor` results, every `server start`
  answer, and every `server diagnose` document: the local runtime is the answering authority
  for activation outcomes, refusals, offline diagnostics, and bounded runtime observations.
- `source.kind="terminal.send"` — `siteId`, the send `requestId`, and the decimal-string
  `snapshotRevision` the binding resolved from, on typed `terminal send` answers.
- `source.kind="terminal-launch-read"` — `siteId` and the decimal-string `revision` the
  Server joined under the coordinator lock, on `terminal get` answers.
- `source.kind="worktree-list"` — `siteId` and the decimal-string `revision` of the
  committed Workbench at the Server's authoritative membership check, on `worktree list`
  results and its post-snapshot failures.
- `source.kind="agent-read"` — the correlated `requestId` the semantic answer carried
  back, on produced agent read results and on post-admission semantic refusals.
- `source.kind="launch-profile-catalog"` — `siteId` and the decimal-string
  `catalogRevision` of the resolved catalog, on `launch-profile list`/`get` results and
  their post-session failures.
- `source.kind="workspace-close-resume"` — `siteId`, the resume `requestId`, the
  `closeRequestId` it names, and the decimal-string `expectedAttemptGeneration` fence the
  request carried, on `workspace close-resume` answers: no `workbench.command` exists for a
  retained lifecycle resume, so the resume request's own echo is the honest source.
- There is no freshness field; one consistent read is not a liveness guarantee.

Result payloads (schema IDs are stable machine names):

- `orchbee.status` → `{site:{reachable:true,siteId,revision},
counts:{projects,workspaces,tabs,panels,boundTerminals},
worker:{status:"unknown",reason:"not-in-workbench-snapshot"}}`. `boundTerminals` counts
  presentations with a `binding`; snapshot validation already guarantees terminal IDs are unique,
  so each binding names a distinct terminal. `reachable` proves only this read succeeded.
  Text reports a running Session only after a successful snapshot read, followed by the
  localized resource counts. It omits the internal Site ID, snapshot revision, and Worker
  observation detail; it does not claim Worker or Agent readiness. The JSON document is unchanged.
- `orchbee.project.list` → `{projects:[{projectId,label,directoryCount,workspaceCount}]}`;
  `orchbee.project.get` → `{project:{projectId,label,directories:[{directoryId,workerId,path}],
workspaces:[{workspaceId,label}]}}`.
- `orchbee.workspace.list` → `{workspaces:[{workspaceId,label,number,tabCount,panelCount,projectId,
primaryDirectoryId|null,terminalContextId}]}` from global navigation, with required-nullable `projectId`; `workspace get` adds
  `tabs:[{tabId,label}]` resolved through `terminalContextId`.
- `orchbee.workspace.close-status` → the verbatim retained `WorkbenchWorkspaceCloseReadOutcome`:
  `{kind:"not-closing"}` when no retained operation covers the scope, or
  `{kind:"available",closeRequestId,workspaceId,terminalContextId,
attemptGeneration,status,counts:{closing,cleanupBlocked,outcomeUnknown,
disposedAwaitingPublication},members:[{tabId,panelId,presentationId,status}…]}`. `status` is
  the scope lifecycle (`kind` `closing`|`cleanup-blocked`|`outcome-unknown`|
  `disposed-awaiting-publication`|`closed` with its tombstone `revision`), each member's
  `status` is the captured per-member lifecycle truth without `closed` (the closed tombstone
  is scope-level only), `attemptGeneration` is the decimal-string attempt fence, and the
  record stays answerable after the Workspace leaves the active projection.
- `orchbee.tab.list` → `{tabs:[{tabId,label,number,panelCount,pinned,groupId|null,
context:{kind:"workspace",terminalContextId,
workspaceId}}]}`; `workspaceId` is always required. `tab get` adds `root`, the
  typed `PanelNode` subtree re-encoded without losing semantic fields.
- `orchbee.panel.list` → `{panels:[{panelId,label,customLabel|null,displayLabel,workspaceId,tabId,terminalContextId,primaryPresentationId,
presentationCount,primaryTerminalId|null}]}` where `primaryTerminalId` reflects only
  `primaryPresentation.binding`; `panel get` → `{panel:{panelId,tabId,context,slotPath,
primaryPresentation,childPresentations}}` with `slotPath` the `PanelSlot` IDs from root to leaf
  (`[]` when the root itself is the panel) and each presentation `{presentationId,label,
binding:{terminalId,workerId}|null}`.
- `orchbee.terminal.list` → `{terminals:[{terminalId,workerId,boundTo:{presentationId,panelId,
tabId,terminalContextId}}]}`; the JSON rows carry identity only, with no lifecycle, process, or
  Agent members. At most one of `--workspace`, `--tab` and `--panel` keeps only the terminals
  bound inside that resolved member, with the standard filter rules: an absent member is
  not-found and an existing member without terminals is an empty list. Windows text output is an
  aligned `HANDLE`, `TITLE`, `AGENT`, `STATE`, `WORKSPACE`, `PANEL`, `CWD` table. Other platforms
  keep the same identity table and render unavailable runtime columns as `-`. `HANDLE`,
  `WORKSPACE` and `PANEL` use stable `@<kind>:0x<hex>` exact identities, so filtering and
  changes to other members cannot affect resolution. All significant digits remain copyable. Every terminal reports its Workspace under `WORKSPACE`. The text-only enrichment shares one five-second deadline and cancellation watch
  across the snapshot, exact-presentation report batches, and at most 64 exact-bound Host viewer
  reads; later rows retain `-` for enrichment. `TITLE` is the stripped Host frame title; a title
  that is only an absolute path to an `.exe`, `.com`, `.cmd`, `.bat` or `.ps1` program shows that
  program's name. `TITLE`, `AGENT` and `STATE` are the only cells shortened to fit 80 cells;
  handles and `CWD` are never cut. `CWD` is the validated terminal-frame directory, falling back to the observed root process directory;
  it never substitutes the Workbench Workspace path. `AGENT` uses the exact presentation report,
  then bounded foreground-process detection; `STATE` preserves a known presentation report state.
  An identified Agent with no known report state displays `unknown`. Otherwise, when a Host frame
  was observed, it displays `observed`, which does not mean the Shell is ready to accept input; with
  neither fact it displays `-`. Missing optional observations render as `-`; denial, transport,
  timeout, cancellation, and protocol failures remain query failures. Each attached viewer gets a
  best-effort detach attempt on both successful and failed
  observation paths. Text preserves printable path characters and escapes terminal controls.
  `--format json` stays on the original snapshot-only path and keeps the same result shape and
  schema version. `terminal get` returns the same identity facts under
  `terminal`, which additionally carries a nested `launch` member — the document is
  `{terminal:{terminalId,workerId,boundTo,launch}}`, never a sibling `result.launch`. Only
  presentation-bound terminals are visible; a binding does not prove the process lives.
  Unlike `terminal list`, `terminal get` does not answer from the Client-held snapshot: it sends
  a `terminal.launch.read` request on the admitted session and the Server resolves the current
  binding, routing, and retained launch fact under the coordinator lock — the snapshot only
  supplies the typed target id. Its envelope is `version:1` with
  `source:{kind:"terminal-launch-read",siteId,revision}` where `revision` is the joined
  workbench revision as a decimal string. `launch` is
  `{state,workerGeneration,createOutcome,geometry}`: `state` is the retained phase verbatim —
  `creating`, `awaiting-start`, `starting`, `started`, `unknown`, `unavailable` — or
  `not-tracked` when no uniquely attributable consistent record exists for the visible binding;
  `workerGeneration` is a positive decimal string, `null` only for `not-tracked`; `geometry` is
  non-null only for `started`; `createOutcome` echoes the retained create outcome, `null` for
  `creating` and `not-tracked`. The required sibling `launchProfile:{profileId,catalogRevision}`
  carries resolved provenance when the launch named a catalog profile; it is `null` for
  explicit `--exec` and default-shell launches. The
  envelope is `version:1`. The state is a retained fact, never process liveness:
  `creating` records a phase, not proof of dispatch, and a deferred-start failure reads
  `unavailable` without a fabricated start reason. Exit codes stay the query contract —
  `not-found` 3, `server-busy` unavailable 4 — and a `found` answer whose
  `launch.state` is `unavailable` with `createOutcome.kind` `failed` still exits 0 because the
  query itself succeeded. The exchange rides the admitted session under the same 5-second
  network budget shared with admission and the snapshot read: a dropped or timed-out exchange
  is `transport-loss` 5, and a correlation mismatch or invalid result is `protocol-invalid` 5 —
  the launch read never reports `timeout`.
  Explicit programs and named-profile launches fill the required nullable sibling `launchSelection:{exec,args}` with the actual program and arguments frozen at creation. Profile deletion and Panel movement never rewrite it. Default-shell and `not-tracked` facts leave it null; environment values are not exposed. Text output uses localized labels for the program and JSON argument array. This read does not guarantee launch-command persistence across restart.
- `orchbee.terminal.read` → `{terminal,frame,content}`: `terminal` carries the same
  `{terminalId,workerId,boundTo}` identity facts; `frame` carries the applied projection's
  `{revision,generation,sequence}` as decimal strings (u64-safe), `alternateScreen`, `geometry`
  `{columns,rows}`, and `cursor` `{row,column,visible}`; `content` carries `rows` (the emitted
  grid rows as `{index,text,continuation:{fromPrevious,intoNext}}` — `text` joins the row's
  cells, dropping wide-cell spacer tails and trimming grid padding), `rowCount` (every row the
  applied frame held), `truncated`/`omittedRows`, and `limits` `{rows:4096,textBytes:4194304}`.
  Logical-line rendering replaces the emitted `rows` member with `lines` — each
  `{firstRow,lastRow,text,startsMidLine,endsMidLine}` a joined logical line or a flagged
  window-edge fragment — plus `lineCount`; under `--lines` a `tail` member reports
  `{requested,omitted,entries:"rows"|"lines"}`. Emitted rows are bounded by a 4 MiB summed
  JSON-encoded-text budget; rows past the budget are
  counted in `omittedRows` and `truncated` is true. Under text output each emitted row (or
  logical line) prints as one escaped line, followed by the localized tail marker when
  `tail.omitted>0` and the truncation marker when `omittedRows>0`. The read is a
  single point-in-time frame: `revision`/`generation`/`sequence` are the host's counters at that
  observation, not a CLI freshness claim.
- `orchbee.terminal.history` → `{terminal,history,content}`: `history` carries the priced
  window's facts — `revision`/`generation`, `alternateScreen`, `columns`, `screenRows`, the
  decimal-string `scrollbackRows`/`totalRows`/`startRow`, `requestedRows`, and `windowRows` (the
  Host window's complete row count); `content` carries `rows` (`{index,text,continuation}` with
  `index` the absolute retained-plus-screen row coordinate as a decimal string), `rowCount`
  (emitted rows), `truncated`/`omittedRows`, and `limits` `{rows:1000,textBytes:4194304}` —
  with the same logical-line and `--lines` substitutions as `terminal read` (`lines` with
  decimal-string `firstRow`/`lastRow`, `lineCount`, `tail`). `omittedRows` honestly counts
  `min(requestedRows,scrollbackRows)` minus emitted rows — rows the
  `MAXIMUM_FRAME_CELLS/columns` cap or the text budget never emitted — while rows older than
  `startRow` were never requested, not omitted. Text output matches `terminal read`: one escaped
  line per emitted row plus the marker.
- `orchbee.terminal.send` → `{action:"terminal.send",requestId,siteId,workerId,terminalId,
payloadKind,records,recordBytes?,recordsAccepted,firstStoppedRecord,acceptedEncodedBytes,
inputStatus,cleanupStatus,reason?,target?}`. `payloadKind` is text, key, bytes, paste or sequence;
  `records` is the submitted record count. `recordBytes` describes the single content record
  (including a folded CR for text/bytes), never a guessed encoding size for trailing semantic
  keys. `recordsAccepted`, nullable zero-based `firstStoppedRecord` and `acceptedEncodedBytes`
  report confirmed progress verbatim; a lost-response failure envelope does not fabricate them.
  `inputStatus` is accepted, rejected, partial-accepted, not-submitted or unknown;
  `cleanupStatus` independently reports cleanup. `reason` is the typed rejection/unavailability
  token. Optional `target` is the physical incarnation actually admitted by the Server
  (`workerGeneration`, `hostEpoch`, `hostTerminalId`). Submitted content is never echoed in
  receipts or diagnostics. The obsolete `key` and `acceptedBytes` fields are not part of the
  current sequence receipt.
- On the canonical non-Windows route, `orchbee.agent.list` v1 returns the `listed` outcome verbatim:
  `{kind:"listed",items:[<session summary>…],nextCursor|null,revision}` where each summary
  is the wire `AgentSessionSummary` (session/provider/lifecycle/semantic status/freshness/
  observation counters/execution location/pending-request count ≤128/presentation
  attachments) and `nextCursor` is the Server-minted continuation token for `--cursor`.
- On the canonical non-Windows route, `orchbee.agent.get` returns the `found` outcome verbatim:
  `{kind:"found",revision,session}` where `session` is the wire `AgentSessionDetail` —
  the summary fields plus `capabilities` (≤10 unique tokens), `lastOutcome`, and
  `pendingRequests`.
- `orchbee.agent.explain` → the `found` outcome verbatim:
  `{kind:"found",revision,session,explanation}` where `explanation` is the canonical
  11-field object (`status`, `statusBasis`, `freshnessMeaning`, `capabilityCeiling`,
  `fidelity`, `freshness`, `generation`, `sequence`, `source`, `actionPath`,
  `contentAvailable`) — the Server's derived reading, never a CLI inference.
- `orchbee.agent.wait` → the matched answer:
  `{matchedConditions,revision,session,wait}` where `matchedConditions` is the
  satisfying subset of the effective `until` list in requested order (1–14 unique
  condition names), `revision` is the satisfying poll's global runtime revision,
  `session` is that poll's wire `AgentSessionDetail` verbatim. `matchedChange` separates historical matching evidence from that sample; it is nullable on Windows and unsupported on Linux; `wait` echoes the
  effective run `{until,afterRevision,timeoutMs,observations,elapsedMs}` —
  `afterRevision` always present, null when no revision was selected.
- `orchbee.api.snapshot` → `{snapshot}`: the complete `WorkbenchProtocolDocument::Snapshot`
  semantic re-encode including `kind:"workbench.snapshot"`; envelope `siteId`/`revision` agree
  with the inner document.
- `orchbee.api.schema` → `{schema:{schemaId,document}}` for a named schema, or
  `{schemas:{schemaId:document,…}}` listing the whole delivered dictionary (draft 2020-12).
  The catalog stays closed: `result.schemas` names exactly the registered ids and each
  embedded document's `$id` matches its key. Introspection clients use the embedded current document from this same build.
- `orchbee.worktree.list` → `{directoryId,path,worktrees:[<entry>…]}`: the
  registered directory identity (`path` is the registered Workbench directory — distinct
  from each entry's observed `worktreePath`) plus the verbatim `git worktree list
--porcelain -z` projection. Each entry carries exactly one of `worktreePath` (UTF-8) or
  `worktreePathBytesHex` (lowercase-hex raw bytes), `head` as a 40- or 64-character
  lowercase-hex object name or `null` for a bare repository (an all-zero HEAD is the real
  Git value for an unborn repository and is preserved verbatim), exactly one of `branch`
  (the verbatim full ref such as `refs/heads/main`, never stripped), `branchBytesHex`, or
  `null` when no branch applies, and the `bare`/`detached`/`locked`/`prunable` flags.
- `orchbee.cli.failure.worktree` → `{schema:"orchbee.cli.failure.worktree",version:1,
failure:{kind,details?},source?}`: the dedicated failure envelope for every `worktree
list` failure past the snapshot boundary (pre-boundary grammar, preamble, and project
  resolution failures keep `orchbee.cli.failure`). Its `details` vocabulary is closed:
  `{resource:"directory",target}` on `not-found`, `{stage:"worktree-list"[,reason]}` on
  `denied`/`service-unavailable` (`server-busy`|`worker-unavailable`|`job-dead`|
  `job-unknown`)/`transport-loss`/`protocol-invalid` (`reason:"correlation-mismatch"`),
  `{stage:"probe"|"list",status}` (`status` integer or `null`) or `{stage:"parse"}` on
  `check-failed`, `{stage:"worktree-list",
timeoutMs:5000}` on `timeout`, and `{stage:"worktree-list",origin,limitBytes}` on
  `output-too-large` (`origin` `git-probe`|`git-list` → 65536, `worker-result` → 7936,
  `workbench-result` → 32768).
- `orchbee.launch-profile.list` → `{catalogRevision,profiles:[<entry>…]}`: the decimal-string
  catalog-private revision plus every catalog entry (at most 64) in catalog order —
  `{profileId,name,launch:{exec,args}}`, all verbatim.
- `orchbee.launch-profile.get` → `{catalogRevision,profile:<entry>}`: the same entry
  shape for the single Server-resolved profile.
- `orchbee.cli.failure.launch-profile` → `{schema:"orchbee.cli.failure.launch-profile",
version:1,failure:{kind,details?},source?}`: the dedicated envelope for every
  `launch-profile` query failure past the session boundary and for profile-resolution
  failures on `launch-profile delete`, `tab create --profile`, and `panel split
--profile` (pre-boundary grammar and preamble failures keep `orchbee.cli.failure`).
  Its `details` vocabulary is closed: `{resource:"launch-profile",target}` on
  `not-found`, `{resource:"launch-profile",target,candidates≤8,totalCount}` on
  `ambiguous` (candidate IDs under the profile-id grammar),
  `{stage:"launch-profile-catalog"}` on `denied`/`transport-loss`, the same plus
  `reason:"service-unavailable"` on `service-unavailable`, the same plus
  `reason:"correlation-mismatch"` on correlation `protocol-invalid`, and
  `{stage:"launch-profile-catalog",timeoutMs:5000}` on `timeout`. `source` is the
  `workbench-snapshot` source on read failures and the `workbench-command` source on
  mutation-side failures.
- `orchbee.mutation.result` → `{action,outcome,revision?,acceptedRevision?,closeRequestId?,
attemptGeneration?,<resolved typed IDs>,direction?,label?,terminal?,catalogRevision?,
profileId?,name?}` per
  the mutation matrix above:
  `revision` on `committed`/`unchanged`, `acceptedRevision`+`closeRequestId` on `accepted`,
  `closeRequestId`+`attemptGeneration` on `resumed`,
  and `terminal:{outcome,reason?,geometry?}` on `tab create`/`panel split`. The
  launch-profile actions add `catalogRevision` plus `profileId` on both receipts and
  `name` on `launch-profile.create`; the envelope is `version:1`. Text output of this receipt and
  of the Windows Workspace and terminal creation receipts shares one localized shape: a sentence
  naming what happened, then aligned rows for each created or affected Project, Workspace, Tab,
  Panel and launch profile ID, with label, directory and terminal outcome. Revisions, request IDs,
  and context, directory, split, slot and presentation IDs appear only in JSON. A receipt whose
  terminal started or is starting ends with a `terminal list --panel` hint for the new Panel,
  because the spawn outcome does not carry the terminal ID. `panel current` text starts with the
  current Panel ID. `workspace get`, `terminal get` and `panel process-info` text likewise opens
  with one sentence followed by aligned labels: the Workspace view shows its Project and primary
  directory path before any Agent or worktree supplement; the terminal view shows its Panel, Tab
  and launch state; the process view names the foreground process with its PID, UTC start time,
  directory and command, omitting the shell initialization Orchbee appends to interactive
  PowerShell, then lists every sampled process. Worker, context, presentation and directory IDs,
  raw launch structures and FILETIME values remain JSON-only.
- `orchbee.cli.failure.close` → `{schema:"orchbee.cli.failure.close",version:1,
failure:{kind,details?},source?}`: the dedicated failure envelope for every
  `workspace close-status` and `workspace close-resume` failure past the admitted session
  (pre-boundary grammar, preamble, and the initial `workspace close` command's own failures
  keep `orchbee.cli.failure`). Its `details` vocabulary is closed:
  `{stage:"close-status"|"close-resume"}` on `denied`/`transport-loss` and bare
  `protocol-invalid`, the same plus `reason:"server-busy"` on `service-unavailable`,
  `{stage:"close-status",timeoutMs:5000}` on `timeout`, `{stage,reason}` with `reason`
  `correlation-mismatch`|`scope-mismatch` on classified `protocol-invalid`,
  `{stage:"close-resume",reason}` on `rejected` where `reason` is the typed resume
  verdict (`stale-generation`|`operation-unavailable`|`operation-mismatch`|
  `request-identity-conflict`|`generation-mismatch`|`not-resumable`|
  `generation-exhausted`), and `{stage:"close-resume",requestId,reason}` on
  `outcome-unknown`. `source` is the `workbench-snapshot` source on close-status
  failures and the `workspace-close-resume` source on resume failures.
- `orchbee.project.rename` → `{action:"project.rename",outcome,revision,projectId,label}`
  with `outcome` in `committed|unchanged`: the dedicated v1 document for `project rename`
  receipts, so the shared `orchbee.mutation.result` action enum stays closed and untouched.
- `orchbee.config.check` → `{ok,checks:[{name,status,details}]}` in fixed order
  `runtime-base`, `runtime-directory`, `cohort`, `locale`, `sockets`; all five always report.
  A failed check still produces this success schema with `ok:false` and exit 1.
- `orchbee.agent.doctor` → `{complete,providers:[{id,executable,runtime:{node},hook}],
endpoint:{socket},environment:{observationSocketEnv},asset:{roots:[<root>…]},hints:
[{id,evidence,text}]}`. `providers` reports every selected provider in canonical order;
  each fact row is `{state[,reason][,detail]}` over closed state/reason vocabularies —
  `executable`/`node` report PATH scan facts (`manualCommand` on providers whose hook
  state needs their own command surface), `hook` reports the managed-hook inspection
  outcome (`present`/`absent`/`disabled`/`conflict`/`unknown`/`update-required`/
  `unsupported`), `socket`/`observationSocketEnv` report the observation endpoint facts,
  and each root carries `origin` (`supplied`/`codex`/`cline`/`opencode`/`pi`/`omp`/
  `gemini`/`qwen-code`), `state`, and exactly one of `path` (UTF-8) or `pathBytesHex`
  (non-UTF-8). `hints[].text` is the canonical `en-US` observation sentence — the machine
  document stays locale-neutral while text output re-localizes the same sentence from
  `id` and `evidence`. `complete:false` marks a budget-bounded partial document; the
  command still exits 0.
- `orchbee.site.start` → `{outcome,scope,runtimeBase}`: `outcome` is `started` or
  `already-running` (the branch this invocation took, not a Session identity receipt), `scope`
  is `orchbee-native-<euid>`, and `runtimeBase` carries exactly one of `path` (UTF-8) or
  `pathBytesHex` (non-UTF-8, never a lossy display string).
- `orchbee.site.diagnose` → `{complete,scope,runtimeBase,runtimeDirectory,marker,locks,
endpoints,hostScope,advisories}`: the bounded observation document described in Session
  diagnostics — closed row states and reasons, `locks:{activation,siteLifetime}`,
  `endpoints:{client,worker,clientPending,agentObservation,hostEndpoint}`, `path`/`pathBytesHex`
  on the two directory rows, and the conditional `advisories` token list.
- `orchbee.cli.failure` → `{schema:"orchbee.cli.failure",version:1,failure:{kind,details?},
source?}`; no English `message` field exists. `source` is attached only from an already trusted
  source (a validated snapshot for post-read failures, `embedded` for `api schema` not-found,
  `workbench-command` for every command-stage mutation failure, `local-runtime` for every
  `server start` answer including its refusals, `terminal-launch-read` for `terminal get`
  output-preparation failures after a trusted join); pre-read and grammar failures have none.
- `orchbee.cli.failure.source` → `{schema:"orchbee.cli.failure.source",version:1,
failure:{kind,details}}` — the send source-stage envelope for `timeout` and `cancelled`
  only. `details` is `{stage:"input",inputSource:"file"|"stdin"}` plus `timeoutMs` on
  timeout; there is never an authority `source`, a request identity, a path, or a payload
  byte, because the failure provably precedes any runtime effect.
- `orchbee.cli.failure.agent` → `{schema:"orchbee.cli.failure.agent",version:1,failure:
{kind,details?},source?}` — the post-admission agent read failure document: `kind` ∈
  `{cancelled,denied,internal-failure,not-found,protocol-invalid,rejected,
service-unavailable,timeout,transport-loss}` with a closed per-kind `details` vocabulary
  (`{stage:"agent-read",operation[,reason|sessionId|timeoutMs]}` where `operation` ∈
  `{explain,get,list,wait}`; `wait` adds `revisionAheadDetails`
  `{sessionId,afterRevision,revision}` on `rejected` and `waitTimeoutDetails`
  `{timeoutMs:null|1-3600000,elapsedMs,reason:wait-deadline|read-deadline,readBudgetMs?,
completedObservations,lastStatusKind?,lastFreshness?,lastRevision?}` on `timeout`
  while the three single-read verbs keep `timeoutMs:5000`), and `source` only on
  trustworthy semantic answers. Pre-admission failures on the agent commands keep the
  generic `orchbee.cli.failure` document instead.

### Config checks

| check               | rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime-base`      | `ORCHBEE_RUNTIME_BASE`/default resolved and observed: pass when it exists as a directory; fail on invalid path, absence, unsafe entry, or I/O error.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `runtime-directory` | `not-applicable` when the base failed or the scope directory is absent under a valid base (a Session that is not running is not a configuration error); pass on a validated existing directory; fail on unsafe owner/mode/type or I/O error.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `cohort`            | pass with `verified:true` when `NativeCohort::current` validates under its 5-second budget; fail on absence, invalidity, hash mismatch (`invalid-cohort`) or deadline (`deadline`).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `locale`            | informational pass carrying `locale` (`en-US`/`zh-CN`) and `source` (`LC_ALL`/`LC_MESSAGES`/`LANG`/`default`).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `sockets`           | `client`/`worker` report `presence` `present`/`absent`/`unchecked` with the existing type+owner+`0600`+no-follow rule after a safely inspected runtime directory. `clientPending`/`host` are derived-path budget entries only: `presence` is always `unchecked`, the address fits yields `not-applicable`/`presence-not-inspected`, and an over-budget address yields `fail`/`address-too-long`; both always carry `path`/`pathBytesHex` plus `addressBytes`/`addressCapacityBytes`. Any entry's `fail` fails the check; otherwise at least one compliant present `client`/`worker` passes; anything else is `not-applicable`. Presence never claims service or Worker liveness, and a fitting budget never claims the endpoint exists — `not-applicable` here means presence was deliberately not inspected, not that the budget is unknown. A derived socket path whose byte length exceeds this platform's `sockaddr_un` capacity fails `address-too-long` regardless of presence — it can never be bound or connected. |

Typed details only: `runtime-base`/`runtime-directory` carry `path` (UTF-8) or `pathBytesHex`
(non-UTF-8, never replacement characters), `exists` only when the path state was established —
`false` for an established absence, `true` for an observed entry, and the field is omitted when
inspection itself failed (permission, path, or I/O error) — plus `reason` in
`{invalid-path,absent,unsafe,io-error}`; `sockets` reasons are `{unsafe,io-error,
runtime-unavailable,address-too-long,presence-not-inspected}`. The `clientPending`/`host`
budget entries and every `address-too-long` entry carry `path`/`pathBytesHex` plus
`addressBytes` (the derived path's encoded byte length) and `addressCapacityBytes` (the
platform socket-address capacity). No cross-file atomic health claim is made.

## Mutations

`terminal send` mutates Host input state, not the Workbench document: it rides the admitted
session's OBCS `terminal.send` exchange described above and does not touch `expectedRevision`,
the Workbench command channel, or the TUI's controller/viewer/resize ownership.

Fifteen commands mutate Workbench state through the admitted OBWP `workbench.command` channel:
`project create`/`rename`, `workspace create`/`rename`/`accept-directory`/`close`,
`tab create`/`rename`/`move`/`close`, `panel split`/`close`/`move`, and
`launch-profile create`/`delete`. Each invocation runs
the query state machine's stages 1–5
unchanged — one cohort check, one discovery and admission, one bounded snapshot read — then
performs two further stages under the same 5-second deadline:

6. **Resolve** — a pure projection of the validated snapshot into the wire action: every
   target resolves with the query resolver's exact semantics (`not-found`/`ambiguous`
   possible, zero commands submitted), the snapshot supplies `siteId` and
   `expectedRevision`, and one fresh `WorkbenchRequestId` is generated.
7. **Command** — exactly one `workbench.command` submission on the admitted session. The
   Client Connection is then fully released before the outcome is mapped or output prepared.

The `expectedRevision` choice is the protocol's optimistic-concurrency contract: the Server
compares the command's base revision to its current revision and rejects a mismatch without
applying anything, so reading once and submitting at that revision is the admitted
interaction sequence — the CLI never re-reads to chase a moving base and never submits
without a base.

Safety contract:

- **At most one effect.** Exactly one command is submitted per invocation; all argument and
  target validation completes before the command is built, so a parse defect or unresolved
  target submits nothing.
- **Fresh identity.** Each invocation generates one new globally unique
  `WorkbenchRequestId`; receipts and `outcome-unknown` failures report it for operator
  reconciliation.
- **Server-owned revision.** `expectedRevision` always comes from the same validated
  snapshot that resolved the targets — never guessed, incremented client-side, or reused.
- **No automatic replay.** An ambiguous post-submission outcome — lost write, lost,
  malformed, or miscorrelated response, or an answer that fails validation — retires the
  session and surfaces as `outcome-unknown` (exit 8). The command may or may not have been
  applied; it is never resent. The localized diagnostic says so and directs the operator to
  inspect Workbench state (`status`, `tab list`, `panel list`, …) before deciding whether
  to repeat the command.
- **Close confirmation.** `tab close`, `panel close`, and `workspace close` are
  destructive; the parser requires the explicit `--confirm` flag so a noninteractive
  invocation can never close by default. `workspace close-resume` owes the same flag:
  opening a fresh attempt window on a retained close is itself an effect.
- **Close waiting.** `tab close` and `workspace close` by default read fresh snapshots after
  acceptance until the Tab or Workspace is gone, within a total `--timeout` (1..3600000 ms,
  default 30000). The text receipt then reports the closed name and terminal count. Expiry exits
  with a `timeout` failure (`details:{stage:"wait",timeoutMs,observed}`) while the close
  continues in the session; the text diagnostic names the `tab list` or `workspace close-status`
  command. `--detach` keeps the `accepted` receipt returned at acceptance and conflicts with
  `--timeout`. `--dry-run` submits nothing and returns the `terminal list` result filtered to that
  Tab or Workspace, with text opened by one dry-run sentence; it conflicts with `--detach` and
  `--timeout`. The JSON receipt shape is unchanged.

Outcomes: `committed` reports the new `revision` (base + 1); `unchanged` — a rename or move
that changed nothing — reports the current revision; `accepted` — the closes — reports
`acceptedRevision` plus the Server's `closeRequestId` for the asynchronous close;
`resumed` — `workspace close-resume` — reports `closeRequestId` plus the new
`attemptGeneration` the Server opened.
`tab create` and `panel split` additionally carry `terminal`: `pending` while the Server
still owes a spawn, or the typed spawn outcome (`started` with the echoed `geometry`,
`unavailable`/`failed` with a reason, `unknown`).

`project create` is the only mutation with no target operand and no filesystem effect: the
action carries just the label, the Server allocates the new `projectId` and returns it on the
`committed` receipt, and the created Project starts with no directory or Workspace. Duplicate
labels are legal — the label never uniquely identifies the new Project; the receipt's
`projectId` does.

`workspace accept-directory` submits `WorkspacePrimaryDirectoryAccept` for the globally resolved Workspace ID, without Project identity. `--path` keeps its literal absolute-path, 4096 UTF-8 byte and no-NUL admission; CLI never creates, repairs or probes directories. Server probes authoritatively and refuses a Workspace that already has a primary Directory. Windows `workspace create` admits the new Workspace's directory within one launch and needs no separate accept-directory command.

Geometry is always explicit in the request: a non-TTY invocation cannot inherit terminal
dimensions, so `tab create` and `panel split` accept the four flags as an all-or-none
group whose omission requests the default 80x24 grid with the paired-zero (unknown)
pixel pair — a CLI request policy materialized into the wire geometry, never a measured
display fact. `--cols`/`--rows` are grid cells and
`--cell-width`/`--cell-height` are per-cell pixel extents; every dimension is a `u16`
bounded by the protocol `WorkbenchTerminalGeometry` validation before any runtime access.

`tab create` and `panel split` launch through one of three selections: the resolved
default shell, the explicit `--exec`/`--arg` pair, or `--profile <selector>`. The
selector is passed verbatim in the command — the Server resolves it under the catalog
lock at command time (exact ID, then a singly-matching name) and the retained launch
becomes the terminal's provenance. An unresolved selector is a zero-change committed
outcome the CLI maps to `not-found`; a multi-matching name maps to `ambiguous` — both
under the launch-profile failure envelope with the command's `workbench-command` source.

`launch-profile create` and `launch-profile delete` mutate the catalog itself through
the same one-command envelope. `create` carries `--name` plus the same validated
`LaunchSelection` the explicit flags build and the committed receipt reports `revision`
(the Workbench revision), `catalogRevision` (the catalog-private counter), the minted
`profileId`, and the stored `name` verbatim. `delete` carries the selector unresolved —
the Server applies the ID-first/name-cardinality rule under the catalog lock; the
committed receipt reports `revision`, `catalogRevision`, and the deleted `profileId`.
A `ProfileUnavailable` settled outcome maps to `not-found{resource:"launch-profile"}`
and `ProfileAmbiguous` maps to `ambiguous` with the bounded candidate IDs — both under
the launch-profile failure envelope; catalog capacity refusals arrive as ordinary
command-stage rejections under the generic envelope.

Workspace and organization mutations use `schema:"orchbee.mutation.result"`, `version:1`; other existing actions retain v1, with
`source:{kind:"workbench-command",siteId,requestId,expectedRevision}` and a `result` carrying
`action` (the dotted command name), `outcome`, the outcome's revision fields, the resolved
typed IDs, and `direction`/`label`/`terminal`/`catalogRevision`/`profileId`/`name` where
the command has them. `workspace close-resume` is the single source exception: no
`workbench.command` exists for a retained lifecycle resume, so its `resumed` receipt and
its post-submission failures report `source.kind="workspace-close-resume"` — the resume
request's own echo with `siteId`, `requestId`, `closeRequestId`, and
`expectedAttemptGeneration`. Text output
renders the same fields one per line under the same escaping rules as queries.
`project rename` is the single exception: its receipt uses the dedicated
`schema:"orchbee.project.rename"`, `version:1` document — the same `workbench-command`
`source` shape, and a `result` closed to `action:"project.rename"`, `outcome`
(`committed`/`unchanged`), `revision`, `projectId`, and `label`.

`workspace close` submits the `WorkspaceClose` action — the Server-authoritative close of
every terminal the Workspace's Clients use, reusing the same terminal-tab-close lifecycle
machinery as `tab close`/`panel close` at the whole-context scope. After terminal cleanup,
the Workspace and its terminal context leave the active projection; its checkout, branch,
and directories are preserved. An `accepted` receipt carries `acceptedRevision`, the operation's
`closeRequestId`, and the resolved `workspaceId`/`terminalContextId`; the Session
finishes the close asynchronously and the retained operation record outlives the
Workspace's active projection. Re-running `workspace close` is never a resume — a resumable
retained operation is continued only through `workspace close-resume`.

`workspace close-status` reads that retained record by strict stable IDs alone: it shares
the query state machine's session admission but never resolves Workspace membership
locally — the record outlives the projection row, so a completed close still answers
`available` with its `closed` tombstone `revision`, while a scope that never saw a close
answers `not-closing`. `workspace close-resume` is a retained-lifecycle request, not a
`workbench.command`: it runs the same admission discipline and submits exactly one resume
exchange carrying the close operation's `closeRequestId` and the caller's last-observed
`expectedAttemptGeneration` fence. Only resumable lifecycle states resume — unknown
outcomes are non-resumable — and a stale fence settles as the typed `stale` answer,
reported as `rejected` with `reason:"stale-generation"` under the close failure envelope,
never as a second attempt.

Deliberately not delivered: `project close`, `panel resize`, terminal open operations, Session lifecycle
restart, disable, recovery or repair commands (Windows stop/status are supported), and every remote operation.

## Failure matrix

Exit codes: 0 success (empty lists included); 1 local config/output/internal determinate failure;
2 usage/invalid target; 3 not-found/ambiguous/denied/rejected; 4 service-unavailable (including
`connection-capacity`); 5 transport-loss/protocol-invalid; 6 incompatible; 7 `timeout`
(`terminal wait-output` deadline exhausted without a match, or the send source-stage shared
deadline expiring during materialization); 8 outcome-unknown; 130 `cancelled` (a handled
SIGINT-class interruption — a HUP/INT/QUIT/TERM during `terminal wait-output` or an eligible
signal between `agent doctor` probe items — and, more generally, a handled `terminal send`
source-stage signal exits conventionally
`128 + signo`: 129/130/131/143 for the eligible set). During an `agent` read a captured
SIGINT or SIGTERM settles as the typed `cancelled` agent failure at status 130 — both
members, by contract — while the unwatched SIGHUP/SIGQUIT keep their inherited
dispositions. An eligible signal the source guard
never captured is delivered to its original disposition — an OS default termination follows
the shell's `128 + signal` convention and is not a synthesized CLI result.

| Source                                                                                                                                    | `failure.kind`                                                             | exit      | `details`                                                                                           |
| ----------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | --------- | --------------------------------------------------------------------------------------------------- |
| argv envelope/grammar/format/target shape errors                                                                                          | `invalid-usage`                                                            | 2         | omitted                                                                                             |
| zero-ID/empty/overlong label, malformed panel/terminal ID                                                                                 | `invalid-identifier`                                                       | 2         | `{resource,target}`                                                                                 |
| valid target or schema ID without a match                                                                                                 | `not-found`                                                                | 3         | `{resource,target}`                                                                                 |
| multiple exact label matches                                                                                                              | `ambiguous`                                                                | 3         | `{resource,target,candidates≤8,totalCount}`                                                         |
| `NativeCohort::current` error                                                                                                             | `check-failed`                                                             | 1         | `{stage:"cohort"}`                                                                                  |
| base invalid/missing/not-a-directory, unsafe runtime/socket, other validation                                                             | `check-failed`                                                             | 1         | `{stage:"runtime-base"                             \| "runtime-directory"   \| "socket"}`           |
| valid base with absent scope directory or absent `client.sock`                                                                            | `service-unavailable`                                                      | 4         | `{stage:"discovery"}`                                                                               |
| `ConnectClientError::ConnectFailed`                                                                                                       | `service-unavailable`                                                      | 4         | `{stage:"admission"}`                                                                               |
| `ConnectClientError::Io`                                                                                                                  | `transport-loss`                                                           | 5         | `{stage:"admission"}`                                                                               |
| OBLC `Rejected(CapacityExceeded)`                                                                                                         | `connection-capacity`                                                      | 4         | `{stage:"admission"}`                                                                               |
| OBLC `Rejected(HandshakeTimeout)`                                                                                                         | `service-unavailable`                                                      | 4         | `{stage:"admission"}`                                                                               |
| `Rejected(IncompatibleProtocol                                                \| IncompatibleCohort)`, `Protocol(IncompatibleVersion)`    | `incompatible`                                                             | 6         | `{stage:"admission"}`                                                                               |
| `Rejected(WrongMessage                                                        \| MalformedFrame)`, `UnexpectedResponse`, other `Protocol` | `protocol-invalid`                                                         | 5         | `{stage:"admission"}`                                                                               |
| `RequestIoConfiguration`, `ConnectionUnavailable`, `RequestEncoding`                                                                      | `internal-failure`                                                         | 1         | omitted                                                                                             |
| `SnapshotReadError::RequestNotSubmitted` (any cause)                                                                                      | `transport-loss`                                                           | 5         | `{stage:"snapshot"}`                                                                                |
| `Failed(Write                                                                 \| ResponseDeadline)`, `Failed(Response(Io))`               | `transport-loss`                                                           | 5         | `{stage:"snapshot"}`                                                                                |
| `Failed(Response(InvalidHeader                                                \| Contract))`, `UnexpectedResponse`                        | `protocol-invalid`                                                         | 5         | `{stage:"snapshot"}`                                                                                |
| `Failed(CorrelationMismatch)`                                                                                                             | `protocol-invalid`                                                         | 5         | `{stage:"snapshot",reason:"correlation-mismatch"}`                                                  |
| outcome `Denied`                                                                                                                          | `denied`                                                                   | 3         | omitted                                                                                             |
| outcome `Unavailable`                                                                                                                     | `service-unavailable`                                                      | 4         | `{stage:"snapshot"}`                                                                                |
| viewer `RequestNotSubmitted`, `OutcomeUnknown(Write\|Read)`                                                                               | `transport-loss`                                                           | 5         | `{stage:"viewer"}`                                                                                  |
| viewer `OutcomeUnknown(InvalidResult\|UnexpectedResult)`, wrong outcome kind                                                              | `protocol-invalid`                                                         | 5         | `{stage:"viewer"}`                                                                                  |
| viewer `OutcomeUnknown(CorrelationMismatch)`                                                                                              | `protocol-invalid`                                                         | 5         | `{stage:"viewer",reason:"correlation-mismatch"}`                                                    |
| viewer outcome `Rejected(AlreadyAttached\|TerminalUnavailable)` or `Unavailable`                                                          | `service-unavailable`                                                      | 4         | `{stage:"viewer"}`                                                                                  |
| viewer outcome `Rejected(FeedbackMismatch)`, record rejected by the projection                                                            | `protocol-invalid`                                                         | 5         | `{stage:"viewer"}`                                                                                  |
| `ReadRows` outcome `RowsRejected(LimitExceeded)`                                                                                          | `rejected`                                                                 | 3         | `{stage:"viewer",operation:"read-history",reason:"limit-exceeded"}`                                 |
| `ReadRows` outcome `RowsRejected` (other producible reason)                                                                               | `service-unavailable`                                                      | 4         | `{stage:"viewer",operation:"read-history",reason}`                                                  |
| `ReadRows` outcome `RowsRejected(InvalidField\|InvalidAcknowledgement)`, mismatched/malformed row-window record                           | `protocol-invalid`                                                         | 5         | `{stage:"viewer"}`                                                                                  |
| launch-read `RequestNotSubmitted`, `OutcomeUnknown(Write\|Read)` (incl. an exhausted shared network budget)                               | `transport-loss`                                                           | 5         | `{stage:"launch-read"}`                                                                             |
| launch-read `OutcomeUnknown(InvalidResult\|UnexpectedResult)`, wrong outcome kind                                                         | `protocol-invalid`                                                         | 5         | `{stage:"launch-read"}`                                                                             |
| launch-read `OutcomeUnknown(CorrelationMismatch)`                                                                                         | `protocol-invalid`                                                         | 5         | `{stage:"launch-read",reason:"correlation-mismatch"}`                                               |
| launch-read outcome `Unavailable(ServerBusy)`                                                                                             | `service-unavailable`                                                      | 4         | `{stage:"launch-read"}`                                                                             |
| `DefinitelyNotSubmitted::RequestNotSubmitted` (command)                                                                                   | `transport-loss`                                                           | 5         | `{stage:"command"}`                                                                                 |
| `DefinitelyNotSubmitted::ConnectionUnavailable`/`RequestEncoding`                                                                         | `internal-failure`                                                         | 1         | omitted                                                                                             |
| command `Rejected` precommit                                                                                                              | `rejected`                                                                 | 3         | `{stage:"command",reason,revision}`                                                                 |
| command `Denied` precommit                                                                                                                | `denied`                                                                   | 3         | omitted                                                                                             |
| command `Unavailable` precommit                                                                                                           | `service-unavailable`                                                      | 4         | `{stage:"command",reason}`                                                                          |
| `OutcomeUnknown` (any reason)                                                                                                             | `outcome-unknown`                                                          | 8         | `{stage:"command",requestId,reason}`                                                                |
| close-status `RequestNotSubmitted`, write/io response failure                                                                             | `transport-loss`                                                           | 5         | `{stage:"close-status"}` under `orchbee.cli.failure.close`                                          |
| close-status invalid/unexpected response, or a scope-mismatched answer                                                                    | `protocol-invalid`                                                         | 5         | `{stage:"close-status"[,reason:"correlation-mismatch"\|"scope-mismatch"]}` under the close envelope |
| close-status response deadline, or the shared 5 s command deadline expiring                                                               | `timeout`                                                                  | 7         | `{stage:"close-status",timeoutMs:5000}` under the close envelope                                    |
| close-status outcome `Denied`                                                                                                             | `denied`                                                                   | 3         | `{stage:"close-status"}` under the close envelope                                                   |
| close-status outcome `Unavailable`                                                                                                        | `service-unavailable`                                                      | 4         | `{stage:"close-status",reason:"server-busy"}` under the close envelope                              |
| close-resume `RequestNotSubmitted`, write/io response failure                                                                             | `transport-loss`                                                           | 5         | `{stage:"close-resume"}` under the close envelope                                                   |
| close-resume invalid/unexpected response                                                                                                  | `protocol-invalid`                                                         | 5         | `{stage:"close-resume"}` under the close envelope                                                   |
| close-resume outcome `Denied`                                                                                                             | `denied`                                                                   | 3         | `{stage:"close-resume"}` under the close envelope                                                   |
| close-resume outcome `Unavailable`                                                                                                        | `service-unavailable`                                                      | 4         | `{stage:"close-resume",reason:"server-busy"}` under the close envelope                              |
| close-resume outcome `Stale`                                                                                                              | `rejected`                                                                 | 3         | `{stage:"close-resume",reason:"stale-generation",attemptGeneration}` under the close envelope       |
| close-resume outcome `Rejected` (typed reason)                                                                                            | `rejected`                                                                 | 3         | `{stage:"close-resume",reason}` under the close envelope                                            |
| close-resume `OutcomeUnknown` (any reason)                                                                                                | `outcome-unknown`                                                          | 8         | `{stage:"close-resume",requestId,reason}` under the close envelope                                  |
| worktree-list `RequestNotSubmitted`, write/io response failure, or the `TransportLoss` outcome (post-delivery response loss)              | `transport-loss`                                                           | 5         | `{stage:"worktree-list"}` under `orchbee.cli.failure.worktree`                                      |
| worktree-list invalid/unexpected response                                                                                                 | `protocol-invalid`                                                         | 5         | `{stage:"worktree-list"[,reason:"correlation-mismatch"]}` under the worktree envelope               |
| worktree-list response deadline, or the shared 5 s command deadline expiring                                                              | `timeout`                                                                  | 7         | `{stage:"worktree-list",timeoutMs:5000}` under the worktree envelope                                |
| worktree-list outcome `NotFound` (directory absent from global navigation at the Server check)                                            | `not-found`                                                                | 3         | `{resource:"directory",target}` under the worktree envelope                                         |
| worktree-list outcome `Denied`                                                                                                            | `denied`                                                                   | 3         | `{stage:"worktree-list"}` under the worktree envelope                                               |
| worktree-list `Unavailable`/`SpawnUnavailable`/`SpawnFailed`/`OwnerIo`                                                                    | `service-unavailable`                                                      | 4         | `{stage:"worktree-list",reason:"server-busy"                                                        | "worker-unavailable"                                           | "job-dead" | "job-unknown"}` |
| worktree-list `GitFailed` (probe/list carry the exact exit status, `null` when signaled) or `MalformedListing`                            | `check-failed`                                                             | 1         | `{stage:"probe"                                                                                     | "list",status}`or`{stage:"parse"}` under the worktree envelope |
| worktree-list `TooLarge` (shared Git stdout, OBCN result, or Workbench result budget crossed)                                             | `output-too-large`                                                         | 1         | `{stage:"worktree-list",origin,limitBytes}` under the worktree envelope                             |
| launch-profile-catalog `RequestNotSubmitted`, write/io response failure                                                                   | `transport-loss`                                                           | 5         | `{stage:"launch-profile-catalog"}` under `orchbee.cli.failure.launch-profile`                       |
| launch-profile-catalog invalid/unexpected response                                                                                        | `protocol-invalid`                                                         | 5         | `{stage:"launch-profile-catalog"[,reason:"correlation-mismatch"]}` under the profile envelope       |
| launch-profile-catalog response deadline                                                                                                  | `timeout`                                                                  | 7         | `{stage:"launch-profile-catalog",timeoutMs:5000}` under the profile envelope                        |
| launch-profile-catalog outcome `NotFound`                                                                                                 | `not-found`                                                                | 3         | `{resource:"launch-profile",target}` under the profile envelope                                     |
| launch-profile-catalog outcome `Ambiguous`                                                                                                | `ambiguous`                                                                | 3         | `{resource:"launch-profile",target,candidates≤8,totalCount}` under the profile envelope             |
| launch-profile-catalog outcome `Denied`                                                                                                   | `denied`                                                                   | 3         | `{stage:"launch-profile-catalog"}` under the profile envelope                                       |
| launch-profile-catalog outcome `Unavailable`                                                                                              | `service-unavailable`                                                      | 4         | `{stage:"launch-profile-catalog",reason:"service-unavailable"}` under the profile envelope          |
| launch-profile mutation `ProfileUnavailable` (settled zero-change outcome on `delete`, `tab create --profile`, `panel split --profile`)   | `not-found`                                                                | 3         | `{resource:"launch-profile",target}` under the profile envelope                                     |
| launch-profile mutation `ProfileAmbiguous` (settled zero-change outcome, same commands)                                                   | `ambiguous`                                                                | 3         | `{resource:"launch-profile",target,candidates≤8,totalCount}` under the profile envelope             |
| send `RequestNotSubmitted` (the request provably never reached the wire)                                                                  | `transport-loss`                                                           | 5         | `{stage:"send"}`                                                                                    |
| send `ConnectionUnavailable`/`RequestEncoding`                                                                                            | `internal-failure`                                                         | 1         | omitted                                                                                             |
| send `OutcomeUnknown` (write/read/invalid-result/correlation)                                                                             | `outcome-unknown`                                                          | 8         | `{stage:"send",requestId,reason}`                                                                   |
| `terminal.send.result` `Rejected`/`Unavailable`                                                                                           | typed answer, not a failure envelope                                       | 3/4       | send result `reason`, `cleanupStatus`; `host-protocol-fault` exits 5                                |
| `terminal.send.result` `Accepted` + `Unconfirmed` cleanup                                                                                 | typed answer, not a failure envelope                                       | 5         | send result `recordsAccepted`, `acceptedEncodedBytes`, `cleanupStatus`                              |
| agent `RequestNotSubmitted`/`Write`/`ResponseDeadline`/`Response(Io)` (deadline not elapsed)                                              | `transport-loss`                                                           | 5         | `{stage:"agent-read",operation}`                                                                    |
| agent exchange or post-admission step with the shared total deadline elapsed                                                              | `timeout`                                                                  | 7         | `{stage:"agent-read",operation,timeoutMs:5000}`                                                     |
| agent armed span captures an eligible SIGINT or SIGTERM (any stage; HUP/QUIT stay raw)                                                    | `cancelled`                                                                | 130       | `{stage:"agent-read",operation}`                                                                    |
| agent `Response(wire fault)`/`UnexpectedResponse`/`CorrelationMismatch`, wrong result kind                                                | `protocol-invalid`                                                         | 5         | `{stage:"agent-read",operation}`                                                                    |
| agent outcome `Rejected(NotFound)` (get/explain)                                                                                          | `not-found`                                                                | 3         | `{stage:"agent-read",operation,sessionId}`                                                          |
| agent outcome `Rejected(ReadDenied)`                                                                                                      | `denied`                                                                   | 3         | `{stage:"agent-read",operation,reason:"read-denied"}`                                               |
| agent list outcome `Rejected(CursorInvalid\|CursorStale)`                                                                                 | `rejected`                                                                 | 3         | `{stage:"agent-read",operation:"list",reason:"cursor-invalid"\|"cursor-stale"}`                     |
| agent outcome `Unavailable`                                                                                                               | `service-unavailable`                                                      | 4         | `{stage:"agent-read",operation,reason:"runtime-unavailable"}`                                       |
| agent `ConnectionUnavailable`/`RequestEncoding`                                                                                           | `internal-failure`                                                         | 1         | omitted                                                                                             |
| send source validation: unopenable/non-regular path, terminal or unsupported stdin, empty, oversized                                      | `invalid-usage`                                                            | 2         | omitted (localized stderr names the declared option and concrete cause)                             |
| send source deadline expired during materialization                                                                                       | `timeout` under `orchbee.cli.failure.source`                               | 7         | `{stage:"input",inputSource,timeoutMs}`                                                             |
| eligible signal captured during send source materialization                                                                               | `cancelled` under `orchbee.cli.failure.source`                             | 128+signo | `{stage:"input",inputSource}`                                                                       |
| send source signal setup/read/restore failure                                                                                             | `internal-failure`                                                         | 1         | omitted                                                                                             |
| prepared output over 8 MiB                                                                                                                | `output-too-large`                                                         | 1         | `{limitBytes:8388608}`                                                                              |
| stdout write/flush failure                                                                                                                | `output-failed`                                                            | 1         | no second stdout document                                                                           |
| `config check` finishes with failures                                                                                                     | success schema `ok:false`                                                  | 1         | five runtime checks preserved; Windows adds user-config                                             |
| `server start` cohort `InvalidData`/`NotFound`/`PermissionDenied`/`TimedOut`/other io kind                                                | `incompatible`/`service-unavailable`/`denied`/`timeout`/`internal-failure` | 6/4/3/7/1 | `{stage:"cohort"}`                                                                                  |
| `server start` socket path over the `sockaddr_un` budget                                                                                  | `service-unavailable`                                                      | 4         | `{stage:"runtime-directory",reason:"address-too-long"}`                                             |
| `server start` connect/admission failure                                                                                                  | the `ConnectClientError` rows above, verbatim                              | 4/5/6     | `{stage:"admission"}`                                                                               |
| `server start` marker or sockets committed without a live owner; Server spawn or `try_wait` io failure                                    | `rejected`                                                                 | 3         | `{stage:"activation",reason:"recovery-required"}`                                                   |
| `server start` activation deadline elapsed; lock acquisition `TimedOut`                                                                   | `timeout`                                                                  | 7         | `{stage:"activation"}`                                                                              |
| `server start` marker/probe/lock io failure (non-timeout)                                                                                 | `internal-failure`                                                         | 1         | `{stage:"activation"}`                                                                              |
| `server diagnose` complete or partial fact document — unsafe/uninspectable rows are facts, not failures                                   | typed `orchbee.site.diagnose` document                                     | 0         | row `state`/`reason` fields                                                                         |
| `server diagnose` outside Linux/Windows; network query, mutation, `server start`, `config check`, or `agent doctor` outside Linux/Windows | `service-unavailable`                                                      | 4         | `{stage:"platform",reason:"unsupported-platform"[,command]}`                                        |
| `agent doctor` captures an eligible signal between probe items                                                                            | `cancelled`                                                                | 130       | `{stage:"agent-doctor"}` — partial documents arise only from budget exhaustion                      |
| `terminal wait-output` deadline exhausted without a match                                                                                 | `timeout`                                                                  | 7         | `{stage:"wait",timeoutMs,observed}`                                                                 |
| HUP/INT/QUIT/TERM during `terminal wait-output`                                                                                           | `cancelled`                                                                | 130       | `{stage:"wait",cleanup}`                                                                            |
| `terminal wait-output` match with unconfirmed `Detach`                                                                                    | typed viewer-stage failure                                                 | *         | `{stage:"viewer",matchObserved:true,cleanup:"unconfirmed"}`                                         |

`resource` is `project|workspace|tab|panel|terminal|schema`; `stage` is
`cohort|runtime-base|runtime-directory|socket|discovery|admission|activation|snapshot|viewer|command|send|agent-read|agent-doctor|platform|wait`.
The dedicated `orchbee.cli.failure.worktree` envelope admits `resource:"directory"` and the
`worktree-list|probe|list|parse` stage vocabulary documented with its schema above — those
values belong to that envelope only, not to the generic `orchbee.cli.failure` vocabulary.
The dedicated `orchbee.cli.failure.close` envelope admits the `close-status|close-resume`
stage vocabulary documented with its schema above — those values likewise belong to that
envelope only.
The source-stage `input` stage lives only in the `orchbee.cli.failure.source` envelope —
it is not admitted into the standard `orchbee.cli.failure` stage vocabulary.
Stages and reasons are finite schema values; no exception text becomes a machine field. `details`
is a closed per-kind vocabulary — `{resource,target}`,
`{resource,target,candidates≤8,totalCount}`, `{stage[,reason]}`,
`{stage:"command",reason,revision}`, `{stage:"command",requestId,reason}`,
`{stage:"send",requestId,reason}`,
`{stage:"viewer",operation:"read-history"|"wait-output",reason}`,
`{stage:"wait",timeoutMs,observed}`, `{stage:"wait",cleanup}`,
`{stage:"viewer",matchObserved,cleanup}`, `{limitBytes}`,
`{stage:"platform",reason,command?}` (the `command` member names the refusing check
command, `"agent doctor"`), or the agent-read family —
`{stage:"agent-read",operation}`, `{stage:"agent-read",operation,sessionId}`,
`{stage:"agent-read",operation,reason}`, `{stage:"agent-read",operation,timeoutMs}` —
and the embedded schemas reject documents outside those shapes. Read-path errors never become
outcome-unknown/failed-effect: only the command stage can produce an undetermined effect, and
`rejected`/`denied`/`unavailable` precommit answers are definite — the Server applied nothing.
The send stage follows the same rule for its single input record: a `RequestNotSubmitted` is
definite non-submission, an `OutcomeUnknown` means the record may already have landed, and the
typed `terminal.send.result` answers are the Server's own verdicts, never a client guess. A
declared `--file`/`--stdin` source is stricter still: every one of its failures happens before
any runtime access, so it can never produce `outcome-unknown`, and a captured-signal
cancellation or an expired shared deadline exits without a wire request ever existing.

## Output contract and side effects

- stdout carries only the contract document: one pretty (2-space) JSON document ending in `\n`
  under `--format json` (and always for `api` commands), or the localized human document under
  text/default. stderr carries localized human diagnostics; query paths print no progress.
- `invalid-usage` failures write one localized stderr diagnostic naming a single concrete
  cause — an unknown command or option, an option or argument the command does not take, a
  duplicated option, conflicting options, a missing option value, a missing required option
  or operand, a missing command, an unknown help path, a misplaced help/version flag, an
  invalid option value or geometry set, or a non-Unicode/oversized argument list — and end
  with a help hint. The hint names `orchbee <path> --help` for a recognized command or
  family context and falls back to `orchbee --help` otherwise; every emitted hint is a real
  path that exits 0 without a Session. Echoed untrusted tokens reuse the quoted/escaped
  rendering below, first cut to a 256-Unicode-scalar prefix marked with an ellipsis when
  more scalars remain; non-Unicode or oversized
  argument lists are never echoed. The machine envelope stays unchanged — the diagnostic
  lives only on stderr.
- Text renders user-controlled strings as quoted escaped values: backslash and quote use `\\`/
  `\"`; C0/C1 controls (including ESC), U+2028/U+2029, and bidi formatting controls use
  unambiguous `\uXXXX` escapes; other Unicode is preserved. List tables truncate only label
  display to 80 display columns with an ellipsis while keeping escapes and the closing quote
  intact; IDs are never truncated; `get` field values are complete. JSON preserves source data
  with standard lossless encoding — never the TUI replacement-character sanitizer.
- The full output is constructed in a bounded buffer before the first stdout write; preparation
  over 8 MiB fails `output-too-large` without touching stdout. A real OS write failure may leave
  partial bytes but never appends a second document; stderr still reports and the exit is nonzero.
- Machine output is locale-neutral and contains no English `message`; stderr diagnostics are
  localized en-US/zh-CN. Machine `kind`, field names, and enum values are never localized; user
  label/path data is preserved as data.

## Invariants

- A syntactically invalid invocation never touches the filesystem, cohort, or socket.
- Offline commands (`help`, `-v`/`-V`/`--version`, `--skill`, `completion`, `api schema`, and Windows `--default-config`) never perform platform,
  runtime, cohort, or Session discovery.
- A network query creates nothing: no directory, lock, marker, terminal, viewer, control grant,
  or second request — except `terminal read`/`terminal wait-output`, whose read-capability
  viewer attachment is the single sanctioned read-path side effect, held minimally and
  released before output. At most one short-lived Client Connection is held, and none survives
  into output.
- A mutation submits at most one `workbench.command` per invocation under a fresh
  `WorkbenchRequestId`; it is never retried or replayed, and an ambiguous outcome is reported
  as `outcome-unknown`, not guessed. `terminal send` follows the same discipline for its one
  OBCS `terminal.send` exchange: one bounded record sequence per invocation under a fresh request
  identity, never retried — an ambiguous answer is `outcome-unknown`, and the localized
  diagnostic says the input may already have landed before the operator decides whether to
  repeat it.
- A declared `terminal send` source is materialized completely before any runtime access:
  one bounded input sequence (text 4096 bytes, opaque bytes 1 MiB, paste 256 KiB) feeds
  exactly one `terminal.send` request — never split, replayed, or partially submitted. A
  source timeout, cancellation, or validation failure produces zero runtime effect and no
  request identity; after materialization the ordinary send lifecycle is authoritative and
  a source-stage outcome cannot semantically cancel an already admitted send.
- On Linux, the source signal guard never alters dispositions and restores the inherited thread mask
  exactly — captured, inherited-blocked, and inherited-ignored signals each keep their
  original semantics; no signal owner outlives the source stage, and no eligible signal
  stays blocked past the output finalization path.
- There is at most one snapshot read per invocation, one admission per invocation, and one
  network deadline covering read and command; `terminal read`'s viewer exchanges and
  `terminal send`'s single send ride on that same session and deadline — never a second
  admission or a renewed budget. Agent target resolution and reads follow the same
  discipline: each exchange has a fresh `ObapRequestId` and one correlated result, the session
  retired on any ambiguous outcome and never replayed — and no Workbench snapshot is
  taken at all.
- Agent observation never fabricates: results are the wire outcome verbatim, a `found`
  `explanation` is the Server's canonical 11-field reading, and success is never inferred
  from terminal contents or screenshots. An `agent-read` source exists only after a
  trustworthy correlated answer; transport, timeout, and protocol-invalid failures carry
  none.
  `terminal wait-output` keeps one admission and bounds each I/O to five seconds and any
  remaining total `--timeout` budget. Source-mode `--forever` omits only the total ceiling;
  per-attachment `requestSequence` strictly increases from 1 across every `read-rows`.
- A match proves only that the text was found inside the searched window during the
  observation — never command completion, a new turn, or the absence of other output; output
  scrolled past Host retention between polls is unobservable to every consumer.
- No result field exists without a trusted source; `null` means the source says "none" and
  `"unknown"` means the source cannot say — the two never mix.
- Failure envelopes never invent `source`: siteId/revision appear only from a validated snapshot.
- Ambiguous never guesses: it reports bounded candidates plus the true count.
- `api schema`, help, and completion enumerate exactly the commands delivered in the same build.
- `server start` is the single activation surface and the one command that creates local-runtime
  state: it may create the runtime directory, hold activation/lifetime locks, commit the marker,
  and spawn the Server — only a grammar failure guarantees zero effect. Its absolute 20-second deadline
  is never reset per stage; a returned `recovery-required` refusal is never retried or force-cleaned;
  Windows cold recovery rechecks the separately documented proof guards before a fresh generation.
  A timeout never claims no Server started. `server diagnose` is the observation
  surface and performs no effect at all beyond its bounded lifetime-lock probe: no connect,
  no create, no lock acquisition, no repair, and no liveness or restart claim.
- The CLI-owned 5-second budgets are checked deadlines; a construction failure is
  `internal-failure`, while transport-internal `RequestNotSubmitted` stays `transport-loss`.

## Scenario matrix and evidence

| Scenario                                                                                                                                                                                                          | Expected                                                                                          | Evidence                                                            |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| Exhaustive grammar: formats, positions, `--`, help forms, duplicates, invalids                                                                                                                                    | exact parse/failure kinds                                                                         | `src/invocation` tests                                              |
| Format envelope ordering: a defect anywhere leaves stdout empty; a valid json anywhere shapes later failures                                                                                                      | envelope precedence over grammar errors                                                           | `src/invocation` tests + `tests/cli_query.rs`                       |
| Filter scope: `workspace list` accepts only `--project`; wrong-command filters are usage before runtime                                                                                                           | complete matrix, no silently dropped filter                                                       | `src/invocation` tests + `tests/cli_query.rs`                       |
| Target rules: ID/label/zero/overlong/ambiguous precedence                                                                                                                                                         | resolver verdicts                                                                                 | `src/cli` resolver tests                                            |
| Validated snapshot → every result shape, empty collections, nulls                                                                                                                                                 | exact JSON/text fixtures                                                                          | `src/cli` output tests + JSON Schema validation (dev `jsonschema`)  |
| Schema dictionary ↔ emitted envelopes                                                                                                                                                                             | every success/failure fixture validates; rejected variants fail                                   | schema contrast tests                                               |
| API failure format: text/default failures keep stdout empty, `--format json` yields the envelope                                                                                                                  | caller-selected failure format on `api schema`/`api snapshot`                                     | `tests/cli_query.rs` + `src/cli` tests                              |
| No Session on disk                                                                                                                                                                                                | `service-unavailable`, zero fs/process/lock mutation                                              | `tests/cli_query.rs`                                                |
| Real staged-cohort Server: status/list/get/api snapshot, 8 admitted + ninth rejected + release, live TUI                                                                                                          | success/`connection-capacity`/unchanged structure                                                 | `tests/local_startup_process`                                       |
| Scripted OBLC/OBWP peers: denied, unavailable, malformed, wrong correlation, timeout, disconnect                                                                                                                  | typed kinds per matrix; never labeled real-Server evidence                                        | `tests/cli_query.rs`                                                |
| Scripted OBCS viewer peer: attach-binding fields, `Empty`→`record` poll, detach release, reject/unavailable/malformed/correlation/disconnect                                                                      | `terminal read` success + typed viewer-stage failures                                             | `tests/cli_query.rs`                                                |
| Scripted OBCS viewer peer + hand-built OBRW records: scrollback pricing, capacity/scrollback clamps, typed `rows-rejected`, aged-pin re-pin, window mismatch, oversized                                           | retained-row projection success + typed failures                                                  | `tests/cli_query.rs`                                                |
| Scripted OBCS send peer: UTF-8 text with optional folded CR and semantic keys arrive as one request, target/request identity, accepted/rejected/unavailable/unknown answers, stalled answer, no retry             | `terminal send` exit 0/3/4/5/8 with the two-axis result fields                                    | `tests/cli_send.rs`                                                 |
| Send source grammar: four-source exclusivity, fixed-order conflict naming, `--enter` scoping, flag scope                                                                                                          | exact parse/`invalid-usage` before any runtime access                                             | `src/invocation` tests                                              |
| File/stdin materialization: nonblocking open + `fstat` type gate, regular-file shared offset, private FIFO reopen, cap+1, opaque bytes, empty/non-regular/unsupported rejections                                  | bounded record or typed source `invalid-usage`; zero runtime effect                               | `src/cli/source` unit tests + `tests/cli_send.rs`                   |
| Source deadline/cancellation: stalled pipe exits at the shared deadline (7), captured eligible signal exits `128+signo` under `orchbee.cli.failure.source`, inherited IGN/BLK preserved, restore failure internal | source envelope facts, no wire request, exact mask restore                                        | `src/cli/source` tests + labelled OS/process evidence               |
| Scripted OBCS send peer with declared sources: materialized file/stdin arrives as one `terminal.send` text request                                                                                                | identical envelope to `--text`; exactly one request                                               | `tests/cli_send.rs`                                                 |
| Windows owned pipe reader: raw bytes, EOF, empty input, byte/paste boundaries, deadline, injected cancellation, child retirement                                                                                  | exact payload or typed refusal; no surviving pipe reader                                          | `src/cli/source_windows/pipe.rs` tests                              |
| Windows public process and PowerShell 5.1/7 pipeline source validation                                                                                                                                            | source completion reaches the text control's same post-source refusal; invalid sources fail first | `tests/cli_windows_input.rs`                                        |
| PowerShell 5.1/7 completion and offline localized skill                                                                                                                                                           | scoped candidates, repeated keys, paste exclusions, sole-root skill without runtime access        | `src/cli/completion/powershell/tests.rs` + `tests/process.rs`       |
| Scripted OBCS viewer peer + hand-built OBTF/OBRW records: increasing `requestSequence`, visible/history boundary window, real `FrameApplied` per frame, stale re-pin, excerpt tail match, deadline incl. preamble | `terminal wait-output` success + typed failures                                                   | `tests/cli_query.rs`                                                |
| Scripted OBLC/OBAP peers: list/get/explain verbatim outcomes, option→wire echo, denied/not-found/cursor/unavailable answers, malformed/wrong-kind/miscorrelated frames, no-retry                                  | agent read success + typed `agent-read` failures                                                  | `tests/cli_agent.rs`                                                |
| Agent envelopes ↔ embedded schemas: emitted success/failure documents validate; wire-impossible variants (count 129, duplicate capabilities/ceiling) rejected                                                     | schema restatement of the wire contract                                                           | `tests/cli_agent.rs`                                                |
| Real staged-cohort Server + live TUI terminal: `wait-output` matches existing/later/scrolled-off/UTF-8 output, short deadline exits 7, cancel frees the slot                                                      | public wait observation coexisting with the TUI's own viewer                                      | `tests/local_startup_process`                                       |
| Real staged-cohort Server + live TUI terminal: `terminal read` returns its grid and schema-validates                                                                                                              | real read-only observation coexisting with the TUI's own viewer                                   | `tests/local_startup_process`                                       |
| Mutation grammar: every command, target rules, `--confirm`, geometry bounds, flag scopes                                                                                                                          | exact parse/failure kinds before runtime access                                                   | `src/invocation` tests                                              |
| Mutation scripted peers: all fifteen commands, precommit verdicts, ambiguous and unapplied outcomes                                                                                                               | one command per call; `outcome-unknown` never replayed                                            | `tests/cli_mutation.rs`                                             |
| Workspace close grammar: `close` target+`--confirm`; `close-status`/`close-resume` strict stable IDs, `--request`/`--generation` fences, stale-but-valid generation parses                                        | exact parse/`invalid-usage` before runtime access                                                 | `src/invocation` tests                                              |
| Workspace close envelopes: close-status verbatim outcome incl. tombstone, resume source echo, closed close-envelope details                                                                                       | every fixture validates; rejected variants fail                                                   | schema contrast tests                                               |
| Worktree list grammar: `--directory` only; Project rejected, strict directory-ID value rule, flag scopes                                                                                                          | exact parse/`invalid-usage` before runtime access                                                 | `src/invocation` tests                                              |
| Launch-profile grammar: four command paths, verbatim selector, `--name`/`--exec` required pair, `--profile` exclusivity and scope                                                                                 | exact parse/`invalid-usage` before runtime access                                                 | `src/invocation` tests                                              |
| Launch-profile document/failure envelopes: entry member names, closed details vocabulary, dual `source` shapes                                                                                                    | every fixture validates; rejected variants fail                                                   | schema contrast tests                                               |
| Worktree list document/failure envelopes: entry XOR byte fields, bare/detached/HEAD invariants, closed details vocabulary                                                                                         | every fixture validates; rejected variants fail                                                   | `tests/cli_query.rs` schema contrast tests                          |
| Worker discovery job: real disposable Git repository (init + commit + detached linked worktree, per-command `-c` identity only), shared TTL/stdout budget attribution                                             | verbatim `-z` bytes, real HEAD/ref, typed outcomes                                                | `apps/orchbee-worker` `worktree_discovery_job` tests                |
| Server worktree-list handler: `-z` parser acceptance/rejection matrix, membership authority, outcome→failure mapping                                                                                              | byte-exact projection; malformed listings are `check-failed`                                      | `apps/orchbee-server` `worktree_list` tests                         |
| Non-TTY; stdout blocked after the session is gone; >8 MiB prepared output on every format; real write failure                                                                                                     | documented exits; transport owners dropped before blocking                                        | `tests/local_startup_process` + emit unit tests                     |
| `config check` matrix: pass/fail/not-applicable per check, `exists` semantics                                                                                                                                     | five runtime checks; Windows additionally checks user configuration                               | `tests/cli_query.rs` + `src/cli/config` tests                       |
| `agent doctor` grammar: provider/asset-root admission, dedup, caps, help paths; synthetic fixture → total-order verdicts (codex ladder, guarded/capsule/root/socket probes), closed vocabularies                  | deterministic offline facts; emitted document validates; no provider execution or socket connect  | `src/invocation` tests + `src/agent_doctor` tests                   |
| `server start` real staged cohort: cold start spawns + admits, second/overlapping starts, marker-without-owner refusal, held-lock deadline, cohort faults, signals, non-UTF-8 base, zh-CN text                    | `started`/`already-running` truth; refused wedge leaves marker byte-identical; bounded deadline   | `tests/local_startup_process`                                       |
| `server diagnose` grammar + observation matrix: format-only options, every row state/reason, conditional advisories, held/free/unsafe lifetime lock, unadmitted parent, spent budget, zero creation               | exact parse; closed fact rows; no connect/create/repair; no liveness claim                        | `src/invocation` tests + `src/cli/site_diagnose` + rendezvous tests |
| Completion scripts project the grammar (bash/zsh executed, fish structural parity)                                                                                                                                | positive and negative contexts; no runtime enumeration                                            | `tests/cli_query.rs`                                                |
| Bilingual text/failure output                                                                                                                                                                                     | exact en-US/zh-CN strings                                                                         | `tests/process.rs` + `tests/cli_query.rs`                           |

This slice adds no Server state machine, persistence or retry. `terminal read` uses the existing `terminal.viewer` attach/poll/detach lifecycle; source selection adds a bounded `read-rows`/`rows-rejected` exchange, and `terminal wait-output` repeats poll plus one bounded `read-rows` per applied frame. The command settlement obligations—flag sampling, cleanup provenance and the late sample—use the implementation tests in [CLI wait evidence](native-cli-wait-model.md). Shared protocol changes require their affected retained cores; inventory output alone is not execution evidence.

## Per-launch cwd and environment

`tab create` and `panel split` accept an absolute `--cwd` and repeated literal
`--env NAME=VALUE`, independently of default shell, `--exec` or `--profile` selection.
Explicit cwd overrides the accepted Workspace directory only for that launch; if neither
is present the existing Worker-home default applies. Placement, Workspace membership,
and stored profiles remain unchanged. The Host checks the actual directory and reports
the existing failed-launch result without creating a directory or substituting a fallback.

The environment overlays inherited/default values. It admits at most 32 pairs and 1024
combined UTF-8 name/value bytes; names are ASCII identifiers of 1–148 bytes. NUL and
case-insensitive duplicate names are rejected. Empty values and literal equals signs in
values are allowed. `ORCHBEE_*`, `TERM`, `COLORTERM` and `TERM_PROGRAM` are reserved,
ignoring ASCII case. There is no shell expansion, profile mutation or global environment
mutation. Usage diagnostics and debug projections omit environment values.

Options participate in request/replay identity. Pending Tab and split operations verify
resolved cwd and environment before commit and on completion. Empty options retain the
legacy JSON and binary encoding. Nonempty OBCN environment uses a zero executable-length
sentinel (invalid in the legacy layout), extension version 1, optional explicit selection,
and bounded length-prefixed pairs. New decoders accept legacy payloads; old decoders reject
the extension rather than discard it. Rollout uses matching cohort hashes. Combined maximum
cwd, argv and environment remain within the 40,960-byte frame bound.

Evidence covers constructor/decoder rejection, legacy and extended roundtrips, combined
maximum sizes, CLI scope and redaction, Server composition and replay identity, and a
Windows child observing cwd/environment after public Tab and split commands. No new
macOS/Linux adapter or execution claim is included; This workflow uses implementation tests.

Native launch commands use a dedicated 96 KiB JSON request budget so maximum escaped cwd, argv, label and environment fit together. OBWP command kind 2 admits this bound before allocation; non-launch intents retain 16 KiB and results retain their existing bounds. OBCS v1 frames admit 63 KiB; OBCN admits 40 KiB. The OBCS v1 schema/corpus covers these launch values; shared Workbench schemas and the Desktop binding still do not establish support for the native extensions.

## Calling Panel context

`panel current [<panel-id-or-selector> | --current] [--client <id>]` reads a
Windows Panel using the [current Panel contract](native-current-panel.md), shared with
`panel process-info`. Explicit Panel selectors ignore caller environment and need no Client.
Explicit `--client` ignores caller environment and conflicts with explicit Panel selectors.
Otherwise, only two absent caller variables permit unique current Client fallback; partial,
malformed, or stale caller information fails. Current Panel reads accept Ready or Busy Clients;
process-info retains Ready-only selection. The Panel must still belong to the reported Context and Tab. Ambiguous Client selection requires `--client`.

The Windows Worker writes `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID` from admitted
launch authorization into child environments, overriding inherited conflicts. These product
identities are lookup hints, not credentials; both must be complete nonzero lowercase 32-hex
IDs. Malformed, empty, non-Unicode, or partially missing values fail `invalid-usage` (exit 2),
with localized guidance and no value echo. Explicit Panel or Client mode bypasses this metadata.

One validated snapshot owns Panel membership. A valid Terminal/Worker binding resolves its
containing Panel, including child Presentations and Panels moved to another Tab or Workspace.
An absent or mismatched binding fails `not-found` (exit 3); duplicate identity fails
`protocol-invalid` (exit 5). Invalid caller binding never falls back. Snapshot and optional
Client observations are not atomic. The answer is bounded by their observation times.

Windows current Panel uses `orchbee.panel.current` v1 with an explicit `currentPresentation`:
primary for an explicit Panel, exact caller terminal, or the selected Client member. Agent,
terminal title/cwd/processes, reports, metadata and scroll share this member while canonical
primary/child structure stays intact. Explicit/caller selection stays headless. The original
Client view/revision and local member token are fenced; changes fail without a partial result. The
five-second deadline, cancellation, read-only operations, Viewer cleanup and Windows evidence
are owned by the linked contract. The offline catalog lists the current documents and command paths. No activation, input,
control-authority or focus effects; non-Windows current retains v1. This workflow uses implementation tests.

Windows Worker identities are fresh per Server connection using the existing OS random source;
one Worker shares that ID across its terminals. Terminal IDs can repeat across Sessions, so exact
Worker matching rejects callers from a previous or different Session. No reconnect or adoption is
introduced. Tab close acceptance is not removal: lookup reflects membership until the Server
publishes removal, after which the old binding is not found.

## Directory Workspace creation

The Windows `workspace create` [contract](windows-workspace-creation.md) owns optional Project, label, source, directory and focus. Its default creates an ungrouped Workspace and default shell, with no Project/label form or automatic AI launch. Explicit advanced options remain available. `orchbee.workspace.creation` v1 records Workspace, directory provenance and optional focus, with no Project setup phase. IDs and terminal facts live in `workspace.result`, including required-nullable `projectId`.

The Worker probes the requested directory before the Server atomically registers the Workspace, directory, Terminal Context and first Tab/Panel/Presentation at one revision. Failed admission creates none of them. A successful commit retains the nested mutation v1 envelope with action `workspace.create`, `projectId`, `workspaceId`, `directoryId`, `terminalContextId`, `tabId`, `panelId`, `presentationId`, literal `path`, `label`, and `terminal`. Project metadata is required-nullable; catalog v1 publishes this changed contract. Terminal launch can remain pending or fail after commit: keep the returned IDs, inspect the Workspace and explicitly close it if unwanted. An unknown response is never automatically replayed or rolled back.

Native `workspace.launch` uses tagged placement: `ungrouped`, `project`, `new-project` or `alongside`. Ordinary CLI creation selects ungrouped or an explicitly resolved existing project, with label, geometry and required cwd; launch/profile remain mutually exclusive. Server receipts carry exact structural IDs and nullable Project metadata; pending/launched preserve admission and terminal settlement separately. Activate and roll back one complete matched cohort.

## Agent Panel targeting model

Targeted Agent commands accept `--panel <id|@panel:0x<hex>>` exclusively with positional,
`--agent-session` and `--name` selectors. A positional full Panel ID first resolves as an exact
retained Session ID or runtime name; only a definite not-found permits Panel fallback.
Explicit Session/name selectors never fall back. A prefixed `@panel:` target selects a Panel
without consulting aliases. Workbench owns Panel identity and its primary presentation;
Agent state owns live, navigable presentation attachments. Hidden attachments remain eligible.
The CLI reads one Workbench snapshot on the held Client Connection and passes the primary
presentation ID to the existing OBAP names family. Its presentation selector resolves,
assigns or releases under the Agent state fold: zero live matches is not-found, one is
selected, and multiple sessions are ambiguous. Repeated attachments of one session count
once. No paging, first-match choice, terminal-text inference or client-side alias table is used.

The workflow is parse → existing-only admission → optional exact ID/name resolution →
Panel snapshot resolution → OBAP presentation resolution/operation → pinned Session read,
wait or terminal operation. It shares the caller's deadline and cancellation wake; failure
before the final names mutation is a read failure, not an unknown mutation. A lost mutation
reply remains outcome-unknown with no replay. Waiters never follow a later Panel occupant.
Terminal operations retain the selected primary presentation and reject a missing binding
or changed Agent attachment before sending; Server terminal authorization remains authoritative.
Workbench and Agent reads are sequential samples, not a cross-owner atomic transaction.
Session/name behavior and existing receipt sources remain unchanged. Resolve receipts may
report `resolvedBy: presentation`; ambiguity is a bounded rejection without a candidate list.

Native grammar and Panel projection tests cover selector exclusivity and primary/child
separation; Agent fold tests cover zero/one/multiple sessions, ended/non-navigable/hidden
attachments and atomic rename refusal. Windows process tests cover the shared admission,
ID/name precedence, pinned waits, pre-mutation read failures and exact terminal selection.
Real Session/Worker/ConPTY evidence covers the integrated command path. These checks do not
establish provider foreground ownership or readiness. This workflow uses implementation tests.

## Panel process information

### Windows interpretation and Agent input preflight

`src/cli/processes.rs` owns a bounded, stateless interpretation of the validated Host
sample. The Host alone owns process collection. Each process adds nullable `argv` and
`providerId`; `foreground` reports `kind`, nullable `pid`, and nullable `providerId`.
The process result is version 1. Windows `CommandLineToArgvW`, through the existing
safe `winsafe` shell binding, interprets nonempty command lines of at most 4096 UTF-16
units without NUL. Empty or unavailable commands remain null; there is no substitution
of the CLI's own command line, shell expansion, file lookup, or environment access.
These are Windows argument parsing rules, not proof of another executable's parser.
The Windows-only feature adds no crate, version, build script, or process execution.

The executable image must identify an Agent directly, or identify a known runtime
whose actual script entrypoint identifies one. Eval/command payloads and arbitrary
later arguments never identify an Agent. Known package entrypoints and executable
aliases are explicit. Provider matching is a diagnostic heuristic, not executable
authentication. It does not create canonical Agent state or establish readiness.

| Sample interpretation                                 | Foreground projection                | Input preflight                        |
| ----------------------------------------------------- | ------------------------------------ | -------------------------------------- |
| One Agent candidate is an ancestor of every candidate | `agent-chain`, selected PID/provider | Expected canonical provider must match |
| Several unrelated candidates                          | `ambiguous`, null PID/provider       | Refuse                                 |
| No candidate, retained root is present                | `root`, root PID, null provider      | Refuse                                 |
| Root absent and no unique candidate                   | `unavailable`, null PID/provider     | Refuse                                 |

Parent edges only connect sampled processes whose parent creation time is no later
than the child's. Traversal is bounded by the sample size and detects cycles; a reused
parent PID cannot manufacture an ancestor. This adopts Herdr's Windows ancestor-chain
selection intent from `e0507237`, within Orchbee's retained Job, without global scans
or the Git Bash environment-marker escape fallback. Ambiguity stays explicit rather
than looking like an available shell. No POSIX foreground process group is claimed.

For canonical session targets on Windows, `agent send-keys` and `agent prompt` resolve the canonical session and its
exact terminal, query this terminal on the same admitted session, check the expected
canonical provider ID (not an executable alias), then re-read the pinned Agent before sending. The second read must preserve
provider, live lifecycle, and the selected navigable presentation and its generation. Prompt freshness
and blocker checks use this latest observation as well. All steps share the existing
five-second input deadline; each viewer has a detach attempt before the next step, with session
drop as cleanup fallback. A preflight failure sends no input. The low-level `terminal
send` remains available for explicit terminal input. The checks and later input are
not atomic: a process can exit after observation. Existing send receipts and unknown
outcome rules still apply; there is no retry or process lease.

Runtime mismatches use `orchbee.cli.failure.agent-runtime` v1 with the correlated
process source and bounded `stage/reason/sessionId/expectedProvider/detectedProvider`
details. Unknown providers, absent Agents, ambiguous chains, provider mismatch, and
changed canonical binding are distinct refusals. Collection and transport failures
retain their original typed failure. Verification covers Windows Unicode/quoting,
empty-command protection, runtime eval decoys, ancestor/cycle/reused-parent cases,
closed result schemas, real CLI/named-pipe preflight ordering and no-input refusals,
and a real isolated Windows Job witness. These checks do not prove provider task
completion, authentication, or atomic foreground ownership.

`panel process-info [<panel-id> | --current] [--client <id>]` reads an exact Terminal's Windows Job. A positional Panel target and `--current` are exclusive; complete IDs and `@panel:0x<hex>` use the existing Panel resolver and primary binding. Without an explicit selector, valid `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID` retain the caller's exact primary or child Terminal; partial or stale metadata fails. With absent caller metadata, require one current Client; `--client` selects an exact current Client and ignores caller metadata. Client selection passively resolves its actual member and validates the same binding and local attachment token before publication. The [current Panel contract](native-current-panel.md) owns revision fences, deadline and cancellation. No unavailable member falls back to a primary terminal.

One existing-only Client Connection and one absolute five-second budget cover snapshot, transient read-only viewer admission, one process read, and best-effort detach. Client Connection drop owns remaining viewer cleanup. The Server revalidates Worker generation and physical terminal binding; Worker and Host retain process authority. Success publishes the exact typed Windows Job sample with Panel/Terminal routing context and correlated read provenance. The sequential snapshot and sample do not claim an atomic cross-owner observation. Host failures remain explicit, are never empty process success, and are not retried. Strings are escaped in text output; machine creation times remain decimal strings. The response describes owned processes, not a Windows foreground process group or Agent interactive readiness.

The JSON envelope is `orchbee.panel.process-info` v1; the schema catalog and bilingual help/completion ship with the command. Evidence covers implicit and explicit targets, conflicts before I/O, primary-versus-child selection, absent primary bindings, denied reads, wrong response identities, preserved Host errors, bounded output, viewer cleanup, and the real Windows Session/Worker/Host process route. The shared typed read is also the actual dependency for subsequent Agent launch/readiness work; it does not close those obligations by itself.

Expected Host refusals use `orchbee.cli.failure.process-info` v1 with the bounded failure code; invalid responses and transport failures retain the generic CLI failure contract. Successful `source` records snapshot revision, OBCS request ID, and Host correlation, epoch, attachment, and physical Terminal ID as distinct identities. The catalog lists the current documents and command paths.

## Windows Agent startup

[Windows Agent startup](windows-agent-start.md) owns `agent start`, its reservation and readiness workflow, Windows quoting, budgets and recovery. `agent names` v1 includes separate pending reservations; names failures use v1 for launch-pending. The catalog exposes the current documents and command paths.

## Windows Session stop

The managed Windows Server saves committed Workbench changes after a 500ms debounce, refreshes sampled per-terminal directories and provider restart references every 30 seconds, and performs a fresh final save before clean-stop terminal disposal. A later admitted start restores layout/Launch Profiles and launches fresh default shells or selected provider resumes. Saved terminal cwd precedes workspace/home defaults; a missing saved directory falls back to Worker home. Save failure retains preceding valid snapshots, invalid originals and live terminals; explicitly retry after resolving the failure. Startup retains unreadable snapshots and falls back to an older valid copy, or opens an empty workspace with a warning when none remain valid. The owning [checkpoint contract](../../orchbee-server/docs/workbench-checkpoint.md) defines compatibility and recovery limits. Old processes, live Agent state, Client views, terminal screens and history are not restored.

`server stop --confirm [--request-id <32-hex>] [--retry] [--detach] [--timeout <ms>]` stops the existing managed Windows Session without activating one. The ID is a nonzero lowercase 128-bit value, generated once if omitted; `--retry` requires it. The CLI issues one start/retry request, then only status reads. `--detach` returns the admitted attempt immediately; otherwise one absolute deadline (65000 ms by default, 1–3600000 ms) covers runtime/cohort admission, request and polling. Cancellation or timeout stops waiting, never rolls back the admitted shutdown. Every runtime answer retains the request ID so `server stop-status <id>` can query it. Status is a fact document, not a second mutation. The `orchbee.site.stop` v1 envelope distinguishes stopped, pending, failed, busy, not-recorded, unmanaged and unavailable/unknown transport outcomes. Every answer includes `wait`: `null` when no wait disposition exists, or detached, timed-out or cancelled for the applicable pending/transport outcome. Known Server failures preserve attempt, phase, cause and per-attempt disposal count. Only the matching strict stopped activation record plus a free existing lifetime lock proves completion; disappearance, invalid records and a different ID never do. A later start consumes this receipt; not-recorded cannot disprove an earlier stop. Linux/macOS return unsupported-platform without runtime effects.

## Windows retained Worktree operations

The [Worktree contract](windows-worktrees.md) owns `worktree create/open/remove/status/resume`, their source/Project inference, per-command flags, operation identity, confirmation, waiting, output and recovery. It extends this record's earlier list-only grammar: Create/Open admit optional --client, --focus/--no-focus, label and terminal geometry, source Workspace or cwd, and path/branch selectors; Create also admits base. Remove admits force and requires Workspace plus confirmation; Resume requires operation ID, generation and confirmation. All five admit text/JSON; only mutation commands admit request identity, trust and waiting as documented there. The `orchbee.worktree.operation` v1 envelope includes nullable original Project metadata, optional focus and shell restoration receipts, without a Project preparation phase. The offline catalog publishes its current schema.

Windows `worktree list` additionally accepts a source Workspace/cwd and optional command-local trust, with validated caller/selected-Client defaults and cwd only when both are absent and no Project requirement. The earlier registered-directory grammar and envelope in this record apply only to the registered `--directory` form (v1, no Project). The [source discovery contract](windows-worktrees.md#source-based-discovery) owns the separate `orchbee.worktree.query` v1 result (catalog v1), mutual exclusions, total deadline, failure exits and known Workspace associations.

## Project close-all projection

Windows `project close-all <target> --confirm` explicitly closes captured members of one Project, with `--request-id`, `--detach` and `--timeout` (1–3600000 ms, default 120000). `project close-status --request <operation-id>` has a five-second read budget; `project close-resume --request <operation-id> --generation <1..64> --confirm` admits the same detach/timeout options. Ordinary `workspace close` closes only the selected Workspace; retired `--group` forms are rejected. Both preserve Git checkouts/branches. Project close-all retains the Project; dissolve only ungroups and retains Workspaces.

`orchbee.project.close-all` v1 separates retained Server truth from nullable wait disposition. Source is `local-workspace-group` with required-nullable Session ID. Results have only a Project scope; at most 256 captured members carry Workspace ID, label and nullable child Close. Empty groups are valid. Start/Resume submits once, followed only by reads of the same attempt/members; unknown effects are never replayed. Exit codes and partial truth follow [Project close-all](windows-worktrees.md#project-close-all). Help, `--skill` and completion declare these commands offline.

## Windows Client navigation

[Windows Client view control](windows-client-control.md) owns `client list/get/control-status`, `workspace focus`, `tab focus`, `panel focus` and `panel zoom`. Navigation targets one current TUI via `--client` or unambiguous validated caller context. It submits once and waits for a drawn acknowledgement; timeout or Ctrl+C stops waiting only. Single-panel zoom has an explicit no-op receipt. The current catalog includes `orchbee.client.view`, `orchbee.client.control` and `orchbee.cli.failure.client-control`; view/control are v1, routing failure is v1 and composed Agent Client read failure is v1. No operating-system window activation or Mac/Linux control runtime is added.

## Windows offline schema export

Text `api schema` without an ID lists the catalog: one row per schema ID with its versions and title, then how to read one schema or every document. `api schema <schema-id>`, `api schema --format json` and file export answer the JSON document. `api schema [<schema-id>] --output <path>` explicitly publishes the same complete JSON document that `api schema [<schema-id>] --format json` writes to stdout. It accepts `--format text|json` for failure presentation; successful stdout and file bytes remain identical JSON with a final newline. Omitting `--output` retains the existing read-only behavior.

The parser admits a nonempty literal file path of at most 4,096 UTF-8 bytes, validates all arguments before effects, and never expands variables or a leading tilde. Relative paths resolve against the invoking process directory. Windows device names, alternate data streams and ambiguous drive-relative paths are refused. This does not select or start a Session, inspect runtime configuration, or use the network to obtain schemas. New Mac/Linux file-export runtimes are not included.

The schema owner resolves the embedded ID and prepares the entire bounded document before any filesystem operation. Unknown IDs preserve existing not-found behavior and create nothing. The Windows exporter resolves the destination, refuses an existing non-regular/reparse target, creates one exclusively owned random sibling `.orchbee-schema-<nonce>.tmp`, writes and synchronizes the complete document, closes the handle, then publishes with one rename. Existing regular destinations may be replaced; parent directories are never created. There is no truncation fallback or automatic retry.

| Phase                                       | Result and ownership                                                                                               |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Grammar or embedded lookup fails            | No filesystem effects; existing typed failure                                                                      |
| Destination/temporary preparation fails     | Target unchanged; local export failure                                                                             |
| Write, synchronization or publication fails | No successful publication; remove the owned temporary and report the exact failed phase                            |
| Temporary cleanup fails                     | Retain and report its path so the caller can remove that owned residue                                             |
| Rename succeeds                             | Complete destination published; emit the already prepared stdout; later stdout failure does not roll the file back |

The dedicated `orchbee.cli.failure.schema-export` version 1 document carries `write-failed`, a closed phase, the literal destination and nullable retained temporary path; it exits 1. It appears on stdout only with `--format json`, alongside a localized stderr diagnostic. No provider, terminal, canonical Workbench state or runtime identity is changed. A process crash may leave a recognizable unpublished sibling temporary; publication does not claim power-loss durability for the parent directory.

Maintained Windows process checks cover Unicode/space and relative paths, full and selected schemas, replacement, grammar/unknown-ID isolation, absent parents, directories, a locked destination preserving its previous bytes, schema-valid failure output and absence of normal cleanup residue. Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` `src/cli/api.rs` supplies the selected file-export capability; Orchbee keeps its own schema envelope and complete-file publication.

## Windows Panel layout queries

`panel layout`, `panel neighbor --direction left|right|up|down` and `panel edges` inspect an exact Windows TUI Client. They accept a positional Panel or `--current`, and `--client` when routing is ambiguous. Reads preserve focus and terminal sizes, including inactive or unvisited targets. `orchbee.panel.layout` v1 retains cell geometry, nullable hidden rectangles, unzoomed neighbors, physical-edge results and the operation receipt. See the [geometry contract](windows-client-layout.md). The offline catalog lists the current documents and command paths.

`panel focus [source] --direction left|right|up|down` finds one neighbor in the selected TUI's unzoomed cell layout and preserves zoom. No neighbor returns `direction-unchanged` without activating the source Tab. A found destination is pinned and returns `direction-focused` only after a successful draw. Receipt reads never repeat the movement.

`panel resize [panel] --direction left|right|up|down [--amount <number>]` adjusts one eligible split border, preserving focus and zoom. `layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` sets an exact binary split path. Both route to one Windows TUI and retain a `layout-adjusted` receipt. Selected changes wait for a draw; inactive changes store without navigation. Finite ratios clamp to 0.1–0.9, and finite resize amounts normalize to an absolute magnitude at most 0.5 (default 0.05). Ratios belong to that TUI session and are not restored after restart. See the [layout contract](windows-client-layout.md).

## Shared Panel titles

`panel rename <panel-id> --label <text>` sets the shared primary Presentation override; `--clear` (or blank text) restores its creation label. It accepts an exact ID or `@panel:0x<hex>`, with no focus or terminal effect. The [Panel label contract](native-panel-labels.md) owns normalization, revision/replay rules and coordinated cohort compatibility. `orchbee.panel.rename` v1 distinguishes `renamed` from `unchanged`; its nullable `customLabel` echoes normalized request metadata. `panel get` and `panel current` v1 expose immutable `label`, nullable `customLabel` and effective `displayLabel`. `api snapshot` v1 preserves the canonical optional override. The offline catalog lists the current documents and command paths.

Panel swap uses the shared structural placement authority for one atomic same-Tab whole-leaf exchange, preserving Split/slot and terminal identities. The Windows CLI separately acknowledges source focus through the original Client; known canonical commits survive later failure or unknown focus. [Native Panel swap](native-panel-swap.md) owns the workflow, receipt and evidence.

Windows `tab create` and `panel split` resolve `--cwd follow|home|current|workspace|<absolute-path>` before creation; follow is the default and samples the resolved source root process. Their v1 receipt records the chosen directory source/path. They accept optional targets and labels, with no focus by default; `--focus` requests post-creation focus, and split accepts a Client-local `--ratio`. Explicit targets can create without a TUI. `orchbee.creation.result` v1 separates committed creation from later ratio/focus results; a later failure is not permission to repeat creation. [Native terminal creation](native-terminal-creation.md) owns target selection, total budget, concurrency guards and recovery for these Windows commands.

Windows `panel input (<panel>|--current) --right-click orchbee|panel` sets the shared right-button policy without moving focus. `--current` requires validated caller metadata; explicit targets bypass it. `panel split --right-click` commits the new Panel policy with creation. The default `orchbee` reserves right-click to the Client; `panel` permits unmodified right clicks in mouse-reporting terminal content. An admitted gesture retains its matching release after policy changes. See the [Panel input contract](native-panel-input.md) for routing, pending-input rechecks and failure recovery. This does not add a context menu. `orchbee.panel.input` v1 reports `configured` or `unchanged`; `panel list` v1 and `panel get/current` v1 expose `rightClick`. `tab get` v1 and `api snapshot` v1 admit the optional canonical field. The catalog lists the current documents and command paths.

## Tab insertion boundary

On Windows, `tab move <id|@tab:0x<hex>|label> (--order previous|next|first|last | --insert-index N)`
requires exactly one placement flag. `N` is a zero-based boundary in the original context order,
not a final ordinal: 0 means first, the Tab count means last, and the source boundary or immediately
following boundary leaves order unchanged. Parse accepts integers 0..256; the Server rejects values
above the current count without clamping. Exact target resolution uses one snapshot and one command
under the existing total mutation budget, with no Client selection or implicit focus.

Indexed movement emits `orchbee.tab.move` v1 with `action: tab.move`, requested `insertIndex`, exact
context/Tab, request/Session/base revision and committed or unchanged revision. Direction movement
keeps `orchbee.mutation.result` v1. The offline catalog lists the current documents and command paths. A definite
bounds rejection retains the Server reason. An ambiguous submission retires the carrier, reports
outcome unknown and never replays or compensates. Old direction behavior remains available on its
existing platforms; this new CLI form is Windows-only. There is no new disk persistence or migration.

Index-command runtime failures use `orchbee.cli.failure.tab-move` v1, adding definite
`tab-move-order-conflict` when the exact requested placement would break a pinned prefix or split
a contiguous group. Direction movement uses that v1 envelope only for this new reason; all other
direction and parser failures retain their existing generic envelope. The embedded schema preserves
closed v1 and rejects the new reason under v1. A refusal leaves order, metadata and authority intact. The index carrier admits a context-close `terminal-busy` as definite
precommit, without widening the direction carrier contract.

PowerShell completion reads plain ASCII decimal integer AST constants as literal source text, so
`--insert-index 2` keeps completion active. Variables, splats, expressions and scriptblocks are
not evaluated or admitted by this repair.

## Source-selected terminal output

Linux/Windows reads use `--source` and `--lines`, defaulting to recent80. Plain JSON emits
`orchbee.terminal.output` v1; ANSI emits v1. `--plain` exports UTF-8 directly and excludes JSON.
`--ansi` preserves styled blanks and Unicode using SGR, with no PTY-byte or graphics replay.
The [output contract](native-terminal-output.md) owns source
ranges, joining, output limits, cursor fencing and sample validation.

Source waits default to 30000 ms and recent80, with `--forever` for explicit unbounded waiting.
They emit v1, or v1 with `--after`; existing output can match without a cursor. A match alone
is not command completion. Reads and waits never take terminal control or change Client focus.

Windows `status server` reads actual Server build/protocol/cohort metadata before Client Connection admission; `status client` checks this executable offline and its v1 JSON includes bounded I/O diagnostics when cohort admission is unavailable. Both accept `--format text|json` and never activate a Session. Bare `status` remains a Workbench query. An absent explicitly selected Session names `orchbee --session <name> server start`; without an explicit selection the hint is `orchbee server start`. The [runtime status contract](native-runtime-status.md) owns the five-second budget, unknown metadata, compatibility, schemas and failures. Update-channel management remains unavailable.

## Agent navigation

Windows `agent focus <target> [--client <id>]` resolves a canonical Session or terminal-backed Agent to one navigable primary Panel. One `agent-focus` control retains the exact Agent and Presentation identity through Client navigation and completion. The Client rechecks its observation; a physical target also requires its matching Host frame. Successful explicit focus marks current known versions in the selected Tab seen, including an already-focused target and outer focus loss. Ordinary paint and read-only `agent attention` keep their existing rules. `--request-id`, `--detach`, the 30000 ms default total deadline and retained status recovery remain unchanged. An unready Client refuses before Agent reads; loss/replacement yields rejection or incomplete navigation, without retargeting. `orchbee.client.control` v1 carries this operation; no OS window activation occurs. The [Agent navigation contract](native-agent-navigation.md) owns identity, acknowledgement and final acceptance.

## Windows Agent change pages

`agent changes [--cursor <c>] [--limit <n>] [--format text|json]` reads one bounded page under the existing five-second total Agent read deadline. Omit cursor for a current-head checkpoint; use the returned opaque cursor for later reads. Limit is 1–100, default 50. No filtering, subscription, long poll or automatic replay occurs. It admits one Client Connection, closes it before interruption arbitration and emits one result. Ctrl-C preserves the shared cancellation/finalization contract.

The `orchbee.agent.changes` version-1 envelope contains the canonical changes/current/reset-required outcome and the correlated Agent source. Reset-required is a visible loss-of-continuity result; rebuild the baseline. Invalid/foreign-boot cursor, read denial and runtime unavailability use the existing Agent failure family with operation changes. A page cannot exceed the requested limit. JSON retains typed facts and cursors; localized text shows status, revision and continuation. The Server owns retention, authentication and coherent history; CLI grammar/help/completion/schema/Skill expose this boundary. Other platform runtimes remain unavailable for this new command.

For canonical session targets, Windows `agent prompt --wait` checkpoints the retained Agent change journal after runtime preflight and before submitting input once. A failed checkpoint prevents submission. The checkpoint must cover the preflight revision. Each later fully validated page preserves transient activity; a fresh newer provider version must show working/blocked or a turn-finished transition to open the activity gate. Only working/blocked conditions can match that gate event; settled conditions must match a current sample, so a transient completion followed by working keeps waiting. Metadata and freshness changes alone do not establish activity. The five-second activity gate, original total/read deadlines, pinned identity and cancellation owner remain. A current canonical sample must cover the journal head. Windows v1 separates nullable `matchedChange` and its source from the current observation; Linux remains v1. History loss or a foreign cursor fails explicitly and preserves the accepted input receipt. This proves observed activity after the checkpoint, not causality or exclusive ownership of the submitted turn.

For canonical session targets, Windows `agent prompt <target> <text> --wait --client <id>` preflights one exact Client before sending and reads its attention on each later sample. The canonical activity gate still applies. Idle/done match only its current corroborated resting projection; missing/stale versions keep waiting. A lost Client or read failure stops the wait with the accepted input receipt retained. The Client read bundle is bounded to two seconds inside the existing five-second evaluation and original total deadline. No read acknowledges, focuses or resubmits. Selected-Client success is v1 with required `client`; ordinary Windows prompt remains v1 and Linux v1. `--client` requires `--wait` and has no Mac/Linux implementation.

## Windows unified Agent projection

Windows `agent list` joins canonical Server sessions and physical terminal Agents into `orchbee.agent.list` v1. Its `source.kind` is `agent-projection`; the source carries `siteId` (32 lowercase hex), decimal-string `workbenchRevision` and `reportRevision`, and safe-integer `canonicalRevision`. Its `result` is `{kind:"listed",revision,nextCursor,items,clientId}`; `nextCursor` is null or a `u1` cursor. Each row has a one-based `ordinal` (1–4096), `provider`, semantic `status` (`unknown`, `idle`, `working`, `blocked`, `done`, `error`, `ended`), identity `source` (`canonical`, `terminal`, `both`), the existing nullable canonical `AgentSessionSummary`, and a nullable physical terminal record with its existing launch binding, nonempty provider, `PanelAgentState` and decimal-string sequence. `source` controls which facets are present. Physical and `both` rows always carry `placement` with 32-hex Panel, Presentation, Terminal, Terminal Context and Tab IDs; canonical-only rows may also carry placement when a current terminal incarnation is observed with no Agent. `workspaceId` within placement is nullable. `cwd` is a nullable string. When `--include-ended` is used, ended sessions sort after all current live rows, keeping live ordinals stable; ended rows follow canonical ID order and can be inspected by exact `--agent-session`.

`clientId` and every row's `client` are always present, null on Windows when no Client was selected, with canonical and terminal Client facets kept separate. Client unread state never supplies semantic Agent status. Optional cwd enrichment covers at most 64 rows per page and returns null when unavailable. Pages contain 1–100 rows (default 50); the joined output ceiling is 4096 rows and 8 MiB. Source admission can fail earlier: the canonical source is bounded to 512 sessions/six pages of 100, and physical source registration is bounded to 1024 entries. The 4096-row output ceiling does not promise that many physical sources.

Windows cursors use `u1.<base64url>` and are at most 1024 characters. The payload is canonical JSON encoded as base64url. Cursors bind to Site, source revisions, `includeEnded`/Client filters, page offset and fingerprint; they require no retained files. Malformed cursors are rejected as invalid input before live source reads. The CLI must read current sources to determine whether a `u1` cursor is stale, so it can reject a stale `u1` cursor only after those reads; the caller must restart at the first page.

Windows `agent get` emits `orchbee.agent.get` v1 with and without `--client`, using the same projection `source`. `result` is `{kind:"projected",agent,inspection}`. `agent` has the projection row's provider, status, source, canonical/terminal facets, placement and cwd, without `ordinal` or `client`; `inspection` is the existing current canonical or terminal inspection. Detailed process, terminal metadata, management and Client evidence stay in their facet-specific inspection shape.

Windows Get, Read, Prompt, Send Keys, Wait and Focus share one resolver. `@agent:<kind>` selects a provider only when the complete current projection proves it unique. `@agent:<n>` selects a one-based ordinal in the complete list order, never a page-relative position; the resolver accepts live rows only, so use exact `--agent-session` for ended sessions. Bare names preserve Server alias precedence before provider fallback. Partial reads cannot establish provider uniqueness. Mutating effects retain the complete physical binding and revalidate it immediately before the write.

Projection failures use `orchbee.cli.failure.agent-projection` v1. The closed `failure.details` has `stage:"agent-projection"`, a reason from `unavailable`, `denied`, `changed`, `invalid`, `too-large`, `timeout`, `cancelled`, `cursor-restart`, `target-not-found` or `target-ambiguous`, and optional `selector`/`candidates`. Candidate entries carry `selector:"@agent:<n>"`, nullable `sessionId`, nullable `panelId` and nullable `AgentLaunchBinding`; at most 16 candidates are returned. The optional envelope `source` has the same typed projection shape used by Agent list/get, and failure envelopes have no `result`.

The native provider registry now has 20 entries, including Windows Cursor, Qoder and Antigravity JSON hook onboarding and offline diagnosis. The Agent projection documents are registered in API catalog version 1.

Mastracode adds Windows flat hook onboarding, partial-install repair, offline diagnosis and keyed lifecycle observations.

Droid adds Windows JSON hook onboarding and diagnosis at the currently selected user configuration source.

Kimi adds Windows TOML session hook onboarding and diagnosis; it does not yet claim complete working/blocked/idle observation.

Letta adds Windows session onboarding and offline diagnosis. Its hook output stays silent because the provider consumes stdout as context.

Devin adds Windows onboarding and offline diagnosis for six identity hooks. All events preserve turn state; missing or invalid APPDATA refuses setup.

Hermes adds Windows plugin activation and content-free session observation. Its offline doctor reports that provider command inspection is required.

## Windows persistent creation defaults

[Windows CLI configuration](windows-cli-configuration.md) owns the bounded TOML reader, `ORCHBEE_CONFIG_FILE`, `[terminal] new_cwd`, `[worktrees] directory`, and precedence before any creation effect. Windows `config check` v1 preserves five runtime checks and adds `user-config`. `orchbee.cli.failure.configuration` v1 reports bounded preparation errors. The offline catalog lists the current documents and command paths.

The current Windows `config check` v1 keeps fatal file and environment admission as `fail`/exit 1 and reports admitted field fallbacks as `warning` with `ok:true`/exit 0. Its bounded `diagnostics` and exact `diagnosticsOmitted` count follow the current closed contract. `server config`/`server reload-config` v1 carry the same admitted warning record in each successful snapshot. The embedded schema catalog is v1.

## Windows terminal observation stream

`terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]` emits NDJSON from one existing binding without control or Host resizing. The [stream contract](windows-terminal-streams.md) owns full safe ANSI frames, resets, flush-before-ACK, deadlines and cleanup. Default viewport: 120×40; default duration: unlimited. Both output formats emit `orchbee.terminal.stream` v1 records. The catalog lists the current documents and command paths. Machine control now accepts NDJSON input through `terminal session control`; interactive terminal/Agent attach is implemented with remaining fidelity and acceptance obligations.

### Panel relocation transfer receipts

All shared mutation receipts use version 1, including Panel relocation.
Relocation includes optional `transfer` facts:
`targetTerminalContextId`, `closedSourceTab`, optional `closedSourceWorkspaceId` and
`createdWorkspaceId`. Existing-Tab moves also preserve the returned Split and slot identities.
Moving a Tab's final leaf removes that Tab without disposing its terminal; the result reports
that removal. The local carrier checks destination kind/context, created-Workspace presence,
source-removal consistency and result correlation before acknowledging a commit. The catalog lists the current documents and command paths.

### Windows Panel relocation workflow

```text
panel move <panel> --into-tab <tab> --direction left|right|up|down [--target-panel <panel>] [--ratio <number>]
panel move <panel> --new-tab [--workspace <workspace>] [--label <text>]
panel move <panel> --new-workspace [--project <project>] [--label <text>] [--tab-label <text>]
```

All forms accept `--focus|--no-focus`, `--client` and `--timeout`. Focus defaults to true.
`--into-tab` also supports placement within the current Tab; `--target-panel` selects its neighbor. Ratios must be finite
and are clamped to 0.1–0.9. New Tab labels are optional.

Resolve one snapshot and an explicit, caller-associated or sole current Client. Ambiguity fails
before effects. Without a Client, only `--no-focus` without an explicit ratio may proceed: choose
the first target leaf and native default ratio, without claiming Client geometry or zoom state.
With a Client, read-only source/target layouts determine zoom and the default target leaf.
Zoomed source/target Tabs return unchanged without mutation. Same-Tab placement submits the existing Panel move effect. An explicit
target Panel must belong to the target Tab.

A new Workspace joins an existing Project only through explicit `--project`; omission is ungrouped and never inherits the source Project or creates a default Project. Its directory follows observed terminal root cwd, falling back to invoking process cwd only when absent. Invalid observations fail. Validate path and label before effects.

Phases are preflight → canonical transfer with new-Workspace Directory → optional ratio → focus → observed layouts, sharing one absolute deadline and cancellation. Server commits topology atomically without restarting the terminal; Directory joins that commit after Worker validation. `orchbee.panel.move` v1 removes the Project preparation receipt/stage and retains canonical and Client results separately. Never replay after failure, cancellation or uncertainty, or report partial success as complete.

Ratio targets the returned Split/slot identities after snapshot refresh; same-Tab placement derives the moved Panel’s parent Split only from the exact movement receipt revision, refusing to follow concurrent rearrangement. Client follow-up waits
for canonical convergence and preserves an unaffected prior selection; unrelated navigation
refuses follow-up. Removal of the selected source permits ordinary selection repair. Other
Clients keep independent navigation. Receipt layouts describe only the selected Client.
An unchanged empty Ready view may enter the existing minimum-revision-fenced control without
waiting for an unrelated navigation heartbeat; its observed revision must not regress. Inactive
target layouts report that Client's remembered canonical Panel, or the first member when absent.
Grammar/decision tests, receipt validation, cancellable transport checks and real Windows multiple-Client
live-terminal/restart acceptance cover these boundaries. Generic events and existing child,
grouped and pinned structure restrictions remain separate obligations.

The offline catalog lists the current documents and command paths.

## Reusable layout export

Windows `layout export` emits a reusable JSON tree with ratios, manual labels, observed directories and frozen launch commands. The [layout contract](windows-layouts.md) owns export targets, Client selection, bounds, cancellation, absence of terminal effects and consistency, together with the implemented `layout apply` creation/replacement workflow and its effect receipts. `orchbee.layout.export` v1 and `orchbee.cli.failure.layout` v1 are listed in the current catalog.

## Programmatic semantic keys

`terminal send --key` and `agent send-keys` share one parser. Named keys and modifiers use canonical lowercase spellings: `enter`, `escape`, `tab`, `backspace`, `delete`, `up`, `down`, `left`, `right`, `home`, `end`, `page-up`, `page-down`, `insert`, `space`, `f1`..`f25`, and `ctrl+`, `alt+`, `shift+`, `super+` before the base key. One non-control Unicode scalar and punctuation names (`plus`, `minus`, `comma`, `period`, `slash`, `backslash`, `quote`, `double-quote`, `semicolon`, `colon`, `percent`, `ampersand`, `backtick`) are accepted; ASCII uppercase characters imply Shift. Use `plus` for the plus key and quote shell metacharacters. Duplicate modifiers, multiple base keys, empty components, alternate names, leading-zero function numbers and modifiers after the base are usage errors before admission. Control-letter presses use `ctrl+c` and `ctrl+d`.

Every semantic key uses one validated stroke record through both native protocols. Worker supplies physical identity, logical text, unshifted scalar and modifiers; shifted printable text consumes Shift for text generation while retaining it for negotiated full key reporting. Host encodes the current terminal mode, including CSI-u for Super character chords in the legacy terminal keyboard mode. Sequence order, at most 64 keys, a shared deadline, input-only attachment, prefix receipts and no replay after partial or unknown outcomes remain unchanged. Function keys may be mode-ignored where the child has not enabled a supporting keyboard protocol; receipts retain the actual encoded-byte count.

The same-author regressions cover parser refusal, independent wire goldens, malformed scalar/flag boundaries, mixed record boundaries, Worker semantic projection and Host legacy/Kitty encoding. Focused verification does not replace final Windows cohort acceptance or full-goal regression.

## Windows Panel metadata query

The [Panel metadata contract](windows-panel-metadata.md#query-observation-model) owns batched primary reads and both final revision fences. [Current Panel](native-current-panel.md#current-member-enrichment) instead checks the exact resolved member's ID, Panel and binding. Panel list/get v1 and current v1 expose their metadata directly; Windows Agent Get v1 retains metadata and source-attributed `reportedAgent` in its terminal `inspection`. Actual report Agent/authority context selects guarded presentation while preserving raw source records and global tokens. Canonical OSC title, names, status and readiness remain distinct.

Launch selections additionally cap worst-case JSON string-content bytes at 30 KiB: ASCII controls count as six, quotes and backslashes as two, and other UTF-8 bytes as one. This preserves the previous 2 KiB input domain while bounding the larger resume commands. Matching cohorts are required for the enlarged limits.

Windows Session restart resumes retained official provider references by default, once per distinct source/Agent/reference kind/value. Set ORCHBEE_RESUME_AGENTS_ON_RESTORE=false before activation to start ordinary shells and retain references; only exact true/false values are accepted. Existing Sessions are unaffected. The Server reads SystemRoot to resolve system Windows PowerShell and inherits PATH for provider program discovery. Shell launch is not provider readiness. See the Server [restart model](../../orchbee-server/docs/agent-restart-reference.md) for failure, deduplication and remaining restoration boundaries.

## Installed Agent detection manifests

Windows `server agent-manifests` reads the running Session's installed rules, revision, engine version,
configuration root and each provider's actual source/version/path/cache/warnings. It also reports the
frozen update catalog, source errors, last update facts and persisted-status errors.
`server reload-agent-manifests [--request-id <operation-id>]` reloads the startup configuration root
and existing status, resets screen stabilization and switches sampling and Explain together.
`server update-agent-manifests [--request-id <operation-id>]` downloads validated rules through the
Server's owned Windows curl worker, commits eligible cache versions and then activates the current
local/cache/bundled selection. Local overrides continue to take precedence. Catalog selection uses
`ORCHBEE_AGENT_DETECTION_MANIFEST_CATALOG_URL`, defaulting to
`https://herdr.dev/agent-detection/index.toml`; invalid overrides disable Update while local rules
remain usable. The source is frozen at Session startup. Managed release Sessions automatically check at startup and
every thirty minutes when `update.manifest_check` is true (default). Debug/test and standalone
execution do not schedule checks. Rule Reload also refreshes this switch from the frozen config
path; malformed configuration preserves the previous policy and detector. Automatic results use
a separate latest-result slot without consuming manual receipts.
`server agent-manifests-status <operation-id>` reads either retained receipt without resubmitting.
These commands require an already running Session and do not focus a Client or send terminal input.

Each invocation uses one five-second admission/exchange budget. Reload and Update submit once and
poll only status. Pending/unknown effects retain the operation ID and exit 8; read transport
uncertainty exits 4. Denials/busy/not-recorded/capacity/kind-conflict/update-disabled failures exit 3;
unavailable exits 4; cancelled operations exit 7; load/revision/internal errors exit 1. A complete
update exits 0 only when every provider result and status persistence succeeded and activation was
applied. Partial failure or failed/skipped activation exits 1 even when some cache files were written.
Late interruption preserves the last admitted receipt. IDs are nonzero 32-character lowercase
hexadecimal values; omission generates one. Same-kind accepted duplicates read the existing receipt;
cross-kind reuse fails. One Reload/Update is active at a time, with 1,024 retained receipts for the
Session generation and no eviction. Restart ends receipt scope; the last cache report remains on disk.

`orchbee.site.agent-manifests` v1 includes Update, maintenance
metadata, update reports and unknown-effect receipts. v1 adds automatic policy gates, settings
provenance and the latest automatic operation. Reports show attempted/cached versions and
per-provider failures separately from activation revision/failure. Text is localized and escaped.
The offline catalog lists the current documents and command paths. The Server
[installed-manifest maintenance contract](../../orchbee-server/docs/agent-screen-model.md#installed-manifest-maintenance)
owns cache safety, installation, cancellation and receipt limits.

Five Windows Panel report commands also accept `--presentation <full-id>` for an exact primary or child member of the positional Panel. Omission retains primary targeting with `presentationId:null`; an explicit member fills that same required field. [Agent reports](windows-panel-agent-reports.md#exact-member-targets) and [metadata reports](windows-panel-metadata.md#exact-member-targets) own identity checks, byte preflight and unknown-outcome handling. The catalog lists the current documents and command paths.

## Source-owned Agent view commands

Windows `agent view get`, `agent view set --spec <json>` and
`agent view clear [--source <name>]` use the selected Session, including `--machine`
routing. The [source-owned view contract](machines.md#source-owned-agent-views)
owns normalization, bounded filter/multikey-sort semantics, transient source
replacement, plugin eligibility, Client attention and TUI projection. The
`orchbee.agent.view` v1 receipt distinguishes Server failures, exchange failures and unknown mutations; The catalog lists the current documents and command paths.

## Agent sidebar configuration publication

Windows `config check` v1 and `server config`/`server reload-config` v1 expose the required bounded `agentSidebarProfile`; configuration failure v1 includes sidebar admission diagnostics. The [Windows configuration owner](windows-cli-configuration.md#windows-agent-sidebar-layouts) defines startup, fatal source boundaries, field-local admission, canonical publication and atomic Client application. The offline catalog lists the current documents and command paths.

## Windows CLI process verification

The Agent focus/Workbench process suite admits at most four live test cohorts per test binary.
Each slot is acquired before copying/hashing its isolated native executable and retained through
process output, peer joining and private-directory cleanup. Waiting for this fixture resource
occurs before the command timer; product and test deadlines remain unchanged. The cap protects
cold Windows process admission and executable I/O from contention with other cases in this suite,
without serializing application state or reducing the checked scenarios.

The viewport-timeout peer may observe `BrokenPipe` when the CLI expires after sending its last
status poll but before receiving the queued response. It then stops writing; the consumer must
still return the exact timeout failure with no partial result. Other write errors and malformed
requests remain failures, and no receipt is fabricated after the connection closes.

## Uniform retained operation paths

[Retained operation status](native-operation-status.md) defines the preferred `<family> status <request-id>` and supported `<family> resume <request-id> --generation <n> --confirm` grammar. Older family-specific paths described in this reference remain compatibility entry points; new callers use the uniform paths. Tab and Panel now have explicit status and resume access.

## Panel targets for terminal I/O

`terminal run/read/send/wait-output` accept a terminal ID or `@terminal:`, a Panel ID or `@panel:`, or `--panel <panel>`. Panel targeting selects the primary presentation in one admitted snapshot; it never selects a child or changes focus. Use a child terminal ID explicitly. Missing, unbound and ambiguous Panels fail before input. Bare IDs that name different Panel and Terminal resources refuse; use typed handles to disambiguate. The response retains the actual terminal and placement identities. Example: `terminal run --panel @panel:0x1 "git status"`. The ordinary target and `--panel` are exclusive.
