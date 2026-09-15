# Native CLI Query

Status: accepted implementation and correctness contract for the first native `orchbee`
command-line query surface. The commands below are read-only observations of an already running
local Site. They never create, repair, lock, or activate a Site; never create terminals, viewers,
or control grants — the single exception is `terminal read`, which holds one short-lived
read-only viewer attachment against an already bound terminal and releases it before output.
They never guess Agent, Worker, process, or PTY state; and never require a TTY.
This is an unpublished local development surface; `apps/cli` remains the delivered TypeScript CLI
authority, and installation, distribution, remote connections, and Site lifecycle operations stay
out of scope.

Authority: this record owns the public grammar, result envelopes, failure matrix, budgets,
single-request read-only state machine, and output side effects implemented under
`src/invocation.rs` and `src/cli/**`. It implements the accepted CLI-Q design and both challenge
dispositions; where prose and code disagree, the behavior is a bug to fix, not a fork to keep.

## Inputs and trust boundary

- `argv`: bounded hostile input. Total UTF-8 bytes across arguments may not exceed 64 KiB
  (65,536). Any non-UTF-8 argument, or any overflow, is an `invalid-usage` failure with empty
  stdout even when another argument spells `--format=json`. No stdin or file input is read
  implicitly.
- Environment: `ORCHBEE_RUNTIME_BASE` (optional; defaults to `/tmp`) selects the runtime base;
  `LC_ALL`/`LC_MESSAGES`/`LANG` select the diagnostic locale. Nothing else is consulted.
- Filesystem: the resolved runtime base, the Site scope directory
  `orchbee-native-<euid>`, its `client.sock`/`worker.sock`, the activation marker, and the
  current executable's sibling `cohort.json` plus four artifacts are observed but never written.
  All directory traversal uses the existing no-follow, owner/mode-checked rendezvous owner
  (`ModeCheckedLocalSiteRendezvous::validate_mode_checked`, `ExistingOnly` policy only).
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
- `--project <target>`, `--workspace <target>`, `--tab <target>`: named value filters, single
  use each. The next token is consumed as the value even when it begins with a dash; a bare `--`
  is never consumed as a value (missing value → `invalid-usage`). A consumed filter value is not
  a format selector. Each filter is valid only on its declared command (`--project` on
  `workspace list`, `--workspace` on `tab list`, `--tab` on `panel list`); anywhere else,
  including `get` commands, is `invalid-usage`. Filters are never silently dropped: a filter
  flag on a command that does not declare it is `invalid-usage` before any runtime access, so
  `workspace list` rejects `--workspace`, `--tab`, and `--standalone`.
- `--standalone`: valueless boolean flag, single use, only on `tab list`; mutually exclusive
  with `--workspace`.
- `--version`: legal only as the sole root invocation; any other position or company is usage.
- `--help`: root sole token, or the single suffix of a complete command path.
- `--` positionals: tokens after `--` are positional only, so `orchbee -- status` queries
  status and `orchbee project get -- --help` resolves the literal label `--help`.

Command paths (fixed words, no abbreviations or aliases):

```
orchbee                         # bare → TUI (unchanged)
orchbee tui                     # TUI (unchanged)
orchbee --version               # version line (unchanged)
orchbee --help | orchbee help   # root help
orchbee status
orchbee project list | project get <target>
orchbee workspace list [--project <target>] | workspace get <target>
orchbee tab list [--workspace <target> | --standalone] | tab get <target>
orchbee panel list [--tab <target>] | panel get <target>
orchbee terminal list | terminal get <target> | terminal read <target>
orchbee api snapshot
orchbee api schema [<schema-id>]
orchbee config check
orchbee completion bash|zsh|fish
orchbee help <recognized path>
orchbee <complete command path> --help
```

Help topics: root; families `project`, `workspace`, `tab`, `panel`, `terminal`, `api`, `config`,
`completion`; and every complete command path including `tui`. `help` takes only a recognized
path (no filters, no format). A family name alone is not a complete command path for the suffix
form: `project --help` is usage while `help project` gives the family overview. Suffix help takes
precedence over missing business arguments (`project get --help` describes `project get`) but not
over supplied business arguments or extra tokens (`project get x --help`, `status extra --help`
are usage). Unlisted targets such as `help help`, `help version`, `help --version` are unknown
paths → usage. Help, `--version`, `completion`, and `api schema` never touch platform, runtime,
cohort, or Site state.

