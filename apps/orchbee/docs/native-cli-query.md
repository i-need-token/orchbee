# Native CLI Query

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` pastes one command and sends Enter in one input request. Supply one quoted string; text is preserved, and targets are exact IDs or `@terminal:<suffix>`. The receipt confirms input submission, not Shell readiness, command completion or exit status; never replay partial or unknown delivery. [Command submission contract](native-terminal-run.md).

Status: current implementation and correctness contract for the native `orchbee`
command line, covering queries, Workbench mutations, terminal observation and input,
Agent names, and local provider onboarding. Queries observe an existing Site; Workbench
mutations follow the mutation contract below. Only explicit `site start` and the TUI
startup path may activate a Site. Agent state comes from the Server's canonical model,
never terminal-text inference; the Server owns names, while `agent integration` manages
only the invoking user's local provider configuration. Each command's effects, budgets,
and release boundaries are specified in its section. CLI commands need no TTY; the
interactive TUI has separate terminal admission requirements. The native CLI remains
an unpackaged development surface; Desktop's `apps/cli` is a separate delivery entry.

Authority: this record owns the public grammar, result envelopes, failure matrix,
budgets, exchange state machines and output effects implemented under `src/invocation.rs`
and `src/cli/**`. Prose and implementation must converge in the same change.

## Workbench topology queries

Windows `workspace list/get` return `number`, `tabCount`, and `panelCount`;
`tab list/get` return `number` and `panelCount`. Workspace numbers are one-based
positions in the complete canonical navigation order, before `--project` filtering.
Tab numbers are one-based positions within their terminal context, independent of
manual or automatic labels. Reordering changes numbers without changing identities.
Empty contexts have zero Tabs and Panels. A Panel is one leaf of a Tab's split tree;
child presentations and terminal bindings never increase its count.

`panel list` includes `workspaceId` (null for standalone), plus the primary
presentation's `label`, nullable `customLabel`, and `displayLabel`. These are the
same canonical values returned by `panel get`, not inferred process or Agent titles.
JSON preserves labels exactly; text escapes them and bounds list display width.

These projections consume one validated Workbench snapshot and retain its source
revision, existing-only admission, bounded output, and headless availability. They
perform no navigation, process inspection or mutation. Schema versions are
`orchbee.workspace.list/get` 2, `orchbee.tab.list` 2, `orchbee.tab.get` 3,
and `orchbee.panel.list` 3; the embedded catalog is version 76. Client focus and
active selections, process/Agent observations, worktree facts and reported metadata
require their respective owners and are not implied by these topology fields.

Verification covers global reorder and filtering without renumbering, empty and
nested trees, child presentations and unbound leaves, list/get agreement, automatic
labels, escaped text and schema validation of real Windows query documents.

## Windows named Site selection

The global `--site <name>` / `--site=<name>` selects the local Windows Site for the entire
invocation, including `site start`, `site stop`, queries and `tui`. It does not change the
Agent `--session` selector. Names are case-sensitive, contain 1–64 ASCII letters, digits,
`.` / `_` / `-`, and cannot be `.` or `..`. `default` selects the existing default Site.
The complete argument grammar is validated before selection; offline help, completion,
version and schema operations do not discover a runtime because a name was supplied.

Selection precedence is explicit `--site`, the managed terminal's `ORCHBEE_RUNTIME_SCOPE`,
`ORCHBEE_SITE`, then `default`. `ORCHBEE_RUNTIME_BASE` still selects the existing absolute
base. The Worker supplies the admitted base and scope to child terminals, so nested CLI
commands keep their owning Site even if a different name was inherited by the Server.
An explicit `--site default` overrides that inherited scope. Invalid selected environment
values fail discovery; they never silently select the default Site.

Named runtime leaves hash the current SID and exact name; names are never filesystem path
components. Activation registers a bounded private identity under its existing activation
lock before committing the startup marker. Read-only discovery validates that identity
without creating it. Checkpoint storage is already isolated by the admitted runtime path.
`session list [--format text|json]` lists default followed by exact named sessions in the
selected runtime base. It reads at most 256 sessions under one five-second deadline, creates
no runtime state and reports running, stopping, stopped, absent, starting, deleting or recovery-required
from authenticated metadata and admitted lifecycle facts. Unknown/unsafe inventory fails explicitly.

`session attach <name> [--mouse on|off]` activates or reconnects the real TUI. `session stop
<name> --confirm` reuses `site stop`, including its timeout, detach, request-id and retry options
and its existing result schema. `session delete <name> --confirm` removes only that stopped
named session's retained checkpoint and owned runtime files; it refuses default, running or
uncertain state and never deletes Project/worktree/user directories. A name operand and explicit
`--site` must agree. Delete is idempotent when absent. A partial removal remains fenced by a
deletion intent and an explicit failure; repeat delete to finish, never start to repair it.
Deletion checks Ctrl+C before each filesystem phase and exits 130 on interruption. Completed removals are not rolled back; a pending intent remains resumable. The `interrupted` result reports whether an intent remains, including cancellation after cleanup completed.

List and delete have `orchbee.session.list` and `orchbee.session.delete` result schemas.

The durable catalog preserves exact default/named identities and stopped receipts in LocalAppData.
With the same admitted runtime base, `session list` still discovers a stopped session after its
runtime leaf is removed; `session attach`/`site start` recreates its route and restores the checkpoint.
Active or orphaned state remains recovery-required, never inferred stopped from missing files.
Pending deletion also survives loss of temporary metadata and remains resumable by the same delete
command. Listing remains read-only. The runtime base itself is not implicitly recreated.

## Agent session names

`agent names` lists all live aliases in the running Site. `agent resolve <target>`
returns a full session ID, testing exact ID existence before aliases. `agent rename
<target> <new-name>` assigns or atomically replaces a name; `agent rename <target>
--clear` releases it without stopping the session. Replace the target operand with
`--session <id>`, `--name <alias>` or `--panel <id|@panel:suffix>` for explicit selection; exactly one selector is
required. Names match `[a-z][a-z0-9_-]{0,31}` and are case-sensitive. They are unique,
sharing a 512-entry cap with pending launches, and released when the session ends, is removed, or the
Site runtime is disposed. Restarting a Site starts with an empty alias table.

All three commands admit `--format text|json`. The embedded success schemas are
`orchbee.agent.names` version 2 (with separate pending reservations), and
`orchbee.agent.resolve` and `orchbee.agent.rename`, version 1;
post-admission failures use `orchbee.cli.failure.agent-names`, version 1. Presentation resolution and ambiguous-target failures use version 2; launch-pending failures use version 3; earlier outcomes keep version 1. Correlated
semantic answers carry an `agent-names` source with the OBAP request ID. These four documents are included in schema catalog version 64, alongside the prompt contracts. Invalid names or conflicting selectors
fail with exit 2 before any runtime access. A collision is exit 3 and identifies the
incumbent session; it never overwrites that session's name.

Each command owns one five-second deadline spanning cohort validation, existing-only
Site discovery, admission, optional Panel resolution and the final OBAP exchange. Signals participate in transport
waits; the session closes before signal arbitration and output. A confirmed reply
remains authoritative if a signal races its completion. A submitted mutation without
a trustworthy matching reply returns outcome-unknown (exit 8), including cancellation,
and is never replayed. A known pre-submission cancellation is exit 130. No alias
cache, provider process, new Site, or canonical-session mutation is involved.

`get`, `explain`, and `wait` accept the same positional target or explicit
`--session`/`--name`/`--panel` selector. Exact retained IDs take precedence over names; explicit
IDs and opaque tokens that cannot name aliases or Panels go directly to the read. Other targets first send
Resolve on the same admitted Client Session. Resolution and reads share the original
deadline and signal owner, without a second admission or renewed budget. A wait bounds
initial resolution by `min(remaining_ms, 5000)` and pins the resolved ID before
polling; releasing, renaming or reusing an alias never retargets it. Success publishes
only the actual read's `agent-read` source. Resolution refusals use their own
`agent-names` source and failure schema, never the identity of an unexecuted read.
Windows process evidence covers real pipe admission, resolved reads, pinned polling,
the shared five-second deadline, explicit-ID reads, schema-valid failures,
lost/mismatched mutation replies and parse-before-effect.

## Agent terminal operations

`agent read <target>` and `agent send-keys <target> <key>...` accept the same
Session ID, runtime name or Panel selectors as `agent get`, including explicit `--session`,
`--name` and `--panel`. Read admits `--history --rows <n>`, `--unwrap`, and `--lines <n>`
with the same bounds and meaning as `terminal read`. Send admits at most 64 semantic
keys or modifier combinations and validates the entire sequence before any effects.

The CLI composes existing owners: one existing-only Client Session reads a Workbench
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
Agent lifecycle transition, new Site, alias cache, input ownership, or per-turn causal
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
uses the same Session/name/Panel selectors as other Agent commands. Text is non-empty UTF-8,
bounded by the 256 KiB paste limit and the command's 64 KiB argv envelope. `--until`
and `--timeout` require `--wait`; Windows wait defaults to idle/done/blocked without a total
timeout, and explicit finite timeouts are at most one hour. Without waiting, the budget is five seconds.

One existing-only Client Session owns snapshot lookup, one resolved Agent identity,
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

Schema catalog version 68 contains 76 documents, including the prompt success and failure contracts.

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

Wait version 3 on Windows adds nullable `matchedChange`. Null means the current sample matched; otherwise the historical change is the matching evidence, while `revision` and `session` remain the separate latest Get sample. The historical record never overwrites current status. `source.requestId` identifies the matching Get or changes read. Requested condition order is preserved. `--after-revision` remains a global revision fence, not turn causality. Linux wait and prompt success retain version 2. Finite/infinite failures keep their existing version and null timeout distinction.

`blocked` matches fresh needs-attention; `done` currently matches fresh succeeded/failed/canceled/interrupted. Client-relative unseen enrichment is a separate dependent consumer and is not implied by the change feed. Evidence covers transient states, unrelated sessions, ordered pages, reset refusal, retained identity, deadline and cancellation cleanup.

Windows `agent wait --client <id>` pins one exact Client as well as the resolved Agent. It uses the Client attention join above: idle/done match only that Client's current corroborated projection, while other conditions preserve canonical current/change-page matching. A missing/stale Client version cannot satisfy idle/done. Historical canonical events never invent a historical Client acknowledgement; selected-Client idle/done are sampled, not replayed. Loss of the selected Client ends the wait rather than choosing another.

Selected-Client wait uses v4 with a required `client` projection and existing nullable `matchedChange`; unselected Windows wait remains v3. Each Client read, cursor proof and canonical sample shares one five-second evaluation bound within the original total deadline. Reads remain valid under Busy without acknowledging or navigating. Finite expiry and cancellation preserve the existing wait failure/finalization contract. Explicit canonical status conditions may return historical evidence even when current Client status differs.

Selected-Client composition keeps missing/stale Client selection failures in `orchbee.cli.failure.client-control` v1. Other Client read failures use that envelope v2 with required `stage: client-agent-read`; their kind distinguishes denial, rejection, lost transport, invalid reply, deadline and cancellation. Existing Agent and admission failures keep their original contracts.

## Inputs and trust boundary

- `argv`: bounded hostile input. Total UTF-8 bytes across arguments may not exceed 64 KiB
  (65,536). Any non-UTF-8 argument, or any overflow, is an `invalid-usage` failure with empty
  stdout even when another argument spells `--format=json`. No stdin or file input is read
  implicitly — the only exceptions are the explicitly declared `terminal send --file`/`--stdin`
  sources under [Send input sources](#send-input-sources), materialized once before any runtime
  effect.
- Environment: `ORCHBEE_RUNTIME_BASE` (optional; defaults to `/tmp`) selects the runtime base;
  `LC_ALL`/`LC_MESSAGES`/`LANG` select the diagnostic locale. `agent doctor` additionally reads
  `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` (the observation-endpoint override its endpoint fact
  reports verbatim), the inherited `PATH` for its bounded executable scan, and `HOME` for the
  provider target prefixes on Linux. Windows uses the native user profile and an optional
  non-empty `CODEX_HOME` for Codex configuration. `panel current` in caller mode additionally reads
  `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID` as described below. Nothing else is consulted.
- Filesystem: the resolved runtime base, the Site scope directory
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
- `--directory <directory-id>`: single-use value flag, required on `worktree list` and valid
  nowhere else. The value is a `WorkbenchDirectoryId` only — a strict nonzero 32-lowercase-hex
  identifier, never a filesystem path, label, or prefix. A missing, malformed, uppercase,
  zero-valued, or wrong-width value is `invalid-usage` before any runtime access; the CLI
  performs no directory-membership precheck — membership is the running Site's authority, and
  a well-formed but absent directory answers `not-found` with `{resource:"directory"}` under
  the worktree failure envelope.
- `--standalone`: valueless boolean flag, single use, only on `tab list` and `tab create`;
  mutually exclusive with `--workspace` on both.
- `--label <label>`: single-use value flag, required on `project create`, `project rename`, legacy `workspace create`, `workspace rename` and `tab rename`. Windows `tab create` and `panel split` allow omission, using the preflight Tab/Panel count plus one. Explicit values are nonempty protocol-bounded Workbench labels; empty or oversized values are `invalid-usage`.
- `--path <path>`: single-use value flag, required on `workspace accept-directory` and also admitted by Windows Worktree Create/Open. The value is a `WorkbenchDirectoryPath`: a literal absolute local-platform path
  (POSIX `/` or an absolute Windows drive path) of at most 4,096 UTF-8 bytes without NUL. The CLI never expands `~` or `$`,
  never creates or repairs the directory, and never probes the filesystem — a missing,
  relative, NUL-bearing, or overlong value is `invalid-usage` before any runtime access, and
  every filesystem fact is the Server's authority after submission.
- `--direction <word>`: single-use value flag, required unless `--insert-index` is used on `tab move`
  (`previous|next|first|last`), `panel split`, and `panel move` (`above|below|left|right`).
  Each word set is closed; a panel word on `tab move` or a missing/unknown word is
  `invalid-usage`.
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
  executable path. At most 128 `--arg` values and at most 2,048 UTF-8 bytes across the
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
  present on any other command is `invalid-usage`.
- `--match <text>`: single-use value flag on `terminal wait-output`, one of the two search
  inputs — exactly one of `--match` or `--regex` is required. A nonempty UTF-8 literal up to
  4096 bytes, matched case-sensitively. A missing, empty, non-UTF-8, or oversized value —
  or supplying both search flags — is `invalid-usage`.
- `--regex <pattern>`: single-use value flag, the alternate `terminal wait-output` search
  input; a nonempty UTF-8 regular expression of at most 1024 bytes compiled once before
  any runtime access under a 1 MiB program-size limit and a 1 MiB lazy-DFA cache limit.
  A missing, empty, oversized, or non-compiling value — or coexistence with `--match` — is
  `invalid-usage`.
- `--timeout <ms>`: single-use value flag, optional on `terminal wait-output` (default
  `30000`) and on `agent wait` (Windows defaults to no total timeout, Linux to `600000`); one decimal `u32` in `1..=3600000`.
  A repeated, non-numeric, or out-of-range value is `invalid-usage`.
- `--rows <n>` on `terminal wait-output` is the searched retained-row window `1..=1000`
  (default `1000`) covering the most recent visible+history rows as one range — unrelated
  to the geometry flag of the same name.
- `--target-panel <target>`: single-use value flag, required on legacy same-Tab `panel move` and optional with `--into-tab`; parsed with
  the `panel` target rule (a complete typed ID only) and resolved inside the moved panel's
  own Tab for legacy moves, or the destination Tab with `--into-tab`.
- `--history`: valueless flag admitted only on `terminal read`, legal only paired with
  `--rows <n>` — on `terminal read` the `--rows` spelling carries the retained-row count
  `1..=1000` instead of its geometry meaning, which stays the contract on `tab create`
  and `panel split`. Either flag without the other, a repeat, a non-numeric or
  out-of-range count, or `--history` on any other command is `invalid-usage` before
  any runtime access.
- `--unwrap`: valueless flag admitted only on `terminal read`; a read-only presentation
  transform that joins verified soft-wrap row chains into logical lines. Hard breaks and
  empty lines are preserved; a window edge inside a wrapped line reports
  `startsMidLine`/`endsMidLine` instead of claiming a complete line.
- `--lines <n>`: single-use value flag admitted only on `terminal read`; a
  presentation-only tail of the last `n` emitted entries `1..=65535` — logical lines under
  `--unwrap`, physical rows otherwise. It never widens the fetched window, and its
  `tail{requested,omitted,entries}` accounting stays separate from the byte-budget's
  `omittedRows`. A repeated, non-numeric, zero, or out-of-range value is `invalid-usage`.
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
- `--cursor <c>`: single-use value flag, admitted only on `agent list`; the minted
  `v1.<59>.<43>` continuation token, carried verbatim — authenticity is verified
  Server-side, never at the CLI. A malformed token is `invalid-usage` before any
  runtime access.
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
- `--version`: legal only as the sole root invocation; any other position or company is usage.
- `-h`/`--help`: root sole token, or the single suffix of a recognized help path (a family
  name or a complete command path).
- `--` positionals: tokens after `--` are positional only, so `orchbee -- status` queries
  status and `orchbee project get -- --help` resolves the literal label `--help`.

Command paths (fixed words, no abbreviations or aliases):

```
orchbee                         # bare → TUI (unchanged)
orchbee tui [--mouse off|on]    # TUI
orchbee --version               # version line (unchanged)
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
orchbee workspace close <target> --confirm
orchbee workspace close-status --project <id> --workspace <id>
orchbee workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm
orchbee worktree list --project <target> --directory <directory-id>
orchbee tab list [--workspace <target> | --standalone] | tab get <target>
orchbee tab create [--workspace <target> | --standalone] [--label <label>] [<geometry>] [--exec <path> [--arg <value>]... | --profile <selector>] [--focus | --no-focus] [--client <id>] [--timeout <ms>]
orchbee tab rename <target> --label <label>
orchbee tab move <target> (--direction previous|next|first|last | --insert-index N)
orchbee tab close <target> --confirm
orchbee panel list [--tab <target>] | panel get <target>
orchbee panel rename <target> (--label <text> | --clear)
orchbee panel split [<target> | --panel <target> | --current] --direction above|below|left|right [--label <label>] [--ratio <number>] [<geometry>] [--exec <path> [--arg <value>]... | --profile <selector>] [--focus | --no-focus] [--client <id>] [--timeout <ms>]
orchbee panel close <target> --confirm
orchbee panel move <target> --target-panel <target> --direction above|below|left|right
orchbee terminal list | terminal get <target> | terminal read <target> [--history --rows <1-1000>] [--unwrap] [--lines <1-65535>]
orchbee terminal wait-output <target> (--match <text> | --regex <pattern>) [--rows <1-1000>] [--timeout <1-3600000>]
orchbee terminal send <target> (--text <utf8> | --file <path> | --stdin) [--enter] | --key <name>
orchbee launch-profile list | launch-profile get <selector>
orchbee launch-profile create --name <name> --exec <path> [--arg <value>]...
orchbee launch-profile delete <selector>
orchbee agent list [--include-ended] [--limit <1-100>] [--cursor <v1.…>]
orchbee agent get <target> | agent explain <target>
orchbee agent read <target> [--history --rows <n>] [--unwrap] [--lines <n>]
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
orchbee panel focus [<target>] [--panel <target> | --current] [--client <id>]
orchbee panel zoom [<target>] [--panel <target> | --current] [--toggle | --on | --off] [--client <id>]
orchbee api snapshot
orchbee api schema [<schema-id>] [--output <path>]
orchbee site start
orchbee site stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>]
orchbee site stop-status <request-id>
orchbee site diagnose
orchbee config check
orchbee completion bash|zsh|fish|powershell|elvish
orchbee --skill
orchbee help <recognized path>
orchbee <recognized path> -h|--help
```

Help topics: root; families `client`, `project`, `workspace`, `worktree`, `tab`, `panel`, `terminal`, `launch-profile`, `agent`,
`agent integration`, `api`,
`site`, `config`, `completion`; and every complete command path including `tui`. `help` takes only a recognized
path (no filters, no format). A trailing `-h` or `--help` is the same request on the same path:
`orchbee --help`, `project --help`, and `project get --help` emit exactly what `help`,
`help project`, and `help project get` emit. Suffix help takes precedence over missing business
arguments (`project get --help` describes `project get`) but not over supplied business
arguments or extra tokens (`project get x --help`, `status extra --help` are usage). Unlisted
targets such as `help help`, `help version`, `help --version` are unknown paths → usage. Help,
`--version`, `--skill`, `completion`, and `api schema` without `--output` never touch platform, runtime, cohort, or Site state. Explicit Windows schema export touches only the caller-selected file destination and its owned sibling temporary.
`--skill` must be the sole root argument and emits the localized, embedded CLI usage guide.
PowerShell completion is a static grammar projection: it never executes user expressions or
queries a Site; loading it is explicit and does not modify a shell profile.

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
script and help body below 256 KiB to flag unexpected grammar growth. Load explicitly in interactive Elvish 0.21 with
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

`help`, `completion`, `tui`, `--version`, `--skill`, and the bare invocation are fixed text outputs that do
not accept `--format`; a single valid `--format=json` turns the resulting syntax failure into a
JSON `invalid-usage` failure document, while a text/default failure keeps stdout empty. Data
commands (`status`, all `list`/`get`, `workspace close-status`, `agent explain`,
`agent doctor`, `agent wait`, `api snapshot`, `api schema`, `site start`, `site stop`, `site stop-status`,
`site diagnose`, `config check`, and all four `agent integration` commands) and every
mutation command accept `--format`. The `api` commands are the machine surface: a successful `api snapshot` or
`api schema` emits the identical JSON document under both formats, while an `api` failure
follows the selected format like any other command — under text/default it reports on stderr
with empty stdout, and only `--format json` produces a failure envelope.

`orchbee completion` prints a static projection of this grammar: verbs, complete help paths,
each command's declared flags in both the separate and the joined `--flag=value` spelling,
both `--format` spellings and its `text`/`json` values, the closed value vocabularies under
either spelling (`--mouse`, `--direction`, `--key`, `--until`, `--provider`), `--`
termination, and flag-value consumption contexts. It enumerates no Site state, resource
names, or runtime data, and never guesses a freeform value.

### Targets and filters

`<target>` operands parse in three forms. First a complete typed ID (exactly 32 lowercase
hexadecimal characters, nonzero), which matches by identity only. Second, an explicit selector
`@<kind>:<suffix>` — `<kind>` one of `project`, `workspace`, `tab`, `panel`, `terminal`, and
`<suffix>` one to thirty-one lowercase hexadecimal characters — which resolves to the one
member of that kind in the read snapshot whose identifier ends with the suffix. A recognized
selector prefix with an empty, uppercase, or 32-digit suffix, or a kind that does not match the
operand's resource, is `invalid-identifier` (exit 2) before any runtime access; any other `@`
token keeps the label reading — only the five prefixes are selector syntax. Any other token is
an exact, case-sensitive label candidate requiring 1–512 Unicode scalars when the resource
admits labels; empty or overlong tokens and the all-zero ID shape are `invalid-identifier`
(exit 2). A label that itself has ID or selector shape can never be reached by label or selector
matching — the entity remains reachable through its real ID. `panel` and `terminal` accept
complete typed IDs and their own `@panel:`/`@terminal:` selectors only; anything else is
`invalid-identifier`. Filters (`--project`, `--workspace`, `--tab`) use the same per-resource
target rules, and so do every mutation operand and `--target-panel` (the `panel` rule, resolved
inside the moved panel's own tab). `api schema` takes a nonempty schema ID; an empty one is
`invalid-identifier` with `resource="schema"`, an unmatched one is `not-found`.
`agent get`, `agent explain`, and `agent wait` take one positional token of 1–128
characters drawn from `[A-Za-z0-9._:-]` (or an explicit Panel prefix), resolved by the ID-first rules above, or an
explicit `--session`/`--name`/`--panel` selector. Malformed tokens are `invalid-usage` before runtime access.
`launch-profile get` and `launch-profile delete` take one positional `<selector>`, and
`--profile` carries the same token: a `LaunchProfileSelector` — a nonempty UTF-8 literal
of at most 512 bytes (the name bound, so every admitted selector can reach either rule).
An empty or overlong token is `invalid-usage`; the CLI never splits it into an id/label
case — the Server resolves it under the catalog lock: an exact profile ID first, then a
name whose match cardinality must be exactly one. Zero matches answer `not-found`, a
name matching more than one entry answers `ambiguous` with at most the first 8 candidate
profile IDs plus the true `totalCount` — both under the dedicated launch-profile failure
envelope.

Resolution after a successful read: zero matches → `not-found` (3); multiple exact label or
selector-suffix matches → `ambiguous` (3) reporting at most the first 8 candidate typed IDs in
canonical traversal order plus the true `totalCount`. Selector candidates are the members
visible in the one snapshot the command already read — complete for every operand's declared
scope, never an extra fetch — and a selector is a momentary handle, not a durable one: deletion
and recreation can put the same suffix on a different member, so commands needing stable
identity use the full typed ID. A filter naming an absent entity is `not-found`; an existing
entity with an empty result set is a successful empty list.

## Query state machine and budgets

Every network query (`status`, `list`/`get`, `api snapshot`) performs exactly this ordered
sequence on Linux; the first failing stage wins and determines the failure:

1. **Cohort** — `NativeCohort::current` under one checked 5-second monotonic deadline
   (`Instant::now().checked_add(5s)`; a failed `checked_add` is `internal-failure`, exit 1).
   Any cohort error (missing, invalid, unsafe, hash mismatch, cooperative deadline) is
   `check-failed` stage `cohort`. The localized stderr diagnostic additionally reports
   the observed `io::ErrorKind` — and the raw OS error number when one exists — without
   paths, OS message text, or a causal claim; the machine record is unchanged.
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
5-second cohort budget for its cohort check. `site diagnose` is the same kind of purely local
command — descriptor-relative path and lock observation only; it never connects, never creates
a lock or a marker, and never repairs — over one cooperative 5-second budget checked between
bounded filesystem operations. Platforms other than Linux and Windows produce `service-unavailable`
`{stage:"platform",reason:"unsupported-platform"}` for network queries and mutations and
`check-failed` with the same details for `config check`; there is no fallback to unbounded
connection APIs. `site start` outside Linux/Windows and `site diagnose` outside Linux/Windows answer
`service-unavailable` with the same platform details.

`agent doctor` is the offline provider-hook diagnostic: it explains, from bounded filesystem
facts alone, why Agent observations may not arrive. It performs no provider execution, no
socket connect, no Site discovery, no lock, and no write of any kind — the inspection set is
the declared one above, and every lstat/read/list/PATH scan runs under one 5-second
cooperative budget, ≤64 probe items, ≤16 MiB aggregate read, and per-file/count caps, with
cancellation checkpoints between items. It answers `check-failed` with
`{stage:"platform",reason:"unsupported-platform",command:"agent doctor"}` on platforms other than Linux and Windows. A run that meets a bound still produces its document with `complete:false` —
partial facts are explicit, never silently dropped — and `providers[]` always reports every
selected provider in canonical order. `hints[]` are observation-only: each `{id,evidence,text}`
names the concrete fact rows it cites and carries no causal, remedy, or trust claim.

### Windows Agent diagnostic refinement

Windows `agent doctor` owns the same bounded offline observation workflow. It emits
`orchbee.agent.doctor` version 3; existing Linux output remains version 1, and the schema
retains the prior version-2 vocabulary. Capsule 0.4.16 supports native Windows Codex hook
transport, but doctor never connects to a named pipe or writes console correlation hints.
Its endpoint remains `not-inspected`, now with reason `windows-endpoint-not-inspected`;
configuration and capsule integrity are not delivery or Site-liveness evidence. `complete`
describes probe coverage. Windows managed installation is owned by the [Windows integration contract](windows-agent-integrations.md); Windows Agent startup is specified in [Windows Agent startup](windows-agent-start.md); prompt composition is specified above.

| Field               | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Inputs and owner    | Native doctor exclusively owns read-only filesystem/PATH/environment observations. Windows home uses the native user profile; a non-empty absolute `CODEX_HOME` naming an existing, non-reparse directory selects Codex configuration. Empty `CODEX_HOME` uses the default; invalid non-empty roots are uninspectable, never silently replaced. This read-only admission is stricter than Codex canonicalizing a relative or symlinked override. |
| Admission           | Regular files are opened without following the final reparse point, then checked through the opened handle before bounded reads. Device namespaces and relative paths are rejected. Reparse entries are uninspected. Ancestor components and synchronous filesystem latency remain outside a hard deadline guarantee.                                                                                                                            |
| Decisions           | Provider configuration absent/disabled/conflicting/unreadable, native executable or script visible/absent, Node visible/absent, capsule consistent/corrupt/over-budget, and an uninspected Windows endpoint are separate facts. PATH search never executes an entry or infers plugin command support.                                                                                                                                            |
| Compatibility       | Windows accepts native absolute paths and single-line TOML literal strings emitted for Windows paths. Existing Linux scanner semantics and output version remain unchanged. New Windows endpoint vocabulary is confined to version 3 in the embedded schema; version 2 retains its earlier closed vocabulary.                                                                                                                                    |
| Effects and bounds  | No provider execution, socket connection, Site activation, lock or write. Existing 5-second cooperative budget, 64 items, 16 MiB aggregate reads and per-probe caps apply. Cancellation returns exit 130; produced complete or partial diagnostic documents return exit 0.                                                                                                                                                                       |
| Evidence and limits | Windows isolated process tests cover localized/schema-valid output, path/configuration isolation, missing and installed declarations, corrupt assets and zero setup effects. Filesystem tests cover reparse refusal and read budgets. Doctor evidence does not prove hook delivery; no independent review is claimed; TLA execution is excluded by user instruction.                                                                             |

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

`terminal read --history --rows <n>` keeps the same attach → poll preamble — `--history` and
`--rows` are admitted only as a pair, and `n` is the bounded retained-row count `1..=1000`, not
a geometry dimension — then reads the scrollback behind the applied frame:

- The applied frame's retained-scrollback fact prices exactly one `ReadRows{attachmentId,
startRow,rowCount,revision,generation,requestSequence:1,maximumBytes}`: `rowCount =
min(n, scrollback, 262144/columns)` and `startRow = scrollback - rowCount`, so the window ends
  at the scrollback boundary. Zero scrollback short-circuits — no `ReadRows` is sent and the
  answer is an empty window.
- The `record` answer must be a complete OBTP `RowsSnapshot` (kind 46) response whose OBRW text
  window independently echoes the request: attachment, Host epoch, sequence, revision,
  generation, columns, start row, row count, and `totalRows = scrollback + screenRows`, with the
  payload inside `maximumBytes` (8 MiB). Any mismatch is `protocol-invalid` stage `viewer`.
- `rows-rejected` maps to a typed vocabulary: `limit-exceeded` is `rejected` (3); Host state
  answers (`history-evicted`, `stale-revision`, `stale-generation`, `missing-resource`,
  `resource-exhausted`, `invalid-phase`) are `service-unavailable` (4) with
  `{stage:"viewer",operation:"read-history",reason}`; reasons the fixed request shape cannot
  produce are `protocol-invalid`. There is no retry, and no `RowsApplied`/feedback anchor is
  ever sent for the read.

`terminal read --unwrap`/`--lines` are presentation transforms applied to the already
materialized emitted rows — they change neither the fetched window nor the read bound:

- `--unwrap` joins only adjacent, verified soft-wrap-compatible rows (`wrap` and
  `wrap_continuation` agreeing pairwise) into logical lines; the JSON `content` then emits
  `lines[]` — `{firstRow,lastRow,text,startsMidLine,endsMidLine}` — instead of `rows[]`,
  plus `lineCount` counted over the observed fragments only (never the retained-history
  total). `rowCount` keeps counting physical rows. A window that opens or ends inside a
  wrapped line yields a flagged fragment, never a claimed complete line.
- `--lines <n>` tails the emitted entries: physical `rows[]` without `--unwrap`, logical
  `lines[]` with it. The `tail{requested,omitted,entries}` member reports the presentation
  drop separately from `omittedRows` (the fetch/budget omissions). Text output prints one
  line per emitted entry and a localized “earlier rows/lines not shown” marker distinct
  from the read-limit marker.
- Either flag lifts the envelope to `version:2` (`orchbee.terminal.read` /
  `orchbee.terminal.history`); flag-free output stays `version:1` byte-identical.

`terminal wait-output` reuses the same snapshot admission and attach preamble, then keeps the
read-only viewer alive for a bounded observation loop instead of a single frame:

- **Deadline** — one absolute deadline `T0 + --timeout` is taken at command start
  (default 30000 ms, admitted range `1..=3600000`). Cohort, discovery, connect, snapshot, and
  every `terminal.viewer` request each run under `min(5 s local budget, remaining deadline)`;
  the deadline is re-checked after every awaited step, so a late success is still `timeout`
  (exit 7). The deadline is cooperative: synchronous filesystem calls cannot be interrupted
  mid-syscall, so the worst-case overshoot is one in-flight local operation.
- **Observe** — `Poll` repeats inside the deadline: `Empty` sleeps 10 ms, backing off to
  100 ms on consecutive empties and resetting on any record (each sleep is capped by the
  remaining deadline — a quiet wait never busy-loops); a `record` feeds the projection, whose
  `FrameApplied` feedback is sent for **every** applied frame and confirmed with
  `FeedbackAccepted` before the next poll; `NeedSnapshot` follows the original feedback rule;
  `ProjectionBlocked` is consumed and ignored.
- **Inspect** — each applied frame pins one `ReadRows` over the unified buffer window
  `startRow = max(0, totalRows − K)`, `rowCount = K` where `K = min(--rows, totalRows,
262144/columns)` and `totalRows = scrollback + screenRows`, at `.exact` revision and
  generation of that frame, `maximumBytes` 8 MiB. `requestSequence` is strictly increasing per
  attachment starting at 1 (`checked_add`; a retry consumes a fresh value, and
  `transport requestId`, frame sequence, and history-event sequence are never mixed in); the
  decoded window must echo the issued sequence plus every pinned field, otherwise
  `protocol-invalid`. `stale-revision`/`stale-generation`/`history-evicted` discard that
  observation and resume polling inside the same deadline — no half-window is ever matched;
  other `rows-rejected` reasons map through the same typed vocabulary as `read-history`
  (operation `"wait-output"`). A payload the row codec cannot encode fails as a typed
  `rows-rejected` — it is never silently truncated.
- **Match** — window rows join into logical lines only when `wrap` and `wrap_continuation`
  agree pairwise; disagreement is a hard boundary and window-edge fragments stay honest
  partials (never padded from outside the window). Each observed fragment is searched
  independently — nothing matches across a hard boundary or an unfetched gap. The first
  fragment containing the pattern wins: the literal, case-sensitive `--match` substring, or
  the regex engine's leftmost match under `--regex`. `^`/`$` anchor at the observed
  fragment's edges only, and a valid zero-length match (`^$` on an empty observed line) is
  a hit. The returned `excerpt` is a ≤4096-byte slice of that fragment that always contains
  a complete `--match` literal; a `--regex` match may exceed the bound, in which case the
  excerpt leads with the match's start and the extended members carry the truth —
  `matchLength` the complete byte length, `matched`/`matchedTruncated` the ≤4096-byte match
  text, `startsMidLine`/`endsMidLine` the fragment-edge facts, all under
  `patternKind:"regex"` (envelope `version:2`; a literal hit stays `version:1`). Offsets
  (`matchOffset`, `excerptOffset`) are UTF-8 byte offsets into the joined fragment, never
  terminal columns. A match ends the command with the physical
  `rowStart`/`rowEnd` (observation-scoped, not a cursor), the frame `revision`/`generation`,
  the resolved terminal/worker/site identity, and the observed `hostEpoch`/`attachmentId`;
  `observed.completedObservations` counts only consistent window+match passes, and
  `observed.windowTruncated` records a capacity-clamped `K`.
- **Cancel** — HUP/INT/QUIT/TERM during the wait is a local `cancelled` failure (exit 130):
  no match is fabricated and no input, semantic cancel, or kill is sent to the terminal.
  Checks run between network steps and around each poll sleep, so latency is bounded by one
  in-flight local request.
- **Release** — on success the `Detach` outcome is confirmed before the document is emitted;
  an unconfirmed cleanup keeps the real failure kind and reports
  `{matchObserved:true,cleanup:"unconfirmed"}` instead of converting it. Timeout, carrier
  failure, and cancel keep their primary failure plus the cleanup state; the session drop
  always runs and is reported as `cleanup:"requested"` when no Detach budget remains —
  never claimed as released.

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
   fresh request identity using the snapshot's Site, Worker, and terminal facts. `--text`
   produces one UTF-8 record (4096 bytes including optional CR); `--file`/`--stdin`
   produce one opaque byte record (1 MiB including optional CR), retaining the text path
   for small UTF-8 content. `--paste` instead produces one paste record (256 KiB) with
   an optional separate Enter record. Repeated `--key` produces up to 64 semantic-key
   records in argument order; keys are protocol names, never canned escape strings.
   The Client Session is fully released before the typed result is mapped or output
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

1. **Resolve** — the `--project` target is resolved on the validated snapshot under the
   ordinary project target rule (exact ID or exact label; `not-found`/`ambiguous` keep the
   generic failure envelope — the snapshot boundary is the command's family boundary).
   Directory membership is never checked here.
2. **Read** — exactly one `workbench.worktree-list.read` request `{projectId,directoryId}`
   under a fresh `WorkbenchRequestId`, bounded by the same deadline, on the same admitted
   session. The Server authoritatively verifies the directory's membership in the project at
   that moment and dispatches one bounded Worker discovery job — a `git rev-parse
--path-format=absolute --git-common-dir` probe followed by `git worktree list --porcelain
-z`, sharing one 5-second job TTL and one 64 KiB combined stdout budget; the stage that
   crosses the shared budget owns the `too-large` attribution. The read is never retried,
   replayed, or reconnected; an undetermined exchange is `outcome-unknown` (8).
3. **Map** — the session is fully dropped before the typed `worktree-list.result` is
   projected. `Found` yields the `orchbee.worktree.list` document: the registered
   `{projectId,directoryId,path}` identity plus verbatim `entries` (`worktreePath`,
   `head`, `branch`, `bare`, `detached`, `locked`, `prunable` — byte-exact through the
   `*BytesHex` alternates). Every failure the exchange produces — including transport and
   protocol defects — wears the dedicated `orchbee.cli.failure.worktree` envelope;
   `not-found{resource:"directory"}` is the absent-directory answer, and no partial entry
   set ever surfaces.

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
they do not simulate physical Ctrl+C. TLA execution is deferred.

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
admitted Client Session they resolve the target when necessary as described above,
then perform one Agent Observation Protocol (OBAP) read exchange:

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
3. **Release** — the Client Session is fully dropped before the outcome is mapped or
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

`agent wait` holds one admitted Client Session. Windows follows canonical changes as described
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
true once reached. On a match the emitted `orchbee.agent.wait` envelope (version 2)
carries `matchedConditions`, the satisfying `revision`, the full `session` detail
verbatim, and a `wait` echo `{until, afterRevision?, timeoutMs, observations,
elapsedMs}`; its `source` is the decisive poll's `agent-read` requestId only — failed,
cancelled, and untrusted exchanges publish none. Text output is a single line:
`session <id> matched <conditions> at revision <n> after <ms>ms`.

## Site activation

`site start` is the headless form of the TUI's local startup: both call the single shared
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
   marker, take the Site lifetime lock, refuse `rejected` with
   `{stage:"activation",reason:"recovery-required"}` when the marker or sockets show no live
   owner, commit the marker,
   spawn `orchbee-server --local-site <runtime-directory>` from the validated cohort, then
   poll connect plus `try_wait` until readiness. A spawn or `try_wait` failure is the same
   `recovery-required` refusal — the marker may already be committed, so the answer is never
   a clean-timeout claim.
5. **Probe** — the admitted probe `LocalClient` is dropped before output; a success implies
   no held session, grant, or Site identity receipt.

`result.outcome` reports the branch this invocation actually took: `started` only when it
spawned the Server child, `already-running` when an existing Site admitted the probe. A
`timeout` answer means readiness stayed unconfirmed — before marker commit nothing was left
behind, between commit and spawn a marker without a live owner may persist (the next
invocation then answers `recovery-required`), and after spawn a Server may still be running;
the command never claims a timeout proves no Server started, never retries `recovery-required`,
and never repairs or deletes runtime state. Signals stay inherited: no capture stage exists,
so an interruption is process death under the conventional `128 + signo`, never a `cancelled`
envelope. Windows explicit stop and subsequent start are described below; there is no repair verb.

## Site diagnostics

On Windows, `site diagnose` emits `orchbee.site.diagnose.windows` v2, a separate
closed schema preserving the Linux v1 document. The native catalog is v69/76.
The same existing-only runtime path/SID scope is used without cohort admission,
activation, endpoint connection or process discovery. Ordered rows describe the
selected base, admitted runtime directory, marker, activation/lifetime locks and
five named-pipe addresses. Windows has no filesystem Host socket scope row and
does not invent POSIX mode/UID facts. `private` means current-user ownership and
the runtime DACL allowlist were checked, not a fresh security guarantee.

Version 2 distinguishes a present marker's `activation: active|stopped`; Stopped
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

`site diagnose` is the read-only counterpart of activation: it reports bounded facts about the
implicit per-euid local Site runtime without connecting to anything, creating anything, or
repairing anything. The runtime base resolves exactly like `site start` (`ORCHBEE_RUNTIME_BASE`
or `/tmp`, scope `orchbee-native-<euid>`), then the command inspects a finite set of known
paths through the rendezvous' own validated directory descriptor — `AT_SYMLINK_NOFOLLOW`
descriptor-relative `fstatat` plus, for the Site lifetime lock only, an open-existing-file
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
held the lock at that observation instant — never that a Site, a Worker, or any process is
alive — and `absent` is a pathname fact, never proof that an execution ended. The Host
directory is admitted before the Host endpoint name is inspected; an unadmitted parent closes
the endpoint row `uninspectable`/`unsafe-parent` without traversal. `complete` is the
attempted-or-classified coverage fact — `false` only when the cooperative 5-second budget was
spent between operations — never a runtime-health claim.

`result.advisories` carries the conditional advisories the current activation predicate's own
facts justify: `activation-recorded` when a fully valid marker was observed, `socket-residue`
when a fully valid client or worker socket leaf was observed — each glossed in text as
"activation recorded" or "socket residue observed; if no existing Site can be joined, fresh
activation is blocked by current recovery policy". The advisory quotes current policy over
observed facts; it never predicts the result of a nonatomic future `site start`, which
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

Canonical traversal order: `navigation.projects` order; each project's `workspaces` order;
`terminalContexts` order then `tabs` order; panels depth-first with `children[0]` before
`children[1]`; terminal bindings in that panel order with `primaryPresentation` before
`childPresentations`. No sorting is added; filtered lists and `ambiguous` candidates preserve it.

Snapshot and one-effect command envelopes use `{schema, version, source, result}`; staged workflows have their own embedded schemas. Every schema defines its emitted version and strict shape. `orchbee.panel.list` and `orchbee.tab.get` are v2; `orchbee.panel.get/current` are v3 after adding Panel input policy; `orchbee.api.snapshot` is v4 with optional automatic Tab naming. `orchbee.terminal.get` is v4 for the retained launch program/arguments. `orchbee.terminal.read/history` use v2 with `--unwrap` or `--lines`; `orchbee.terminal.wait-output` uses v2 for regex match details. Their flag-free outputs remain v1. See `api schema <id>` for each command and failure's complete version contract. The current catalog is v74 with 83 required documents: registering an ID changes the closed dictionary shape, independently of business document versions. Sources retain their owner's meaning:

- `source.kind="workbench-snapshot"` with `siteId` and decimal-string `revision` from the same
  validated snapshot — present only after a complete valid read.
- `source.kind="workbench-command"` with `siteId`, `requestId`, and decimal-string
  `expectedRevision` — produced mutation results, and command-stage failures after a valid
  read.
- `source.kind="embedded"` — the compiled-in schema dictionary (`api schema` results and its
  `not-found` failure).
- `source.kind="local-runtime"` — produced `config check` results, `agent doctor` results, every `site start`
  answer, and every `site diagnose` document: the local runtime is the answering authority
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
- `orchbee.project.list` → `{projects:[{projectId,label,directoryCount,workspaceCount}]}`;
  `orchbee.project.get` → `{project:{projectId,label,directories:[{directoryId,workerId,path}],
workspaces:[{workspaceId,label}]}}`.
- `orchbee.workspace.list` → `{workspaces:[{workspaceId,label,number,tabCount,panelCount,projectId,
primaryDirectoryId|null,terminalContextId}]}` flattened across projects; `workspace get` adds
  `tabs:[{tabId,label}]` resolved through `terminalContextId`.
- `orchbee.workspace.close-status` → the verbatim retained `WorkbenchWorkspaceCloseReadOutcome`:
  `{kind:"not-closing"}` when no retained operation covers the scope, or
  `{kind:"available",closeRequestId,projectId,workspaceId,terminalContextId,
attemptGeneration,status,counts:{closing,cleanupBlocked,outcomeUnknown,
disposedAwaitingPublication},members:[{tabId,panelId,presentationId,status}…]}`. `status` is
  the scope lifecycle (`kind` `closing`|`cleanup-blocked`|`outcome-unknown`|
  `disposed-awaiting-publication`|`closed` with its tombstone `revision`), each member's
  `status` is the captured per-member lifecycle truth without `closed` (the closed tombstone
  is scope-level only), `attemptGeneration` is the decimal-string attempt fence, and the
  record stays answerable after the Workspace leaves the active projection.
- `orchbee.tab.list` → `{tabs:[{tabId,label,number,panelCount,pinned,groupId|null,
context:{kind:"standalone",terminalContextId}|{kind:"workspace",terminalContextId,
workspaceId}}]}`; `workspaceId` is absent on standalone, never null. `tab get` adds `root`, the
  typed `PanelNode` subtree re-encoded without losing semantic fields.
- `orchbee.panel.list` → `{panels:[{panelId,label,customLabel|null,displayLabel,workspaceId|null,tabId,terminalContextId,primaryPresentationId,
presentationCount,primaryTerminalId|null}]}` where `primaryTerminalId` reflects only
  `primaryPresentation.binding`; `panel get` → `{panel:{panelId,tabId,context,slotPath,
primaryPresentation,childPresentations}}` with `slotPath` the `PanelSlot` IDs from root to leaf
  (`[]` when the root itself is the panel) and each presentation `{presentationId,label,
binding:{terminalId,workerId}|null}`.
- `orchbee.terminal.list` → `{terminals:[{terminalId,workerId,boundTo:{presentationId,panelId,
tabId,terminalContextId}}]}`; `terminal get` returns the same identity facts under
  `terminal`, which additionally carries a nested `launch` member — the document is
  `{terminal:{terminalId,workerId,boundTo,launch}}`, never a sibling `result.launch`. Only
  presentation-bound terminals are visible; a binding
  does not prove the process lives. No lifecycle/process/agent fields exist anywhere else.
  Unlike `terminal list`, `terminal get` does not answer from the Client-held snapshot: it sends
  a `terminal.launch.read` request on the admitted session and the Server resolves the current
  binding, routing, and retained launch fact under the coordinator lock — the snapshot only
  supplies the typed target id. Its envelope is `version:4` with
  `source:{kind:"terminal-launch-read",siteId,revision}` where `revision` is the joined
  workbench revision as a decimal string; older `version:1` validators reject it, which is the
  deliberate compatibility boundary. `launch` is
  `{state,workerGeneration,createOutcome,geometry}`: `state` is the retained phase verbatim —
  `creating`, `awaiting-start`, `starting`, `started`, `unknown`, `unavailable` — or
  `not-tracked` when no uniquely attributable consistent record exists for the visible binding;
  `workerGeneration` is a positive decimal string, `null` only for `not-tracked`; `geometry` is
  non-null only for `started`; `createOutcome` echoes the retained create outcome, `null` for
  `creating` and `not-tracked`. When the launch named a catalog profile the terminal gains one
  more sibling member `launchProfile:{profileId,catalogRevision}` — the resolved provenance
  verbatim, absent (never `null`) for explicit `--exec` and default-shell launches; the
  envelope is `version:4`. The state is a retained fact, never process liveness:
  `creating` records a phase, not proof of dispatch, and a deferred-start failure reads
  `unavailable` without a fabricated start reason. Exit codes stay the query contract —
  `not-found` 3, `server-busy` unavailable 4 — and a `found` answer whose
  `launch.state` is `unavailable` with `createOutcome.kind` `failed` still exits 0 because the
  query itself succeeded. The exchange rides the admitted session under the same 5-second
  network budget shared with admission and the snapshot read: a dropped or timed-out exchange
  is `transport-loss` 5, and a correlation mismatch or invalid result is `protocol-invalid` 5 —
  the launch read never reports `timeout`.
  Explicit programs and named-profile launches also return optional sibling `launchSelection:{exec,args}` with the actual program and arguments frozen at creation. Profile deletion and Panel movement never rewrite it. Default-shell and `not-tracked` facts omit it; environment values are not exposed. Text output uses localized labels for the program and JSON argument array. This read does not guarantee launch-command persistence across restart.
- `orchbee.terminal.read` → `{terminal,frame,content}`: `terminal` carries the same
  `{terminalId,workerId,boundTo}` identity facts; `frame` carries the applied projection's
  `{revision,generation,sequence}` as decimal strings (u64-safe), `alternateScreen`, `geometry`
  `{columns,rows}`, and `cursor` `{row,column,visible}`; `content` carries `rows` (the emitted
  grid rows as `{index,text,continuation:{fromPrevious,intoNext}}` — `text` joins the row's
  cells, dropping wide-cell spacer tails and trimming grid padding), `rowCount` (every row the
  applied frame held), `truncated`/`omittedRows`, and `limits` `{rows:4096,textBytes:4194304}`.
  Under `--unwrap` the emitted `rows` member is replaced by `lines` — each
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
  with the same `--unwrap`/`--lines` substitutions as `terminal read` (`lines` with
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
- `orchbee.agent.list` → the `listed` outcome verbatim:
  `{kind:"listed",items:[<session summary>…],nextCursor|null,revision}` where each summary
  is the wire `AgentSessionSummary` (session/provider/lifecycle/semantic status/freshness/
  observation counters/execution location/pending-request count ≤128/presentation
  attachments) and `nextCursor` is the Server-minted continuation token for `--cursor`.
- `orchbee.agent.get` → the `found` outcome verbatim:
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
  `session` is that poll's wire `AgentSessionDetail` verbatim. Windows v3 additionally carries
  nullable `matchedChange`, separating historical matching evidence from that sample; `wait` echoes the
  effective run `{until,afterRevision?,timeoutMs,observations,elapsedMs}` —
  `afterRevision` present only when supplied.
- `orchbee.api.snapshot` → `{snapshot}`: the complete `WorkbenchProtocolDocument::Snapshot`
  semantic re-encode including `kind:"workbench.snapshot"`; envelope `siteId`/`revision` agree
  with the inner document.
- `orchbee.api.schema` → `{schema:{schemaId,document}}` for a named schema, or
  `{schemas:{schemaId:document,…}}` listing the whole delivered dictionary (draft 2020-12).
  The catalog stays closed: `result.schemas` names exactly the registered ids and each
  embedded document's `$id` matches its key. Introspection clients validating against the
  older catalog select the embedded v2 document from this same build.
- `orchbee.worktree.list` → `{projectId,directoryId,path,worktrees:[<entry>…]}`: the
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
  `name` on `launch-profile.create`; the envelope is `version:4`.
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
  `already-running` (the branch this invocation took, not a Site identity receipt), `scope`
  is `orchbee-native-<euid>`, and `runtimeBase` carries exactly one of `path` (UTF-8) or
  `pathBytesHex` (non-UTF-8, never a lossy display string).
- `orchbee.site.diagnose` → `{complete,scope,runtimeBase,runtimeDirectory,marker,locks,
endpoints,hostScope,advisories}`: the bounded observation document described in Site
  diagnostics — closed row states and reasons, `locks:{activation,siteLifetime}`,
  `endpoints:{client,worker,clientPending,agentObservation,hostEndpoint}`, `path`/`pathBytesHex`
  on the two directory rows, and the conditional `advisories` token list.
- `orchbee.cli.failure` → `{schema:"orchbee.cli.failure",version:1,failure:{kind,details?},
source?}`; no English `message` field exists. `source` is attached only from an already trusted
  source (a validated snapshot for post-read failures, `embedded` for `api schema` not-found,
  `workbench-command` for every command-stage mutation failure, `local-runtime` for every
  `site start` answer including its refusals, `terminal-launch-read` for `terminal get`
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
| `runtime-directory` | `not-applicable` when the base failed or the scope directory is absent under a valid base (a Site that is not running is not a configuration error); pass on a validated existing directory; fail on unsafe owner/mode/type or I/O error.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
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
   Client Session is then fully released before the outcome is mapped or output prepared.

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

`workspace accept-directory` submits the existing `WorkspacePrimaryDirectoryAccept` action —
the same operation the TUI directory wizard uses — so a headless
`project create → workspace create → workspace accept-directory → tab create` sequence needs
no TUI. The target resolves like any Workspace (typed ID first, then exact label; ambiguous
labels are rejected), and the true `projectId` comes from the same snapshot row as the
resolved Workspace. `--path` is parsed at the CLI boundary into a `WorkbenchDirectoryPath`
(literal absolute path, ≤4,096 UTF-8 bytes, no NUL; `~`/`$` stay literal) — the CLI never
creates, repairs, canonicalizes, or probes it, so an absent or non-directory path submits
normally and surfaces through the Server's `DirectoryUnavailable` precommit as
`service-unavailable` (exit 4). The Server remains authoritative over navigation mutation,
directory association, worker probing and generation, optimistic revision, and resource
limits: a Workspace that already holds a directory — including the same path — is rejected
(`workspace-directory-conflict`, exit 3), and the CLI neither pre-checks `primary_directory_id`
nor infers an idempotent success. A `committed` receipt carries the new `revision`, the
Server-allocated `directoryId` and `workerId`, the resolved `projectId` and `workspaceId`,
and the request's literal `path` (never a canonicalized form).

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

Mutation results use `schema:"orchbee.mutation.result"`, `version:4`, with
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
`closeRequestId`, and the resolved `projectId`/`workspaceId`/`terminalContextId`; the Site
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

Deliberately not delivered: `project close`, `panel resize`, terminal open operations, Site lifecycle
restart, disable, recovery or repair commands (Windows stop/status are supported), and every remote operation.

## Failure matrix

Exit codes: 0 success (empty lists included); 1 local config/output/internal determinate failure;
2 usage/invalid target; 3 not-found/ambiguous/denied/rejected; 4 service-unavailable (including
`session-capacity`); 5 transport-loss/protocol-invalid; 6 incompatible; 7 `timeout`
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
| OBLC `Rejected(CapacityExceeded)`                                                                                                         | `session-capacity`                                                         | 4         | `{stage:"admission"}`                                                                               |
| OBLC `Rejected(HandshakeTimeout)`                                                                                                         | `service-unavailable`                                                      | 4         | `{stage:"admission"}`                                                                               |
| `Rejected(IncompatibleProtocol                                                \| IncompatibleCohort)`, `Protocol(IncompatibleVersion)`    | `incompatible`                                                             | 6         | `{stage:"admission"}`                                                                               |
| `Rejected(WrongMessage                                                        \| MalformedFrame)`, `UnexpectedResponse`, other `Protocol` | `protocol-invalid`                                                         | 5         | `{stage:"admission"}`                                                                               |
| `RequestIoConfiguration`, `SessionUnavailable`, `RequestEncoding`                                                                         | `internal-failure`                                                         | 1         | omitted                                                                                             |
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
| `DefinitelyNotSubmitted::SessionUnavailable`/`RequestEncoding`                                                                            | `internal-failure`                                                         | 1         | omitted                                                                                             |
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
| worktree-list outcome `NotFound` (directory absent from the project at the Server check)                                                  | `not-found`                                                                | 3         | `{resource:"directory",target}` under the worktree envelope                                         |
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
| send `SessionUnavailable`/`RequestEncoding`                                                                                               | `internal-failure`                                                         | 1         | omitted                                                                                             |
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
| agent `SessionUnavailable`/`RequestEncoding`                                                                                              | `internal-failure`                                                         | 1         | omitted                                                                                             |
| send source validation: unopenable/non-regular path, terminal or unsupported stdin, empty, oversized                                      | `invalid-usage`                                                            | 2         | omitted (localized stderr names the declared option and concrete cause)                             |
| send source deadline expired during materialization                                                                                       | `timeout` under `orchbee.cli.failure.source`                               | 7         | `{stage:"input",inputSource,timeoutMs}`                                                             |
| eligible signal captured during send source materialization                                                                               | `cancelled` under `orchbee.cli.failure.source`                             | 128+signo | `{stage:"input",inputSource}`                                                                       |
| send source signal setup/read/restore failure                                                                                             | `internal-failure`                                                         | 1         | omitted                                                                                             |
| prepared output over 8 MiB                                                                                                                | `output-too-large`                                                         | 1         | `{limitBytes:8388608}`                                                                              |
| stdout write/flush failure                                                                                                                | `output-failed`                                                            | 1         | no second stdout document                                                                           |
| `config check` finishes with failures                                                                                                     | success schema `ok:false`                                                  | 1         | five runtime checks preserved; Windows adds user-config                                             |
| `site start` cohort `InvalidData`/`NotFound`/`PermissionDenied`/`TimedOut`/other io kind                                                  | `incompatible`/`service-unavailable`/`denied`/`timeout`/`internal-failure` | 6/4/3/7/1 | `{stage:"cohort"}`                                                                                  |
| `site start` socket path over the `sockaddr_un` budget                                                                                    | `service-unavailable`                                                      | 4         | `{stage:"runtime-directory",reason:"address-too-long"}`                                             |
| `site start` connect/admission failure                                                                                                    | the `ConnectClientError` rows above, verbatim                              | 4/5/6     | `{stage:"admission"}`                                                                               |
| `site start` marker or sockets committed without a live owner; Server spawn or `try_wait` io failure                                      | `rejected`                                                                 | 3         | `{stage:"activation",reason:"recovery-required"}`                                                   |
| `site start` activation deadline elapsed; lock acquisition `TimedOut`                                                                     | `timeout`                                                                  | 7         | `{stage:"activation"}`                                                                              |
| `site start` marker/probe/lock io failure (non-timeout)                                                                                   | `internal-failure`                                                         | 1         | `{stage:"activation"}`                                                                              |
| `site diagnose` complete or partial fact document — unsafe/uninspectable rows are facts, not failures                                     | typed `orchbee.site.diagnose` document                                     | 0         | row `state`/`reason` fields                                                                         |
| `site diagnose` outside Linux/Windows; network query, mutation, `site start`, `config check`, or `agent doctor` outside Linux/Windows     | `service-unavailable`/`check-failed`                                       | 4/1       | `{stage:"platform",reason:"unsupported-platform"[,command]}`                                        |
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
  path that exits 0 without a Site. Echoed untrusted tokens reuse the quoted/escaped
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
- Offline commands (`help`, `--version`, `--skill`, `completion`, `api schema`) never perform platform,
  runtime, cohort, or Site discovery.
- A network query creates nothing: no directory, lock, marker, terminal, viewer, control grant,
  or second request — except `terminal read`/`terminal wait-output`, whose read-capability
  viewer attachment is the single sanctioned read-path side effect, held minimally and
  released before output. At most one short-lived Client Session is held, and none survives
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
- `site start` is the single activation surface and the one command that creates local-runtime
  state: it may create the runtime directory, hold activation/lifetime locks, commit the marker,
  and spawn the Server — only a grammar failure guarantees zero effect. Its absolute 20-second deadline
  is never reset per stage; a `recovery-required` refusal is never retried, repaired, or
  cleaned up; a timeout never claims no Server started. `site diagnose` is the observation
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
| No Site on disk                                                                                                                                                                                                   | `service-unavailable`, zero fs/process/lock mutation                                              | `tests/cli_query.rs`                                                |
| Real staged-cohort Server: status/list/get/api snapshot, 8 admitted + ninth rejected + release, live TUI                                                                                                          | success/`session-capacity`/unchanged structure                                                    | `tests/local_startup_process`                                       |
| Scripted OBLC/OBWP peers: denied, unavailable, malformed, wrong correlation, timeout, disconnect                                                                                                                  | typed kinds per matrix; never labeled real-Server evidence                                        | `tests/cli_query.rs`                                                |
| Scripted OBCS viewer peer: attach-binding fields, `Empty`→`record` poll, detach release, reject/unavailable/malformed/correlation/disconnect                                                                      | `terminal read` success + typed viewer-stage failures                                             | `tests/cli_query.rs`                                                |
| Scripted OBCS viewer peer + hand-built OBRW records: scrollback pricing, capacity/scrollback clamps, typed `rows-rejected`, window mismatch, oversized record, no retry                                           | `terminal read --history` success + typed failures                                                | `tests/cli_query.rs`                                                |
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
| Worktree list grammar: `--project`/`--directory` both required, strict directory-ID value rule, flag scopes                                                                                                       | exact parse/`invalid-usage` before runtime access                                                 | `src/invocation` tests                                              |
| Launch-profile grammar: four command paths, verbatim selector, `--name`/`--exec` required pair, `--profile` exclusivity and scope                                                                                 | exact parse/`invalid-usage` before runtime access                                                 | `src/invocation` tests                                              |
| Launch-profile document/failure envelopes: entry member names, closed details vocabulary, dual `source` shapes                                                                                                    | every fixture validates; rejected variants fail                                                   | schema contrast tests                                               |
| Worktree list document/failure envelopes: entry XOR byte fields, bare/detached/HEAD invariants, closed details vocabulary                                                                                         | every fixture validates; rejected variants fail                                                   | `tests/cli_query.rs` schema contrast tests                          |
| Worker discovery job: real disposable Git repository (init + commit + detached linked worktree, per-command `-c` identity only), shared TTL/stdout budget attribution                                             | verbatim `-z` bytes, real HEAD/ref, typed outcomes                                                | `apps/orchbee-worker` `worktree_discovery_job` tests                |
| Server worktree-list handler: `-z` parser acceptance/rejection matrix, membership authority, outcome→failure mapping                                                                                              | byte-exact projection; malformed listings are `check-failed`                                      | `apps/orchbee-server` `worktree_list` tests                         |
| Non-TTY; stdout blocked after the session is gone; >8 MiB prepared output on every format; real write failure                                                                                                     | documented exits; transport owners dropped before blocking                                        | `tests/local_startup_process` + emit unit tests                     |
| `config check` matrix: pass/fail/not-applicable per check, `exists` semantics                                                                                                                                     | five runtime checks; Windows additionally checks user configuration                               | `tests/cli_query.rs` + `src/cli/config` tests                       |
| `agent doctor` grammar: provider/asset-root admission, dedup, caps, help paths; synthetic fixture → total-order verdicts (codex ladder, guarded/capsule/root/socket probes), closed vocabularies                  | deterministic offline facts; emitted document validates; no provider execution or socket connect  | `src/invocation` tests + `src/agent_doctor` tests                   |
| `site start` real staged cohort: cold start spawns + admits, second/overlapping starts, marker-without-owner refusal, held-lock deadline, cohort faults, signals, non-UTF-8 base, zh-CN text                      | `started`/`already-running` truth; refused wedge leaves marker byte-identical; bounded deadline   | `tests/local_startup_process`                                       |
| `site diagnose` grammar + observation matrix: format-only options, every row state/reason, conditional advisories, held/free/unsafe lifetime lock, unadmitted parent, spent budget, zero creation                 | exact parse; closed fact rows; no connect/create/repair; no liveness claim                        | `src/invocation` tests + `src/cli/site_diagnose` + rendezvous tests |
| Completion scripts project the grammar (bash/zsh executed, fish structural parity)                                                                                                                                | positive and negative contexts; no runtime enumeration                                            | `tests/cli_query.rs`                                                |
| Bilingual text/failure output                                                                                                                                                                                     | exact en-US/zh-CN strings                                                                         | `tests/process.rs` + `tests/cli_query.rs`                           |

Model obligations: this slice adds no new Server state machine, persistence, or retry and reuses
the registered OBLC/OBWP/OBCS/Client models — `terminal read` rides the existing
`terminal.viewer` attach/poll/detach lifecycle, with `--history` adding one bounded
`read-rows`/`rows-rejected` exchange inside it, and `terminal wait-output` looping that same
poll plus one bounded `read-rows` per applied frame — so no new Server-side protocol model is
required. The command's own settlement gate — per-step flag sampling, cleanup provenance, and
the unconditional late sample — is modeled separately in
`formal/tla/NativeCliWaitSettlement`; see `native-cli-wait-model.md`. Registered models whose
triggers cover `apps/orchbee` source, manifest, project scope, README, and test paths must
still actually run; the inventory output is not a substitute.

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
cwd, argv and environment remain within the existing 8192-byte frame bound.

Evidence covers constructor/decoder rejection, legacy and extended roundtrips, combined
maximum sizes, CLI scope and redaction, Server composition and replay identity, and a
Windows child observing cwd/environment after public Tab and split commands. No new
macOS/Linux adapter or execution claim is included; TLA execution remains deferred.

Native launch commands use a dedicated 48 KiB JSON request budget so maximum escaped cwd, argv, label and environment fit together. OBWP command kind 2 admits this bound before allocation; non-launch intents retain 16 KiB and results retain their existing bounds. OBCS v2 frames admit 48 KiB; OBCN remains 8 KiB. The OBCS v2 schema/corpus covers these launch values; shared Workbench schemas and the Desktop binding still do not establish support for the native extensions.

## Calling Panel context

`panel current [--panel <panel-id-or-selector> | --current] [--client <id>]` reads a
Windows Panel using the [current Panel contract](native-current-panel.md), shared with
`panel process-info`. Explicit Panel selectors ignore caller environment and need no Client.
Explicit `--client` ignores caller environment and conflicts with explicit Panel selectors.
Otherwise, only two absent caller variables permit unique current Client fallback; partial,
malformed, or stale caller information fails. Selected Clients must be Ready with a Panel still
belonging to their reported Context and Tab. Ambiguous Client selection requires `--client`.

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

The current Panel result uses `orchbee.panel.current` v3 with its snapshot source, matching the
Panel Get projection. The five-second query deadline, cancellation, read-only Client fallback,
Viewer cleanup for process queries, and claim-mapped Windows evidence are owned by the linked
contract. Catalogue v69 contains 78 schemas. No activation, input, control, or focus effects;
Desktop and other-platform runtime additions remain excluded. TLA execution remains deferred.

Windows Worker identities are fresh per Server connection using the existing OS random source;
one Worker shares that ID across its terminals. Terminal IDs can repeat across Sites, so exact
Worker matching rejects callers from a previous or different Site. No reconnect or adoption is
introduced. Tab close acceptance is not removal: lookup reflects membership until the Server
publishes removal, after which the old binding is not found.

## Directory Workspace creation

The Windows `workspace create` [contract](windows-workspace-creation.md) owns optional Project, label, source, directory and focus. Legacy explicit `--project <target> --label <label>` creates an empty Workspace; adding an absolute `--cwd` with no new options keeps one native Server request (`workspace.launch`) and mutation v4. Omitted operands, relative paths, directory policies or new source/Client/focus/timeout options select enhanced launch. Env/program/profile/geometry options without cwd also use the new defaults. Enhanced `orchbee.workspace.creation` v1 preserves first-Project setup, Workspace and focus separately; catalog v69 contains 78 schemas and 94 command paths.

The Worker probes the requested directory before the Server atomically registers the Workspace, directory, Terminal Context and first Tab/Panel/Presentation at one revision. Failed admission creates none of them. A successful legacy commit returns the existing mutation v4 envelope with action `workspace.create`, `projectId`, `workspaceId`, `directoryId`, `terminalContextId`, `tabId`, `panelId`, `presentationId`, literal `path`, `label`, and `terminal`. The existing schema already admits these fields; its version and catalog are unchanged. Terminal launch can remain pending or fail after commit: keep the returned IDs, inspect the Workspace and explicitly close it if unwanted. An unknown response is never automatically replayed or rolled back.

The unpublished native wire action requires `projectId`, `label`, `geometry`, and `options.cwd`; optional `launch` and `profile` remain mutually exclusive. It uses the existing 48 KiB launch request limit, exact request ledger and terminal settlement. `workspace-launch-pending` and `workspace-launched` echo the committed revision and all seven IDs; only the latter carries `terminalOutcome`. The Server owns all six new structural IDs and the terminal binding. Shared Workbench schemas and Desktop bindings do not establish this native extension's support. Activate and roll back the same complete binary cohort. The Server's [refinement record](../../orchbee-server/README.md#directory-workspace-launch) owns admission, commit and recovery.

## Agent Panel targeting model

Targeted Agent commands accept `--panel <id|@panel:suffix>` exclusively with positional,
`--session` and `--name` selectors. A positional full Panel ID first resolves as an exact
retained Session ID or runtime name; only a definite not-found permits Panel fallback.
Explicit Session/name selectors never fall back. A prefixed `@panel:` target selects a Panel
without consulting aliases. Workbench owns Panel identity and its primary presentation;
Agent state owns live, navigable presentation attachments. Hidden attachments remain eligible.
The CLI reads one Workbench snapshot on the held Client Session and passes the primary
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
Real Site/Worker/ConPTY evidence covers the integrated command path. These checks do not
establish provider foreground ownership or readiness. TLA execution remains deferred.

## Panel process information

### Windows interpretation and Agent input preflight

`src/cli/processes.rs` owns a bounded, stateless interpretation of the validated Host
sample. The Host alone owns process collection. Each process adds nullable `argv` and
`providerId`; `foreground` reports `kind`, nullable `pid`, and nullable `providerId`.
The process result is version 2. Windows `CommandLineToArgvW`, through the existing
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

On Windows, `agent send-keys` and `agent prompt` resolve the canonical session and its
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

`panel process-info [<panel-id>] [--panel <panel-id> | --current]` reads the Windows Job of the Panel's primary Terminal. Positional and explicit Panel targets are exclusive; complete IDs and `@panel:<suffix>` use the existing Panel resolver. Without an explicit target, validate `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID` before admission, then derive the caller's Panel from one Workbench snapshot. Missing or stale caller context fails rather than guessing a focused client. The primary presentation must have a terminal binding; children are not fallback candidates.

One existing-only Client Session and one absolute five-second budget cover snapshot, transient read-only viewer admission, one process read, and best-effort detach. Session drop owns remaining viewer cleanup. The Server revalidates Worker generation and physical terminal binding; Worker and Host retain process authority. Success publishes the exact typed Windows Job sample with Panel/Terminal routing context and correlated read provenance. The sequential snapshot and sample do not claim an atomic cross-owner observation. Host failures remain explicit, are never empty process success, and are not retried. Strings are escaped in text output; machine creation times remain decimal strings. The response describes owned processes, not a Windows foreground process group or Agent interactive readiness.

The JSON envelope is `orchbee.panel.process-info` v2; the schema catalog and bilingual help/completion ship with the command. Evidence covers implicit and explicit targets, conflicts before I/O, primary-versus-child selection, absent primary bindings, denied reads, wrong response identities, preserved Host errors, bounded output, viewer cleanup, and the real Windows Site/Worker/Host process route. The shared typed read is also the actual dependency for subsequent Agent launch/readiness work; it does not close those obligations by itself.

Expected Host refusals use `orchbee.cli.failure.process-info` v1 with the bounded failure code; invalid responses and transport failures retain the generic CLI failure contract. Successful `source` records snapshot revision, OBCS request ID, and Host correlation, epoch, attachment, and physical Terminal ID as distinct identities. Catalog version 68 contains 78 schemas.

## Windows Agent startup

[Windows Agent startup](windows-agent-start.md) owns `agent start`, its reservation and readiness workflow, Windows quoting, budgets and recovery. `agent names` v2 includes separate pending reservations; names failures use v3 for launch-pending. The catalog is v69 with 78 schemas.

## Windows Site stop

Managed Windows clean stop saves Workbench configuration before disposing terminals. A later start
restores the retained layout and Launch Profiles and launches fresh default shells using Workspace
directory defaults. Save failure leaves the previous checkpoint and running terminals intact;
explicitly retry the failed stop after resolving the failure. Corrupt data refuses startup rather
than replacing it with an empty Workbench. See the owning [checkpoint contract](../../orchbee-server/docs/workbench-checkpoint.md)
for storage and recovery limits: this does not restore processes, Agent state, Client views,
terminal history or each shell's observed cwd, and it does not add named-session commands.

`site stop --confirm [--request-id <32-hex>] [--retry] [--detach] [--timeout <ms>]` stops the existing managed Windows Site without activating one. The ID is a nonzero lowercase 128-bit value, generated once if omitted; `--retry` requires it. The CLI issues one start/retry request, then only status reads. `--detach` returns the admitted attempt immediately; otherwise one absolute deadline (65000 ms by default, 1–3600000 ms) covers runtime/cohort admission, request and polling. Cancellation or timeout stops waiting, never rolls back the admitted shutdown. Every runtime answer retains the request ID so `site stop-status <id>` can query it. Status is a fact document, not a second mutation. The `orchbee.site.stop` v1 envelope distinguishes stopped, pending, failed, busy, not-recorded, unmanaged and unavailable/unknown transport outcomes; pending answers may carry detached, timed-out or cancelled wait disposition. Known Server failures preserve attempt, phase, cause and per-attempt disposal count. Only the matching strict stopped activation record plus a free existing lifetime lock proves completion; disappearance, invalid records and a different ID never do. A later start consumes this receipt; not-recorded cannot disprove an earlier stop. Linux/macOS return unsupported-platform without runtime effects.

## Windows retained Worktree operations

The [Worktree contract](windows-worktrees.md) owns `worktree create/open/remove/status/resume`, their source/Project inference, per-command flags, operation identity, confirmation, waiting, output and recovery. It extends this record's earlier list-only grammar: Create/Open admit optional --client, --focus/--no-focus, label and terminal geometry, source Workspace or cwd, and path/branch selectors; Create also admits base. Remove admits force and requires Workspace plus confirmation; Resume requires operation ID, generation and confirmation. All five admit text/JSON; only mutation commands admit request identity, trust and waiting as documented there. The `orchbee.worktree.operation` v1 (no focus) / v2 (explicit focus) / v3 (Project setup) / v4 (shell restoration) envelope is included in catalog v69 (76 documents).

Windows `worktree list` additionally accepts a source Workspace/cwd and optional command-local trust, with validated caller/selected-Client defaults and cwd only when both are absent and no Project requirement. The earlier registered-directory grammar and envelope in this record apply only to the legacy `--project/--directory` form. The [source discovery contract](windows-worktrees.md#source-based-discovery) owns the separate `orchbee.worktree.query` result (catalog v69/76), mutual exclusions, total deadline, failure exits and known Workspace associations.

## Workspace group Close projection

Windows `workspace close <target> --group --confirm` uses the separate retained group contract; ordinary Close keeps its existing grammar and document. Group Close accepts `--request-id`, `--detach` and `--timeout` (1–3600000 ms, default 120000). Group status uses `workspace close-status --group --request <operation-id>` with a five-second pure-read budget. Group Resume uses `workspace close-resume --group --request <operation-id> --generation <1..64> --confirm`, with the same detach/timeout options. `--project` and `--workspace` cannot accompany group mode. These are options on the existing three verbs; the native command inventory contains 88.

The `orchbee.workspace.group-close` v1 envelope separates retained Server truth from nullable caller waiting disposition. Its source is `local-workspace-group` with required-nullable Site ID. Start/Resume is sent once; subsequent requests are reads, pinned to the same attempt and captured member identities. Loss after handoff remains unknown, never automatic replay. Exits are 0 for Pending/Closed, 3 for rejected or known failed closure, 4 for unavailable/Worker loss/Site stop, 8 for unknown outcome, 1 for internal or inconsistent observation, and 5/7/130 for transport loss/timeout/cancelled waiting while retaining the last reply. [Group ownership and recovery](windows-worktrees.md#workspace-group-closure) define membership, limits and child observations. Every schema is embedded; help, `--skill` and four completion projections include the group forms without probing a Site.

## Windows Client navigation

[Windows Client view control](windows-client-control.md) owns `client list/get/control-status`, `workspace focus`, `tab focus`, `panel focus` and `panel zoom`. Navigation targets one current TUI via `--client` or unambiguous validated caller context. It submits once and waits for a drawn acknowledgement; timeout or Ctrl+C stops waiting only. Single-panel zoom has an explicit no-op receipt. Catalog v69 includes 78 schemas, adding `orchbee.client.view`, `orchbee.client.control` and `orchbee.cli.failure.client-control`; view/control are v1, routing failure is v1 and composed Agent Client read failure is v2. No operating-system window activation or Mac/Linux control runtime is added.

## Windows offline schema export

`api schema [<schema-id>] --output <path>` explicitly publishes the same complete JSON document that the command writes to stdout. It accepts `--format text|json` for failure presentation; successful stdout and file bytes remain identical JSON with a final newline. Omitting `--output` retains the existing read-only behavior.

The parser admits a nonempty literal file path of at most 4,096 UTF-8 bytes, validates all arguments before effects, and never expands variables or a leading tilde. Relative paths resolve against the invoking process directory. Windows device names, alternate data streams and ambiguous drive-relative paths are refused. This does not select or start a Site, inspect runtime configuration, or use the network to obtain schemas. New Mac/Linux file-export runtimes are not included.

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

`panel layout`, `panel neighbor --direction left|right|up|down` and `panel edges` inspect an exact Windows TUI Client. They accept a positional Panel, `--panel` or `--current`, and `--client` when routing is ambiguous. Reads preserve focus and terminal sizes, including inactive or unvisited targets. `orchbee.panel.layout` v1 retains cell geometry, nullable hidden rectangles, unzoomed neighbors, physical-edge results and the operation receipt. See the [geometry contract](windows-client-layout.md). Catalog v69 contains 76 documents; the native command inventory contains 88 paths.

`panel focus [source] --direction left|right|up|down` finds one neighbor in the selected TUI's unzoomed cell layout and preserves zoom. No neighbor returns `direction-unchanged` without activating the source Tab. A found destination is pinned and returns `direction-focused` only after a successful draw. Receipt reads never repeat the movement.

`panel resize [panel] --direction left|right|up|down [--amount <number>]` adjusts one eligible split border, preserving focus and zoom. `layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` sets an exact binary split path. Both route to one Windows TUI and retain a `layout-adjusted` receipt. Selected changes wait for a draw; inactive changes store without navigation. Finite ratios clamp to 0.1–0.9, and finite resize amounts normalize to an absolute magnitude at most 0.5 (default 0.05). Ratios belong to that TUI session and are not restored after restart. See the [layout contract](windows-client-layout.md).

## Shared Panel titles

`panel rename <panel-id> --label <text>` sets the shared primary Presentation override; `--clear` (or blank text) restores its creation label. It accepts an exact ID or `@panel:<suffix>`, with no focus or terminal effect. The [Panel label contract](native-panel-labels.md) owns normalization, revision/replay rules and coordinated cohort compatibility. `orchbee.panel.rename` v1 distinguishes `renamed` from `unchanged`; its nullable `customLabel` echoes normalized request metadata. `panel get` and `panel current` v3 expose immutable `label`, nullable `customLabel` and effective `displayLabel`. `api snapshot` v4 preserves the canonical optional override. Catalog v69 contains 78 schemas and the native command inventory contains 88 paths.

Panel swap uses the shared structural placement authority for one atomic same-Tab whole-leaf exchange, preserving Split/slot and terminal identities. The Windows CLI separately acknowledges source focus through the original Client; known canonical commits survive later failure or unknown focus. [Native Panel swap](native-panel-swap.md) owns the workflow, receipt and evidence.

Windows `tab create` and `panel split` resolve `--cwd follow|home|current|workspace|<absolute-path>` before creation; follow is the default and samples the resolved source root process. Their v2 receipt records the chosen directory source/path. They accept optional targets and labels, with no focus by default; `--focus` requests post-creation focus, and split accepts a Client-local `--ratio`. Explicit targets can create without a TUI. `orchbee.creation.result` v2 separates committed creation from later ratio/focus results; a later failure is not permission to repeat creation. [Native terminal creation](native-terminal-creation.md) owns target selection, total budget, concurrency guards and recovery for these Windows commands.

Windows `panel input (<panel>|--panel <panel>|--current) --right-click orchbee|panel` sets the shared right-button policy without moving focus. `--current` requires validated caller metadata; explicit targets bypass it. `panel split --right-click` commits the new Panel policy with creation. The default `orchbee` reserves right-click to the Client; `panel` permits unmodified right clicks in mouse-reporting terminal content. An admitted gesture retains its matching release after policy changes. See the [Panel input contract](native-panel-input.md) for routing, pending-input rechecks and failure recovery. This does not add a context menu. `orchbee.panel.input` v1 reports `configured` or `unchanged`; `panel list` v2 and `panel get/current` v3 expose `rightClick`. `tab get` v2 and `api snapshot` v4 admit the optional canonical field. Catalog v69 has 78 schemas and the native inventory has 94 command paths.

## Tab insertion boundary

On Windows, `tab move <id|@tab:suffix|label> (--direction previous|next|first|last | --insert-index N)`
requires exactly one placement flag. `N` is a zero-based boundary in the original context order,
not a final ordinal: 0 means first, the Tab count means last, and the source boundary or immediately
following boundary leaves order unchanged. Parse accepts integers 0..256; the Server rejects values
above the current count without clamping. Exact target resolution uses one snapshot and one command
under the existing total mutation budget, with no Client selection or implicit focus.

Indexed movement emits `orchbee.tab.move` v1 with `action: tab.move`, requested `insertIndex`, exact
context/Tab, request/Site/base revision and committed or unchanged revision. Direction movement
keeps `orchbee.mutation.result` v4. Catalog v69 contains 78 schemas and 94 command paths. A definite
bounds rejection retains the Server reason. An ambiguous submission retires the carrier, reports
outcome unknown and never replays or compensates. Old direction behavior remains available on its
existing platforms; this new CLI form is Windows-only. There is no new disk persistence or migration.

Index-command runtime failures use `orchbee.cli.failure.tab-move` v1, including the closed
`tab-move-index-out-of-bounds` reason. Parser failures and legacy direction failures retain their
existing generic envelope. The index carrier admits a context-close `terminal-busy` as definite
precommit, without widening the direction carrier contract.

PowerShell completion reads plain ASCII decimal integer AST constants as literal source text, so
`--insert-index 2` keeps completion active. Variables, splats, expressions and scriptblocks are
not evaluated or admitted by this repair.

## Source-selected terminal output

Windows `terminal read` and `agent read` accept `--source visible|recent|recent-unwrapped|detection`.
Recent combines retained history with the live screen; detection reads the recent screen-height
range. `--plain` exports text directly for pipes, defaults to recent, and excludes JSON. Source-mode
`--lines` allows zero, clamps to 1000, and defaults to 80 for recent. It excludes the legacy
history/rows/unwrap flags, whose behavior stays unchanged. The [output contract](native-terminal-output.md)
owns selection, exact sample validation and `orchbee.terminal.output` v1; catalog v69 has 78 schemas.

Windows `terminal wait-output` accepts the same `--source` and `--lines` selection, or `--forever`
for an explicit wait without a total ceiling. Any of these chooses source mode (default recent),
excludes `--rows`, and emits `orchbee.terminal.wait-output` v3. Without `--forever`, the default
remains 30000 ms; `--forever` conflicts with `--timeout`, retains five-second I/O bounds and supports
Ctrl+C. Recent waits join soft wraps; visible/detection match physical lines. Old invocations retain
v1/v2 output. A match can come from existing output and never proves command completion.

Windows `terminal read` and `agent read --ansi` export styled snapshots, defaulting to recent output.
Use `--format json` for `orchbee.terminal.output` v2 with `result.encoding: "ansi"`; text/plain reads
retain v1. ANSI excludes `--plain` and legacy history/rows/unwrap flags. It preserves styled blanks
and Unicode, uses only generated SGR, and joins verified soft wraps under `recent-unwrapped`.
Detection remains plain text. It does not reproduce original PTY bytes, hyperlinks or graphics.
The render request and prepared output each keep their existing 8 MiB bounds; oversized render
resources fail without a fallback or retry. See the [output contract](native-terminal-output.md).

Windows `status server` reads actual Server build/protocol/cohort metadata before Client Session admission; `status client` checks this executable offline. Both accept `--format text|json` and never activate a Site. Bare `status` remains a Workbench query. The [runtime status contract](native-runtime-status.md) owns the five-second budget, unknown metadata, compatibility, schemas and failures. Update-channel management remains unavailable.

## Agent navigation

Windows `agent focus <target> [--client <id>]` resolves a name, session or Panel to its unique live primary Panel before one Client control submission. Explicit `--session`, `--name` and `--panel` selectors use the existing resolver; `--request-id`, `--detach` and the 30000 ms total default timeout retain the common navigation recovery contract. An unready Client fails with `client-not-ready` before Agent reads. The [Agent navigation contract](native-agent-navigation.md) defines primary binding, ambiguity, cancellation and retained operation status. It does not acknowledge seen/unseen state.

## Windows Agent change pages

`agent changes [--cursor <c>] [--limit <n>] [--format text|json]` reads one bounded page under the existing five-second total Agent read deadline. Omit cursor for a current-head checkpoint; use the returned opaque cursor for later reads. Limit is 1–100, default 50. No filtering, subscription, long poll or automatic replay occurs. It admits one Client Session, closes it before interruption arbitration and emits one result. Ctrl-C preserves the shared cancellation/finalization contract.

The `orchbee.agent.changes` version-1 envelope contains the canonical changes/current/reset-required outcome and the correlated Agent source. Reset-required is a visible loss-of-continuity result; rebuild the baseline. Invalid/foreign-boot cursor, read denial and runtime unavailability use the existing Agent failure family with operation changes. A page cannot exceed the requested limit. JSON retains typed facts and cursors; localized text shows status, revision and continuation. The Server owns retention, authentication and coherent history; CLI grammar/help/completion/schema/Skill expose this boundary. Other platform runtimes remain unavailable for this new command.

Windows `agent get <target> --client <id>` adds a selected-Client projection in `orchbee.agent.get` v4; ordinary Windows Get is v3 with runtime information. The [Client attention contract](native-client-attention.md) owns version joining, focus and idle/done mapping. It uses one five-second total and never acknowledges or navigates. Missing/stale observations expose null seen; canonical status remains separate.

Windows `agent prompt --wait` checkpoints the retained Agent change journal after runtime preflight and before submitting input once. A failed checkpoint prevents submission. The checkpoint must cover the preflight revision. Each later fully validated page preserves transient activity; a fresh newer provider version must show working/blocked or a turn-finished transition to open the activity gate. Only working/blocked conditions can match that gate event; settled conditions must match a current sample, so a transient completion followed by working keeps waiting. Metadata and freshness changes alone do not establish activity. The five-second activity gate, original total/read deadlines, pinned identity and cancellation owner remain. A current canonical sample must cover the journal head. Windows v3 separates nullable `matchedChange` and its source from the current observation; Linux remains v2. History loss or a foreign cursor fails explicitly and preserves the accepted input receipt. This proves observed activity after the checkpoint, not causality or exclusive ownership of the submitted turn.

Windows `agent prompt <target> <text> --wait --client <id>` preflights one exact Client before sending and reads its attention on each later sample. The canonical activity gate still applies. Idle/done match only its current corroborated resting projection; missing/stale versions keep waiting. A lost Client or read failure stops the wait with the accepted input receipt retained. The Client read bundle is bounded to two seconds inside the existing five-second evaluation and original total deadline. No read acknowledges, focuses or resubmits. Selected-Client success is v4 with required `client`; ordinary Windows prompt remains v3 and Linux v2. `--client` requires `--wait` and has no Mac/Linux implementation.

Windows `agent get` v5 adds `runtime`; `--client` uses v6 with the existing Client projection. One five-second total covers target resolution, canonical Get, a read-only process sample for the unique live primary Terminal, a second canonical Get, and optional Client proof. The final Agent provider and selected Presentation generation must still match; a changed binding discards sampled runtime. Ended, unbound or ambiguous primary bindings report explicit unavailable runtime. An observed runtime retains the actual process-query source and Windows Job DTO (PID, creation time, image, command line, argv and nullable cwd). `promptReady` means the current fresh known nonblocked canonical Agent agrees with a recognized provider process chain. It is a submission preflight, not interactive UI readiness, OS foreground or an atomic input lease. Process failures preserve their existing envelopes and all Viewer attachments close. Linux get remains v1. The embedded schema retains earlier versions while the matched Windows cohort emits v5/v6.

Windows `agent list --client <id>` joins the selected TUI status in the v2 list envelope. It batches one bounded page, rereads its original cursor and checks retained changes before exposing seen/unseen. Empty pages still validate the exact Client. No-flag list retains v1. The [Client attention contract](native-client-attention.md) owns consistency and deadlines.

Windows Agent Get now emits v5 (v6 with `--client`), adding a separate `management` observation with source, revision and Server-owned startup state. Active records come only from completed startup; manually named sessions remain unmanaged. Pending startup makes promptReady false and `agent prompt` refuses before input. The bounded Status read shares the original command deadline and can fail with Agent read failure v3 (`operation: management`); it never retries or completes a launch. `agent start` v2 explicitly reports interactiveReady after its existing successful completion. The [startup contract](windows-agent-start.md) owns lifecycle and limits.

The native provider registry now has 20 entries, including Windows Cursor, Qoder and Antigravity JSON hook onboarding and offline diagnosis. The API catalogue is version 62; schema and command-path counts remain unchanged.

Mastracode adds Windows flat hook onboarding, partial-install repair, offline diagnosis and keyed lifecycle observations.

Droid adds Windows JSON hook onboarding and diagnosis at the currently selected user configuration source.

Kimi adds Windows TOML session hook onboarding and diagnosis; it does not yet claim complete working/blocked/idle observation.

Letta adds Windows session onboarding and offline diagnosis. Its hook output stays silent because the provider consumes stdout as context.

Devin adds Windows onboarding and offline diagnosis for six identity hooks. All events preserve turn state; missing or invalid APPDATA refuses setup.

Hermes adds Windows plugin activation and content-free session observation. Its offline doctor reports that provider command inspection is required.

## Windows persistent creation defaults

[Windows CLI configuration](windows-cli-configuration.md) owns the bounded TOML reader, `ORCHBEE_CONFIG_FILE`, `[terminal] new_cwd`, `[worktrees] directory`, and precedence before any creation effect. Windows `config check` v2 preserves five runtime checks and adds `user-config`; v1 remains five checks. `orchbee.cli.failure.configuration` v1 reports bounded preparation errors. Catalog v69 contains 78 schemas and 94 command paths.

## Windows terminal observation stream

`terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]` emits NDJSON from one existing binding without control or Host resizing. The [stream contract](windows-terminal-streams.md) owns full safe ANSI frames, resets, flush-before-ACK, deadlines and cleanup. Default viewport: 120×40; default duration: unlimited. Both output formats emit `orchbee.terminal.stream` v2 records. Catalog v69 has 78 schemas and 94 native command paths. Machine control now accepts NDJSON input through `terminal session control`; interactive terminal/Agent attach is implemented with remaining fidelity and acceptance obligations.

### Panel relocation transfer receipts

Panel relocation advances the shared mutation envelope to v5; other shared actions retain v4.
Relocation includes optional `transfer` facts:
`targetTerminalContextId`, `closedSourceTab`, optional `closedSourceWorkspaceId` and
`createdWorkspaceId`. Existing-Tab moves also preserve the returned Split and slot identities.
Moving a Tab's final leaf removes that Tab without disposing its terminal; the result reports
that removal. The local carrier checks destination kind/context, created-Workspace presence,
source-removal consistency and result correlation before acknowledging a commit. Catalog v71
contains 80 embedded schemas at this contract revision.

### Windows Panel relocation workflow

```text
panel move <panel> --into-tab <tab> --direction above|below|left|right [--target-panel <panel>] [--ratio <number>]
panel move <panel> --new-tab [--workspace <workspace> | --standalone] [--label <text>]
panel move <panel> --new-workspace [--project <project>] [--label <text>] [--tab-label <text>]
```

All forms accept `--focus|--no-focus`, `--client` and `--timeout`. Focus defaults to true,
matching pinned Herdr CLI `e0507237`; its JSON method separately defaults to false. The legacy
same-Tab `--target-panel --direction` form remains a one-effect mutation. Ratios must be finite
and are clamped to 0.1–0.9. New Tab labels are optional.

Resolve one snapshot and an explicit, caller-associated or sole current Client. Ambiguity fails
before effects. Without a Client, only `--no-focus` without an explicit ratio may proceed: choose
the first target leaf and native default ratio, without claiming Client geometry or zoom state.
With a Client, read-only source/target layouts determine zoom and the default target leaf.
Zoomed source/target Tabs and same-Tab relocation return unchanged without mutation. An explicit
target Panel must belong to the target Tab.

New Workspace ownership follows the source Project unless overridden. A standalone source uses
the sole Project, or prepares the first Project when none exists; multiple Projects require a
selector. Its directory follows the observed terminal root cwd; an absent cwd falls back to the
invoking process cwd. Invalid observations fail. Validate paths and labels before effects.

Phases: preflight → optional first Project → canonical transfer with new-Workspace directory → optional ratio → focus → observed layouts. One absolute deadline and cancellation
cover the invocation. The Server commits topology atomically without restarting the terminal;
Directory creation shares the canonical commit after Worker validation; Project preparation and Client effects have separate receipts.
`orchbee.panel.move` v1 retains every known commit on later failure, cancellation or unknown
outcome. Never replay ambiguous commands or report a partial workflow as completed.

Ratio targets the returned Split/slot identities after snapshot refresh. Client follow-up waits
for canonical convergence and preserves an unaffected prior selection; unrelated navigation
refuses follow-up. Removal of the selected source permits ordinary selection repair. Other
Clients keep independent navigation. Receipt layouts describe only the selected Client.
An unchanged empty Ready view may enter the existing minimum-revision-fenced control without
waiting for an unrelated navigation heartbeat; its observed revision must not regress. Inactive
target layouts report that Client's remembered canonical Panel, or the first member when absent.
Grammar/decision tests, receipt validation, cancellable transport checks and real Windows multiple-Client
live-terminal/restart acceptance cover these boundaries. Generic events and existing child,
grouped and pinned structure restrictions remain separate obligations.

Catalog v72 contains 81 schemas and retains 96 command paths.

## Reusable layout export

Windows `layout export` emits a reusable JSON tree with ratios, manual labels, observed directories and frozen launch commands. The [layout contract](windows-layouts.md) owns targets, Client selection, bounds, cancellation, absence of terminal effects and consistency. `orchbee.layout.export` v1 and `orchbee.cli.failure.layout` v1 join the current catalog v74 (83 documents). Export does not establish the still-unimplemented layout apply workflow.