`help`, `completion`, `tui`, `--version`, and the bare invocation are fixed text outputs that do
not accept `--format`; a single valid `--format=json` turns the resulting syntax failure into a
JSON `invalid-usage` failure document, while a text/default failure keeps stdout empty. Data
commands (`status`, all `list`/`get`, `api snapshot`, `api schema`, `config check`) accept
`--format`. The `api` commands are the machine surface: a successful `api snapshot` or
`api schema` emits the identical JSON document under both formats, while an `api` failure
follows the selected format like any other command — under text/default it reports on stderr
with empty stdout, and only `--format json` produces a failure envelope.

`orchbee completion` prints a static projection of this grammar: verbs, complete help paths,
each command's declared flags, both `--format` spellings and its `text`/`json` values, `--`
termination, and flag-value consumption contexts. It enumerates no Site state, resource names,
or runtime data.

### Targets and filters

`<target>` for `project`/`workspace`/`tab` is first parsed as a complete typed ID (exactly 32
lowercase hexadecimal characters, nonzero). A valid ID matches by identity only. Any other token
is an exact, case-sensitive label candidate requiring 1–512 Unicode scalars; empty or overlong
tokens and the all-zero ID shape are `invalid-identifier` (exit 2). A label that itself has valid
ID shape can never be reached by label matching — IDs take precedence by construction; the entity
remains reachable through its real ID. `panel` and `terminal` accept only complete typed IDs;
anything else is `invalid-identifier`. Filters (`--project`, `--workspace`, `--tab`) use the same
per-resource target rules. `api schema` takes a nonempty schema ID; an empty one is
`invalid-identifier` with `resource="schema"`, an unmatched one is `not-found`.

Resolution after a successful read: zero matches → `not-found` (3); multiple exact label matches
→ `ambiguous` (3) reporting at most the first 8 candidate typed IDs in canonical traversal order
plus the true `totalCount`. A filter naming an absent entity is `not-found`; an existing entity
with an empty result set is a successful empty list.

## Query state machine and budgets

Every network query (`status`, `list`/`get`, `api snapshot`) performs exactly this ordered
sequence on Linux; the first failing stage wins and determines the failure:

1. **Cohort** — `NativeCohort::current` under one checked 5-second monotonic deadline
   (`Instant::now().checked_add(5s)`; a failed `checked_add` is `internal-failure`, exit 1).
   Any cohort error (missing, invalid, unsafe, hash mismatch, cooperative deadline) is
   `check-failed` stage `cohort`.
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
5-second cohort budget for its cohort check. Non-Linux platforms produce `service-unavailable`
`{stage:"platform",reason:"unsupported-platform"}` for network queries and `check-failed` with the
same details for `config check`; there is no fallback to unbounded connection APIs.

`terminal read` shares steps 1–5 unchanged, then keeps the admitted session alive inside the
same deadline — never a second admission — for one read-only viewer observation:

1. **Resolve** — the typed terminal ID is looked up in the validated snapshot's presentation
   bindings; absent → `not-found` (3) with `{resource:"terminal",target}` and the session drops.
   Labels are never consulted.
2. **Attach** — one `terminal.viewer` `AttachBinding{siteId,workerId,terminalId}` from the
   snapshot's own facts. `Attached{attachmentId,hostEpoch}` seeds a `TerminalProjection` scoped
   to that attachment and epoch; any other outcome class maps through the failure matrix.
3. **Poll** — `Poll{attachmentId}` repeats inside the same deadline: an `Empty` outcome waits
   10 ms and retries; a `record` outcome feeds the projection. A record that fails transport or
   frame validation is `protocol-invalid` stage `viewer`; a record the projection answers with
   `NeedSnapshot` sends that feedback once and polling continues (a `FeedbackAccepted` answer);
   a consumed `ProjectionBlocked` is ignored. The first record the projection accepts ends the
   observation — the applied frame is the answer; `FrameApplied` feedback is never sent for a
   one-shot read.
4. **Release** — a best-effort `Detach{attachmentId}` precedes the unconditional session drop;
   both complete before any mapping, formatting, or output. The viewer attachment is never
   retained into output, and the session drop is itself the teardown guarantee.

## Canonical data and results

Canonical traversal order: `navigation.projects` order; each project's `workspaces` order;
`terminalContexts` order then `tabs` order; panels depth-first with `children[0]` before
`children[1]`; terminal bindings in that panel order with `primaryPresentation` before
`childPresentations`. No sorting is added; filtered lists and `ambiguous` candidates preserve it.

Success envelopes are `{schema, version:1, source, result}`:

- `source.kind="workbench-snapshot"` with `siteId` and decimal-string `revision` from the same
  validated snapshot — present only after a complete valid read.
- `source.kind="embedded"` — the compiled-in schema dictionary (`api schema` results and its
  `not-found` failure).
- `source.kind="local-runtime"` — produced `config check` results.
- There is no freshness field; one consistent read is not a liveness guarantee.

Result payloads (schema IDs are stable machine names):

- `orchbee.status` → `{site:{reachable:true,siteId,revision},
counts:{projects,workspaces,tabs,panels,boundTerminals},
worker:{status:"unknown",reason:"not-in-workbench-snapshot"}}`. `boundTerminals` counts
  presentations with a `binding`; snapshot validation already guarantees terminal IDs are unique,
  so each binding names a distinct terminal. `reachable` proves only this read succeeded.
- `orchbee.project.list` → `{projects:[{projectId,label,directoryCount,workspaceCount}]}`;
  `orchbee.project.get` → `{project:{projectId,label,directories:[{directoryId,workerId,path}],
workspaces:[{workspaceId,label}]}}`.
- `orchbee.workspace.list` → `{workspaces:[{workspaceId,label,projectId,
primaryDirectoryId|null,terminalContextId}]}` flattened across projects; `workspace get` adds
  `tabs:[{tabId,label}]` resolved through `terminalContextId`.
- `orchbee.tab.list` → `{tabs:[{tabId,label,pinned,groupId|null,
context:{kind:"standalone",terminalContextId}|{kind:"workspace",terminalContextId,
workspaceId}}]}`; `workspaceId` is absent on standalone, never null. `tab get` adds `root`, the
  typed `PanelNode` subtree re-encoded without losing semantic fields.
- `orchbee.panel.list` → `{panels:[{panelId,tabId,terminalContextId,primaryPresentationId,
presentationCount,primaryTerminalId|null}]}` where `primaryTerminalId` reflects only
  `primaryPresentation.binding`; `panel get` → `{panel:{panelId,tabId,context,slotPath,
primaryPresentation,childPresentations}}` with `slotPath` the `PanelSlot` IDs from root to leaf
  (`[]` when the root itself is the panel) and each presentation `{presentationId,label,
binding:{terminalId,workerId}|null}`.
- `orchbee.terminal.list` → `{terminals:[{terminalId,workerId,boundTo:{presentationId,panelId,
tabId,terminalContextId}}]}`; `terminal get` returns the same identity facts under
  `terminal`. Only presentation-bound terminals are visible; a binding does not prove the process
  lives. No lifecycle/process/agent fields exist anywhere.
- `orchbee.terminal.read` → `{terminal,frame,content}`: `terminal` carries the same
  `{terminalId,workerId,boundTo}` identity facts; `frame` carries the applied projection's
  `{revision,generation,sequence}` as decimal strings (u64-safe), `alternateScreen`, `geometry`
  `{columns,rows}`, and `cursor` `{row,column,visible}`; `content` carries `rows` (the emitted
  grid rows as `{index,text,continuation:{fromPrevious,intoNext}}` — `text` joins the row's
  cells, dropping wide-cell spacer tails and trimming grid padding), `rowCount` (every row the
  applied frame held), `truncated`/`omittedRows`, and `limits` `{rows:4096,textBytes:4194304}`.
  Emitted rows are bounded by a 4 MiB summed JSON-encoded-text budget; rows past the budget are
  counted in `omittedRows` and `truncated` is true. Under text output each emitted row prints as
  one escaped line followed by a localized truncation marker when `omittedRows>0`. The read is a
  single point-in-time frame: `revision`/`generation`/`sequence` are the host's counters at that
  observation, not a CLI freshness claim.
- `orchbee.api.snapshot` → `{snapshot}`: the complete `WorkbenchProtocolDocument::Snapshot`
  semantic re-encode including `kind:"workbench.snapshot"`; envelope `siteId`/`revision` agree
  with the inner document.
- `orchbee.api.schema` → `{schema:{schemaId,document}}` for a named schema, or
  `{schemas:{schemaId:document,…}}` listing the whole delivered dictionary (draft 2020-12).
- `orchbee.config.check` → `{ok,checks:[{name,status,details}]}` in fixed order
  `runtime-base`, `runtime-directory`, `cohort`, `locale`, `sockets`; all five always report.
  A failed check still produces this success schema with `ok:false` and exit 1.
- `orchbee.cli.failure` → `{schema:"orchbee.cli.failure",version:1,failure:{kind,details?},
source?}`; no English `message` field exists. `source` is attached only from an already trusted
  source (a validated snapshot for post-read failures, `embedded` for `api schema` not-found);
  denied/transport failures have none.

### Config checks

| check               | rule                                                                                                                                                                                                                                                                                                                                                                           |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `runtime-base`      | `ORCHBEE_RUNTIME_BASE`/default resolved and observed: pass when it exists as a directory; fail on invalid path, absence, unsafe entry, or I/O error.                                                                                                                                                                                                                           |
| `runtime-directory` | `not-applicable` when the base failed or the scope directory is absent under a valid base (a Site that is not running is not a configuration error); pass on a validated existing directory; fail on unsafe owner/mode/type or I/O error.                                                                                                                                      |
| `cohort`            | pass with `verified:true` when `NativeCohort::current` validates under its 5-second budget; fail on absence, invalidity, hash mismatch (`invalid-cohort`) or deadline (`deadline`).                                                                                                                                                                                            |
| `locale`            | informational pass carrying `locale` (`en-US`/`zh-CN`) and `source` (`LC_ALL`/`LC_MESSAGES`/`LANG`/`default`).                                                                                                                                                                                                                                                                 |
| `sockets`           | only after a safely inspected runtime directory; each of `client`/`worker` reports `presence` `present`/`absent`/`unchecked` with the existing type+owner+`0600`+no-follow rule. Any inspected invalidity fails the check; otherwise at least one compliant present socket passes; all absent/unchecked is `not-applicable`. Presence never claims service or Worker liveness. |

Typed details only: `runtime-base`/`runtime-directory` carry `path` (UTF-8) or `pathBytesHex`
(non-UTF-8, never replacement characters), `exists` only when the path state was established —
`false` for an established absence, `true` for an observed entry, and the field is omitted when
inspection itself failed (permission, path, or I/O error) — plus `reason` in
`{invalid-path,absent,unsafe,io-error}`; `sockets` reasons are `{unsafe,io-error,
runtime-unavailable}`. No cross-file atomic health claim is made.

## Failure matrix

Exit codes: 0 success (empty lists included); 1 local config/output/internal determinate failure;
2 usage/invalid target; 3 not-found/ambiguous/denied; 4 service-unavailable (including
`session-capacity`); 5 transport-loss/protocol-invalid; 6 incompatible; 130 default SIGINT
semantics. 7/8 stay reserved and unreachable in this read-only slice.

| Source                                                                        | `failure.kind`                                           | exit               | `details`                                          |
| ----------------------------------------------------------------------------- | -------------------------------------------------------- | ------------------ | -------------------------------------------------- |
| argv envelope/grammar/format/target shape errors                              | `invalid-usage`                                          | 2                  | omitted                                            |
| zero-ID/empty/overlong label, malformed panel/terminal ID                     | `invalid-identifier`                                     | 2                  | `{resource,target}`                                |
| valid target or schema ID without a match                                     | `not-found`                                              | 3                  | `{resource,target}`                                |
| multiple exact label matches                                                  | `ambiguous`                                              | 3                  | `{resource,target,candidates≤8,totalCount}`        |
| `NativeCohort::current` error                                                 | `check-failed`                                           | 1                  | `{stage:"cohort"}`                                 |
| base invalid/missing/not-a-directory, unsafe runtime/socket, other validation | `check-failed`                                           | 1                  | `{stage:"runtime-base"                             | "runtime-directory"   | "socket"}` |
| valid base with absent scope directory or absent `client.sock`                | `service-unavailable`                                    | 4                  | `{stage:"discovery"}`                              |
| `ConnectClientError::ConnectFailed`                                           | `service-unavailable`                                    | 4                  | `{stage:"admission"}`                              |
| `ConnectClientError::Io`                                                      | `transport-loss`                                         | 5                  | `{stage:"admission"}`                              |
| OBLC `Rejected(CapacityExceeded)`                                             | `session-capacity`                                       | 4                  | `{stage:"admission"}`                              |
| OBLC `Rejected(HandshakeTimeout)`                                             | `service-unavailable`                                    | 4                  | `{stage:"admission"}`                              |
| `Rejected(IncompatibleProtocol                                                | IncompatibleCohort)`, `Protocol(IncompatibleVersion)`    | `incompatible`     | 6                                                  | `{stage:"admission"}` |
| `Rejected(WrongMessage                                                        | MalformedFrame)`, `UnexpectedResponse`, other `Protocol` | `protocol-invalid` | 5                                                  | `{stage:"admission"}` |
| `RequestIoConfiguration`, `SessionUnavailable`, `RequestEncoding`             | `internal-failure`                                       | 1                  | omitted                                            |
| `SnapshotReadError::RequestNotSubmitted` (any cause)                          | `transport-loss`                                         | 5                  | `{stage:"snapshot"}`                               |
| `Failed(Write                                                                 | ResponseDeadline)`, `Failed(Response(Io))`               | `transport-loss`   | 5                                                  | `{stage:"snapshot"}`  |
| `Failed(Response(InvalidHeader                                                | Contract))`, `UnexpectedResponse`                        | `protocol-invalid` | 5                                                  | `{stage:"snapshot"}`  |
| `Failed(CorrelationMismatch)`                                                 | `protocol-invalid`                                       | 5                  | `{stage:"snapshot",reason:"correlation-mismatch"}` |
| outcome `Denied`                                                              | `denied`                                                 | 3                  | omitted                                            |
| outcome `Unavailable`                                                         | `service-unavailable`                                    | 4                  | `{stage:"snapshot"}`                               |
| viewer `RequestNotSubmitted`, `OutcomeUnknown(Write|Read)`                    | `transport-loss`                                         | 5                  | `{stage:"viewer"}`                                 |
| viewer `OutcomeUnknown(InvalidResult|UnexpectedResult)`, wrong outcome kind   | `protocol-invalid`                                       | 5                  | `{stage:"viewer"}`                                 |
| viewer `OutcomeUnknown(CorrelationMismatch)`                                  | `protocol-invalid`                                       | 5                  | `{stage:"viewer",reason:"correlation-mismatch"}`   |
| viewer outcome `Rejected(AlreadyAttached|TerminalUnavailable)` or `Unavailable` | `service-unavailable`                                  | 4                  | `{stage:"viewer"}`                                 |
| viewer outcome `Rejected(FeedbackMismatch)`, record rejected by the projection | `protocol-invalid`                                      | 5                  | `{stage:"viewer"}`                                 |
| prepared output over 8 MiB                                                    | `output-too-large`                                       | 1                  | `{limitBytes:8388608}`                             |
| stdout write/flush failure                                                    | `output-failed`                                          | 1                  | no second stdout document                          |
| `config check` finishes with failures                                         | success schema `ok:false`                                | 1                  | all five checks preserved                          |
| non-Linux network query / `config check`                                      | `service-unavailable`/`check-failed`                     | 4/1                | `{stage:"platform",reason:"unsupported-platform"}` |

`resource` is `project|workspace|tab|panel|terminal|schema`; `stage` is
`cohort|runtime-base|runtime-directory|socket|discovery|admission|snapshot|viewer|platform`.
Stages and reasons are finite schema values; no exception text becomes a machine field. `details` is a closed per-kind vocabulary —
`{resource,target}`, `{resource,target,candidates≤8,totalCount}`, `{stage[,reason]}`, or
`{limitBytes}` — and the embedded schemas reject documents outside those shapes. Read-path errors
never become outcome-unknown/failed-effect: a read-only request cannot produce an unknown side
effect.

## Output contract and side effects

- stdout carries only the contract document: one pretty (2-space) JSON document ending in `\n`
  under `--format json` (and always for `api` commands), or the localized human document under
  text/default. stderr carries localized human diagnostics; query paths print no progress.
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
- Offline commands (`help`, `--version`, `completion`, `api schema`) never perform platform,
  runtime, cohort, or Site discovery.
- A network query creates nothing: no directory, lock, marker, terminal, viewer, control grant,
  or second request — except `terminal read`, whose read-capability viewer attachment is the
  single sanctioned side effect, held minimally and released before output. At most one
  short-lived Client Session is held, and none survives into output.
- There is at most one snapshot read per invocation, one admission per invocation, and one
  network deadline covering both; `terminal read`'s viewer exchanges ride on that same session
  and deadline — never a second admission or a renewed budget.
- No result field exists without a trusted source; `null` means the source says "none" and
  `"unknown"` means the source cannot say — the two never mix.
- Failure envelopes never invent `source`: siteId/revision appear only from a validated snapshot.
- Ambiguous never guesses: it reports bounded candidates plus the true count.
- `api schema`, help, and completion enumerate exactly the commands delivered in the same build.
- Both 5-second budgets are CLI-owned checked deadlines; their construction failure is
  `internal-failure`, while transport-internal `RequestNotSubmitted` stays `transport-loss`.

## Scenario matrix and evidence

| Scenario                                                                                                      | Expected                                                        | Evidence                                                           |
| ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------ |
| Exhaustive grammar: formats, positions, `--`, help forms, duplicates, invalids                                | exact parse/failure kinds                                       | `src/invocation` tests                                             |
| Format envelope ordering: a defect anywhere leaves stdout empty; a valid json anywhere shapes later failures  | envelope precedence over grammar errors                         | `src/invocation` tests + `tests/cli_query.rs`                      |
| Filter scope: `workspace list` accepts only `--project`; wrong-command filters are usage before runtime       | complete matrix, no silently dropped filter                     | `src/invocation` tests + `tests/cli_query.rs`                      |
| Target rules: ID/label/zero/overlong/ambiguous precedence                                                     | resolver verdicts                                               | `src/cli` resolver tests                                           |
| Validated snapshot → every result shape, empty collections, nulls                                             | exact JSON/text fixtures                                        | `src/cli` output tests + JSON Schema validation (dev `jsonschema`) |
| Schema dictionary ↔ emitted envelopes                                                                         | every success/failure fixture validates; rejected variants fail | schema contrast tests                                              |
| API failure format: text/default failures keep stdout empty, `--format json` yields the envelope              | caller-selected failure format on `api schema`/`api snapshot`   | `tests/cli_query.rs` + `src/cli` tests                             |
| No Site on disk                                                                                               | `service-unavailable`, zero fs/process/lock mutation            | `tests/cli_query.rs`                                               |
| Real staged-cohort Server: status/list/get/api snapshot, 8 admitted + ninth rejected + release, live TUI      | success/`session-capacity`/unchanged structure                  | `tests/local_startup_process`                                      |
| Scripted OBLC/OBWP peers: denied, unavailable, malformed, wrong correlation, timeout, disconnect              | typed kinds per matrix; never labeled real-Server evidence      | `tests/cli_query.rs`                                               |
| Scripted OBCS viewer peer: attach-binding fields, `Empty`→`record` poll, detach release, reject/unavailable/malformed/correlation/disconnect | `terminal read` success + typed viewer-stage failures | `tests/cli_query.rs`                                  |
| Real staged-cohort Server + live TUI terminal: `terminal read` returns its grid and schema-validates          | real read-only observation coexisting with the TUI's own viewer | `tests/local_startup_process`                                      |
| Non-TTY; stdout blocked after the session is gone; >8 MiB prepared output on every format; real write failure | documented exits; transport owners dropped before blocking      | `tests/local_startup_process` + emit unit tests                    |
| `config check` matrix: pass/fail/not-applicable per check, `exists` semantics                                 | five typed checks always reported                               | `tests/cli_query.rs` + `src/cli/config` tests                      |
| Completion scripts project the grammar (bash/zsh executed, fish structural parity)                            | positive and negative contexts; no runtime enumeration          | `tests/cli_query.rs`                                               |
| Bilingual text/failure output                                                                                 | exact en-US/zh-CN strings                                       | `tests/process.rs` + `tests/cli_query.rs`                          |

Model obligations: this slice adds no new Server state machine, persistence, or retry and reuses
the registered OBLC/OBWP/OBCS/Client models — `terminal read` rides the existing
`terminal.viewer` attach/poll/detach protocol unchanged — so no new TLA+ model is required.
Registered models whose triggers cover `apps/orchbee` source, manifest, project scope, README,
and test paths must still actually run; the inventory output is not a substitute.
