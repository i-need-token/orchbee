# Native CLI Query

This document covers the native `orchbee` command line: queries, Workbench mutations, terminal observation and input. Queries observe an existing Session. Only `server start`, `server connect`, the TUI startup path, and `workspace create`, `project create`, `worktree create` and `worktree open` may activate a Session; those four creating commands start the selected Session first when it is not running, exactly as `server start` would. Agent state comes from the Server's canonical model, never terminal-text inference. CLI commands need no TTY; the interactive TUI has separate terminal admission requirements.

The grammar, result documents, failure kinds, limits and budgets are declared in `src/invocation/grammar/`, the embedded schema catalog (`orchbee api schema`) and the help text. This document keeps what those cannot say: boundaries, reasons, operating-system facts and cross-process rules.

## Canonical grammar boundary

`src/invocation/grammar/` is the single declarative Rust syntax authority. Its const/static records own paths, option storage bindings and scopes, closed values, operand limits, targets, conflicts, repeatability, execution availability and the schema dictionary. The hand-written parser in `src/invocation.rs`, help directories/options, and Bash/Zsh/Fish/PowerShell/Elvish adapters consume those records. POSIX shells receive a literal metadata projection; PowerShell and Elvish receive JSON. Shell adapters interpret syntax without probing runtime state or evaluating user input. There is no parser framework or parallel command/option table. Localized descriptions and examples remain authored product copy; parser-backed examples and real-shell witnesses cover behavior that structural derivation cannot prove.

Syntax and discovery are identical on Windows, Linux and macOS. Unsupported execution returns `stage: "platform"`, `reason: "unsupported-platform"`, exit `4`, in either locale. Parsed requests preserve omitted Agent wait conditions/timeouts and opaque list cursors; each existing backend retains its own defaults, cursor encoding and observation behavior. Unix Workspace creation still uses its existing Workbench create/launch adapter for supported requests. No retired flag, suffix selector, environment fallback or configuration import is restored.

Where this document qualifies the Agent projection, per-Client Agent attention or Client control as Windows, macOS behaves the same; their Unix or other-platform statements describe Linux.

Terminal observation, control, attachment and Pane graphics share protocol, framing, acknowledgement and cleanup workflows. Windows adapters own the stdin helper and stdout relay processes, and a command starts its budgets only after its helpers report ready; macOS and Linux adapters read the inherited stdin and write the inherited stdout in place, each read or write gated by a zero-timeout poll, writes bounded by `PIPE_BUF`, and never switch the shared descriptors to nonblocking mode. Shared polling interprets their actual byte, EOF and exit observations. A host that does not admit a command refuses before terminal admission, signal registration or helper creation; macOS admits all of these commands, and Linux none. Pane graphics and the terminal streams observe interrupts through the CLI interrupt-pair port, which delegates to the existing Windows guard or the macOS signal producer.

## Sessions and background services

Only canonical CLI spellings are accepted. Session selection reads `ORCHBEE_SESSION` without a
legacy fallback. Machine fields use `sessionId`, and service documents use `orchbee.server.*`;
their versions remain `1`. This terminology boundary preserves Desktop's current runtime integration.

## Current CLI discovery and platform contracts

Unlisted surfaces stay accepted, but help directories, usage advice and completion never name them:
`server connect` (the byte relay Client shells run), `install`, and `status client` and
`status server` (which `machine add` runs on a remote computer that may have an older build; see
[runtime status](native-runtime-status.md)) are unlisted commands whose own `--help` still answers, and `machine add --no-install` and `machine remove --stop-session` are
unlisted options while they do nothing. The terminal geometry group `--cols`, `--rows`,
`--cell-width` and `--cell-height` is unlisted on the creation commands (`workspace create`,
`tab create`, `panel split`, `panel create-terminal`, `worktree create|open`, `plugin panel open`):
the Client sizes new terminals, and a test harness may still pin the grid. `--cols` and `--rows`
stay listed on `terminal session observe|control`.

Completion never queries runtime state or invents ordinal suggestions. In PowerShell, start an `@agent:<kind>` alias or a compact handle with a quote (for example, `'@agent:c` or `panel get '@panel:0x1'`); completion inserts the full target as a safely single-quoted handle because an unquoted `@agent` token is parsed as splatting syntax.

## Clear terminal output

Windows and macOS `terminal clear <terminal>` clears one exact bound primary or child terminal
in an existing Session. It removes preceding history and other output, clears selection
and returns the viewport to the active screen. It keeps only the row the cursor is on, so a
prompt that wraps keeps its last row; the bundled ConPTY keeps that row at the top of its own
buffer so the next typed command stays at the prompt. A Host on another platform keeps the
cursor's whole soft-wrapped line, and the command help states both (see the Host's
[prompt-preserving clear](../../terminal-host/docs/runtime-model.md#prompt-preserving-clear)).
An alternate screen is refused without changing content.
Terminal modes, title, working directory and child process remain intact. Clearing never injects terminal input.

The operation identity is a request correlation, not a retained idempotent receipt; no status lookup or automatic retry is advertised.

## Workspace worktree observations

Each request uses the exact Workspace and `trustRepository: false`. A successful
reply must agree with the original Session, Workbench revision, primary Directory path
and Worker, and include that Workspace at the source checkout. One admitted
connection, signal owner and original five-second total cover all rows, Agent/Client
joins and the final canonical/Client consistency checks. Under `--worktrees` the queries
run one row at a time, so the cost grows with the listed rows that have a Directory and a
long list can exhaust that shared total; narrow it with `--project`. Queries never activate a
Session, change associations, navigate, launch a terminal or write Git configuration.
Git cleanup remains owned by the existing Worker read task. External filesystem
changes are not fenced: these are sequential live observations, not an atomic Git
snapshot. `unavailable` never means “not a repository.”

## Named Session selection

Selection precedence is explicit `--session`, the managed terminal's `ORCHBEE_RUNTIME_SCOPE`,
`ORCHBEE_SESSION`, then `default`; Linux reads the same variables. `ORCHBEE_RUNTIME_BASE` still
selects the existing absolute base. The Worker supplies the admitted base and scope to child terminals, so nested CLI
commands keep their owning Session even if a different name was inherited by the Server.
An explicit `--session default` overrides that inherited scope. Invalid selected environment
values fail discovery; they never silently select the default Session.

## Physical Agent inspection

Agent names are dormant in this release: no command assigns one. `agent resolve`, `agent rename`, `agent start --name`, and the `--name` and `--agent-session` selectors are removed. The names protocol, the Server names service and the name display remain as dormant engine, and `agent list` rows keep their `name` field, null when no name exists. The failure document `orchbee.cli.failure.agent-names`, version 1, stays with that engine; its `resolve`, `rename` and `clear-name` operation values describe removed commands until the engine is narrowed.

## Agent prompt submission

Waiting from a non-working state first requires evidence of activity. A newer turn
outcome also qualifies when a short working interval was missed between polls. Old idle
or an unchanged completed outcome cannot satisfy this gate. Gate expiry is
`prompt-stalled`; an earlier caller deadline is `wait-deadline`. Subsequent read errors,
end, timeout or cancellation do not erase the confirmed submission. An already-working
Agent may complete its existing turn; a matching result does not identify the prompt's
own turn. Polling can miss transitions and does not replay events.

On Windows, canonical provider `codex` uses one ordered paste, Right, Enter sequence. Right is a non-character boundary that flushes Codex's paste-burst buffer and clears its Enter suppression window. It neither embeds escape bytes in prompt text nor guesses delivery speed with a fixed delay. Other providers retain paste, Enter; ordinary `terminal send --paste --enter` adds no boundary key. Receipts report the actual three or two records. Stopping before Right or Enter reports only the accepted prefix, never retries the suffix and never starts observation. This boundary does not establish an empty composer, foreground ownership or provider readiness.

## Inputs and trust boundary

- `argv`: bounded hostile input. Total UTF-8 bytes across arguments may not exceed 64 KiB
  (65,536). Any non-UTF-8 argument, or any overflow, is an `invalid-usage` failure with empty
  stdout even when another argument spells `--format=json`. No stdin or file input is read
  implicitly — the only exceptions are the explicitly declared `terminal send --file`/`--stdin`
  sources under [Send input sources](#send-input-sources), materialized once before any runtime
  effect.
- Environment: `ORCHBEE_RUNTIME_BASE` (optional; defaults to `/tmp`) selects the runtime base;
  `LC_ALL`/`LC_MESSAGES`/`LANG` select the diagnostic locale. `doctor` additionally reads the home folder and
  `PATH`. `panel current` in caller mode additionally reads
  `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID` as described below. Nothing else is consulted.
- Filesystem: the resolved runtime base, the Session scope directory (the Session name, `default`
  unless a managed terminal routes another), its Client and Worker sockets in the socket folder,
  the activation marker, and the
  current executable's sibling `cohort.json` plus four artifacts are observed but never written.
  The manifest also declares the `agent-observation-adapters` sidecar digest — startup never
  opens or traverses that directory. All directory traversal uses the existing no-follow,
  owner/mode-checked rendezvous owner
  (`ModeCheckedLocalSessionRendezvous::validate_mode_checked`, `ExistingOnly` policy only).
- Socket peer: the Client socket may belong to a real Server, a zombie socket, or a scripted peer.
  The CLI trusts the typed transport and protocol layers (OBLC admission, OBWP correlation and
  contract validation) and never re-derives verdicts from `Display` text, `ErrorKind` guesses, or
  byte counts.
- Snapshot data: user-controlled labels and directory paths are arbitrary Unicode and are treated
  as hostile text for terminal output (escaping below); JSON output preserves them losslessly.

## Public grammar

Help wraps at 78 display cells and never queries a terminal, inspects `COLUMNS`, or reads configuration or runtime state, so its output is the same in terminals and in redirected output. Hosts narrower than 78 columns may wrap the text further themselves.

PowerShell completion is a static grammar projection: it never executes user expressions or queries a Session; loading it is explicit and does not modify a shell profile. PowerShell 7 invokes it for a bare `-` and offers the root `-v`/`-V` aliases; Windows PowerShell 5.1 does not call native completers for that unfinished token. It reads plain ASCII decimal integer AST constants as literal source text, so `--insert-index 2` keeps completion active; variables, splats, expressions and scriptblocks are not evaluated.

Load Elvish completion explicitly in interactive Elvish 0.21 with `eval (orchbee completion elvish | slurp)`. Its callback evaluates no supplied word and starts no process. The Elvish interpreter test is ignored by the portable gate and must be run with `ORCHBEE_TEST_ELVISH` pointing to an interpreter; absence fails that requested test.

## Query state machine and budgets

Every network query uses one checked 5-second monotonic deadline, created immediately before admission, reused unchanged for the read and never renewed; it never falls back to the 50-second request default. A query performs one admission and one request, with no reconnect, retry, publication catch-up or second read. All owned `LocalClient` and socket handles are dropped before any mapping, formatting or output, so a slow stdout never retains a Session. No shutdown exchange exists or is claimed; the Server may observe EOF asynchronously.

Capsule manifests use contract 1 with exactly `contractVersion` and `payloadSha256`.
The integration result's `capsule.payloadSha256` replaces payload-version fields;
its root is `internal/integrations/agent-observation/<version>` in the environment's own data
root, where an entry's root names the environment owning it, as the
[writer](../../../crates/agent-integration-writer/README.md#capsule-location-and-ownership)
describes. Guarded and Cline copies compare their owner record, and hook-configuration scans
leave other environments' entries out. Marketplace inspection uses `absent|current|update-required`;
old-root aliases are not admitted. Only the strict current source is ready for mutation. Windows
managed installation is owned by the [Windows integration contract](windows-agent-integrations.md);
Windows Agent startup is specified in [Windows Agent startup](windows-agent-start.md); prompt
composition is specified above.

For `terminal send`, `accepted` means the Host queued the input record, never that a child process read or executed it. The request is never retried, including after partial acceptance, because the input may already have taken effect. The send rides the shared Worker-Host link: when that connection fails, other viewers on the same Host may be interrupted, which is a transport-level side effect of the underlying connection, not a send seizing control.

### Send input sources

Windows pipe acquisition owns one hidden, noninteractive Windows PowerShell reader
resolved from the OS system directory. Only the inherited stdin handle is passed;
source bytes never become script or argument text. The fixed script reports ready on
its standard error, then copies at most the source limit plus one byte into a private
output pipe. The reader's start, from spawn to that report, moves the command's
deadline later by its own length, for the read and every later step; it has its own
20-second allowance, after which acquisition is `timeout`, and the interrupt guard
ends it. The CLI then polls under the moved deadline and interrupt guard, and
kills/reaps the child on success and handled failures.
It does not change the caller's pipe mode or leave a blocking reader thread behind.
The shared send workflow keeps source arbitration, one submission and receipt projection together.
Linux and Windows retain their existing send routes. macOS takes the Linux route, declared `--file`
and `--stdin` sources included; shell-run observation remains Windows-admitted, and unavailable
commands refuse before source, signal or Session effects.
Regular-file stdin retains direct reads. Process startup and termination remain OS
scheduling boundaries, not hard real-time guarantees.

- **Unix file admission.** `--file` opens `O_RDONLY|O_NONBLOCK|O_CLOEXEC` — the open itself
  cannot block on a FIFO or device — then `fstat` on the already-open descriptor must
  report a regular file, which also closes the path-replacement race: a swapped path
  still reports its true type. An unopenable/unreadable path, a non-regular result
  (directory, FIFO, device, or a swapped replacement), an empty file, or a read error is
  a source-stage `invalid-usage`.
- **Unix stdin admission.** `--stdin` refuses a terminal descriptor, then dispatches on the
  inherited descriptor's exact `S_IFMT` type: a regular file is read in place,
  unbuffered, on the borrowed descriptor — the shared file position advances by exactly
  the bytes returned, at most the declared source bound plus one probe byte (that is the inherited-position
  contract); on Linux a FIFO/pipe is reopened through `/proc/self/fd/0` into a private
  nonblocking file description, so the caller's descriptor flags are never mutated and
  nothing needs restoring, and the reopened description must verify as a FIFO. macOS has
  no private reopen — `/dev/fd/0` shares the caller's description — so a FIFO is read in
  place on the borrowed descriptor, flags untouched, each read gated on its poll. Every
  other kind (socket, device, …) is rejected.
- **Source signal guard.** On macOS and Linux the source stage owns process signal capture for
  its duration — single-threaded, with no other signal owner and no child spawned before
  handoff. The eligible set is {SIGHUP, SIGINT, SIGQUIT, SIGTERM} minus every signal the
  process already blocks or ignores at source entry; the blocked set comes from the
  current thread mask. On Linux the ignored set comes from a
  strictly bounded `/proc/self/status` `SigIgn` field (at most a 64 KiB read yielding
  exactly one well-formed hexadecimal field — missing, malformed, duplicated, or
  oversized is an internal failure before any source I/O). Setup creates a `NONBLOCK|CLOEXEC` signalfd for the eligible set
  first — while inherited dispositions still apply, so a setup fault there needs nothing
  restored — then swaps the thread mask atomically and captures the old mask; the capture
  interval begins at the swap, before any source read. macOS has neither signalfd nor a
  safe read of dispositions, so setup creates a kqueue, swaps in a mask that blocks every
  candidate, and classifies each candidate by the kernel's own rule: a blocked signal
  raised on this thread stays pending unless its disposition is ignore. Each candidate is
  raised and dequeued again; one that never became pending is ignored, leaves the kqueue's
  `EVFILT_SIGNAL` filters and is unblocked again. A signal sent to the process during this
  classification is recorded by the kqueue or found pending, and is raised again once, so
  arming never swallows it. The nonblocking dequeue blocks `SIGCONT`, raises it as a
  sentinel and calls `sigwait` over the probed set plus `SIGCONT`: XNU never discards
  `SIGCONT`, and a pending probed member has a lower number, so it returns first. A setup
  fault before the first raised probe restores the old mask; a later one keeps the
  candidates blocked for the rest of the invocation, so no probe is ever delivered.
  Dispositions are never touched and
  inherited blocked/ignored state is preserved exactly — an empty eligible set is a real
  armed policy (nothing to capture), never a no-op fallback, and inherited pending
  signals stay pending under their original mask.
- **Arbitration and finalization.** The read loop polls the signal descriptor (the signalfd, or
  the macOS kqueue) beside the
  source descriptor and drains it before processing ready data, before judging a
  timeout, and once more at the completion boundary; the first consumed eligible signal
  selects source cancellation, which wins over deadline and source I/O error.
  Materialization finish linearizes on a final nonblocking signal read: a consumed
  signal means cancel; no pending signal means the source no longer owns cancellation — the
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
- **Stated OS limits.** The deadline is a logical bound around each syscall, not inside
  it: a kernel-stalled regular-file read is not preemptible, so no prompt-termination
  promise exists there. The pre-arm window before the mask swap runs under inherited
  dispositions — a default death there is honest and effect-free, not a guarded-window
  hole, and neither arming order closes every signal-arrival race. Inherited `SIG_IGN`
  dispositions are respected, not overridden. On Linux these claims are backed by
  procfs, consistent with the procfd input admission. On macOS they rest on the kernel
  discarding a blocked ignored signal, and a FIFO read can stall only when another reader
  of the same pipe drains it between the poll and the read.

On macOS and Linux the Agent read commands own a separate, narrower signal watch for the whole
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

The Linux wait samples the current projected state at each completed poll; it is not a subscription and provides no change notification. A transition that begins and ends between two completed observations is missed: pending requests may clear, turn outcomes are replaced by later turns, and freshness moves between polls. The miss window is not bounded by the poll interval alone, because Server read duration and scheduling also widen it, and no sticky-state guarantee exists even for conditions that stay true once reached.

## Session activation

`server start` is the headless form of the TUI's local startup: both call the single shared connect-or-activate core in the workbench-client crate, so the sequence is identical and only the failure projection differs. One absolute 20-second deadline starts before cohort validation and is never reset per stage.

On Linux, a `timeout` answer means readiness stayed unconfirmed: before marker commit nothing was left
behind, between commit and spawn a marker without a live owner may persist (the next
invocation then answers `recovery-required`), and after spawn a Server may still be running.
The command never claims a timeout proves no Server started, never retries `recovery-required`,
and never force-cleans uncertain runtime state. Windows may have published a generation before marker
commit; a later invocation rechecks exact process identities, registration and all recovery guards.
Only a proved-dead generation is automatically reconciled, preserving its checkpoint. Signals stay inherited: no capture stage exists,
so an interruption is process death under the conventional `128 + signo`, never a `cancelled`
envelope. There is no repair verb.

## Runtime folder facts

The runtime folder report is the read-only counterpart of activation: it holds bounded facts about the
selected local Session runtime without connecting to anything, creating anything, or
repairing anything. The runtime base and Session resolve exactly like `server start`
(`ORCHBEE_RUNTIME_BASE` or `<data root>/internal/runtime`, Session `default` unless routed), then
the collector inspects a finite set of known
paths through the rendezvous' own validated directory descriptor — `AT_SYMLINK_NOFOLLOW`
descriptor-relative `fstatat` plus, for the Session lifetime lock only, an open-existing-file
probe whose `try_lock` result is immediately released on the same descriptor. The activation
lock is metadata only (presence/kind/owner/mode/link count) and is never acquired. No endpoint is connected — the Client socket least of all, where a connect would consume
an admission slot — and no process is enumerated, signaled, or hashed.

A `held` lifetime lock means only that some owner held the lock at that observation instant, never that a Session, a Worker, or any process is alive. `absent` is a pathname fact, never proof that an execution ended, and `complete` is a coverage fact, never a runtime-health claim.

Unsafe and uninspectable rows are facts, not failures of the report: the runtime check fails only
through the `config check` path rows. A temporary diagnostic lifetime-lock probe can contend with a
concurrent activation's own lock attempt — a bounded transient effect the caller should expect. The
collector never acquires the activation lock, never creates or deletes a path, never repairs, and
never manufactures liveness or termination witnesses.

## Canonical data and results

- A creation receipt whose terminal started or is starting ends with a `terminal list --panel @panel:0x<hex>` hint for the new Panel, because the spawn outcome does not carry the terminal ID.
- `orchbee.server.start` lists `negotiatedCapabilities`: the capability names both sides listed in this invocation's `client-server` handshake. The window's Update reads it to prove that the active window version joins this Session; it is optional in the schema because Clients before the window self-update release omit it.
- `orchbee.server.start` also lists `sessionVersion`: the product version the Session's Server named in that handshake, or null without one. Desktop reads it to tell that a Session runs an older version than its own window and moves the Session to its version, as [Desktop's Session update](../../desktop/README.md#session-update) describes; it is optional in the schema because Clients before it omit it.

## Mutations

`terminal send` mutates Host input state, not the Workbench document: it rides the admitted
session's OBCS `terminal.send` exchange described above and does not touch `expectedRevision`,
the Workbench command channel, or the TUI's controller/viewer/resize ownership.

The `expectedRevision` choice is the protocol's optimistic-concurrency contract: the Server
compares the command's base revision to its current revision before any effect and rejects a
mismatch without applying anything. A `revision-mismatch` therefore proves that nothing
happened, and the CLI follows the moving base a bounded number of times: it reads a fresh
snapshot, resolves every target again with the same semantics, and submits again only when the
resulting action — every target identity and argument — is identical to the first submission.
When any target now resolves differently or not at all, the fresh read fails, the deadline
passes, or three retries are spent, the CLI reports the last refusal unchanged (exit 3,
`reason:"revision-mismatch"`). It never acts on a different target and never submits without a
base. The Windows creation workflows of `tab create`, `panel split` and `workspace create` take
more from the snapshot than their targets, so each retry prepares all of it again: the parent
target, the label (a `workspace create` label with its first-free ` (2)` suffix), the directory and its source, the plugin
context and the selected Client. They submit again only when every derived value and target is
identical. Phased workflows that submit an action resolved in an earlier phase — `panel move`
with `--into-tab`, `--new-tab` or `--new-workspace`, `layout apply`, and plugin panel management —
do not retry; their refusal reports as before.

Safety contract:

- **At most one effect.** Every submission before the last ended in a definite no-effect
  refusal, so at most one submission per invocation can apply. All argument and target
  validation completes before each command is built, so a parse defect or unresolved target
  submits nothing.
- **Fresh identity.** Each submission generates one new globally unique `WorkbenchRequestId`.
  The Server ledger retains a refusal under its request ID, so reusing that ID with a new
  `expectedRevision` would be a request-identity conflict. The attempts remain one logical
  request; receipts and failures report the identity of the last submission for operator
  reconciliation.
- **Server-owned revision.** `expectedRevision` always comes from the same validated
  snapshot that resolved the targets of that submission — never guessed, incremented
  client-side, or reused.
- **No automatic replay.** Only a definite `revision-mismatch` refusal is submitted again. An
  ambiguous post-submission outcome — lost write, lost, malformed, or miscorrelated response,
  or an answer that fails validation — retires the session and surfaces as `outcome-unknown`
  (exit 8). The command may or may not have been applied; it is never resent. The localized diagnostic says so and directs the operator to
  inspect Workbench state (`status`, `tab list`, `panel list`, …) before deciding whether
  to repeat the command.
- **Close confirmation.** `tab close`, `panel close`, and `workspace close` are
  destructive; the parser requires the explicit `--confirm` flag so a noninteractive
  invocation can never close by default. `operation resume` owes the same flag:
  opening a fresh attempt window on a retained close is itself an effect.

Geometry is always explicit in the request: a non-TTY invocation cannot inherit terminal
dimensions, so `tab create` and `panel split` accept the four flags as an all-or-none
group whose omission requests the default 80x24 grid with the paired-zero (unknown)
pixel pair — a CLI request policy materialized into the wire geometry, never a measured
display fact. `--cols`/`--rows` are grid cells and
`--cell-width`/`--cell-height` are per-cell pixel extents; every dimension is a `u16`
bounded by the protocol `WorkbenchTerminalGeometry` validation before any runtime access.

Launch selections cap worst-case JSON string-content bytes at 30 KiB: ASCII controls count as six, quotes and backslashes as two, and other UTF-8 bytes as one. This preserves the previous 2 KiB input domain while bounding the larger resume commands. Matching cohorts are required for the enlarged limits.

Re-running `workspace close` is never a resume: a resumable retained operation is continued only through `operation resume <close-request-id>`. An unfinished close continues by itself on the Server, which retries up to about three 30-second attempts and only then reports one failure, so `operation resume` is for a script that wants to continue a close that finally stopped.

Deliberately not delivered: `project close`, `panel resize`, terminal open operations, Session lifecycle
restart, disable, recovery or repair commands (Windows stop/status are supported), and every remote operation.

## Failure matrix

On Windows, a Session started as administrator labels its endpoints High (see
[local IPC](../../../crates/local-ipc/README.md#windows-transport-refinement)), so a Client below
High integrity meets OS error 5 at the pipe open or the peer check. Whatever kind its row gives,
stderr then says that the Session runs as administrator and to run `orchbee server stop` in an
administrator terminal; `server stop` adds the same line to its `unavailable` answer, and the TUI
shows it as its startup failure. Kind, exit code, details and the JSON document stay those of the
row. Every other `denied` from the local runtime says only that the local Session refused this
connection. A Session started at standard integrity admits its own user's administrator Clients,
such as a shell over Win32-OpenSSH, which an administrator account receives elevated; the
elevated-Session rule above stays.

Stages and reasons are finite schema values; no exception text becomes a machine field. Read-path errors never become outcome-unknown or failed-effect: only the command stage can produce an undetermined effect, and `rejected`/`denied`/`unavailable` precommit answers are definite, because the Server applied nothing. Of these, only `revision-mismatch` earns the bounded fresh-read retry described under [Mutations](#mutations); a `rejected` failure with that reason means the retries were spent or the target no longer resolved to the same identity. The send stage follows the same rule for its single input record: a `RequestNotSubmitted` is definite non-submission, an `OutcomeUnknown` means the record may already have landed, and the typed `terminal.send.result` answers are the Server's own verdicts, never a client guess. A declared `--file`/`--stdin` source is stricter still: every one of its failures happens before any runtime access, so it can never produce `outcome-unknown`.

## Invariants

- A syntactically invalid invocation never touches the filesystem, cohort, or socket.
- Offline commands (`help`, `-v`/`-V`/`--version`, `--skill`, `completion`, `api schema`, and `config default` on Windows and macOS) never perform platform,
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
- On macOS and Linux, the source signal guard never alters dispositions and restores the inherited thread mask
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
- Failure envelopes never invent `source`: sessionId/revision appear only from a validated snapshot.
- Ambiguous never guesses: it reports bounded candidates plus the true count.
- `api schema`, help, and completion enumerate exactly the commands delivered in the same build.
- `server start` is the activation verb, and the commands listed above that declare activation
  share its start and failure vocabulary. A start may create the runtime directory, hold
  activation/lifetime locks, commit the marker, and spawn the Server — only a grammar failure guarantees zero effect. Its absolute 20-second deadline
  is never reset per stage; a returned `recovery-required` refusal is never retried or force-cleaned;
  Windows cold recovery rechecks the separately documented proof guards before a fresh generation.
  A timeout never claims no Server started. `doctor` is the observation
  surface and performs no effect at all beyond its bounded lifetime-lock probe and the Session
  check's handshake `query`: no connect,
  no create, no lock acquisition, no repair, and no liveness or restart claim.
- `workspace create`, `project create`, `worktree create` and `worktree open` run that start as
  their first phase, after local parsing and before their own deadline begins. When the invocation
  started the Session, standard error carries `server start`'s started line and restore notice;
  nothing is printed when the Session was already running, and stdout and JSON documents are
  unchanged. A start failure is `server start`'s failure in the generic `orchbee.cli.failure`
  document with the local-runtime source, and the command's effect is not submitted. A mistyped
  `--session` name creates that Session, as `server start` does. A command a machine runs for a
  [`--machine` route](machines.md#command-routing-contract) never starts a Session. Every other
  command keeps the unavailable failure when no Session runs.
- The CLI-owned 5-second budgets are checked deadlines; a construction failure is
  `internal-failure`, while transport-internal `RequestNotSubmitted` stays `transport-loss`.

## Calling Panel context

The Windows Worker writes `ORCHBEE_TERMINAL_ID` and `ORCHBEE_WORKER_ID` from admitted
launch authorization into child environments, overriding inherited conflicts. These product
identities are lookup hints, not credentials; both must be complete nonzero lowercase 32-hex
IDs. Malformed, empty, non-Unicode, or partially missing values fail `invalid-usage` (exit 2),
with localized guidance and no value echo. Explicit Panel or Client mode bypasses this metadata.

Windows Worker identities are fresh per Server connection using the existing OS random source;
one Worker shares that ID across its terminals. Terminal IDs can repeat across Sessions, so exact
Worker matching rejects callers from a previous or different Session. No reconnect or adoption is
introduced. Tab close acceptance is not removal: lookup reflects membership until the Server
publishes removal, after which the old binding is not found.

## Panel process information

### Windows interpretation and Agent input preflight

The Host alone owns process collection. The CLI interprets the validated Host sample statelessly and selects no foreground from it: every process read is followed by one Workbench Panel Agent `identify` read on the same connection and budget, and `foreground` projects the Server's identity result and run anchor. Windows `CommandLineToArgvW` interprets nonempty command lines of at most 4096 UTF-16 units without NUL; these are Windows argument parsing rules, not proof of another executable's parser. Empty or unavailable commands remain null: there is no substitution of the CLI's own command line, shell expansion, file lookup, or environment access.

The executable image must identify an Agent directly, or identify a known runtime
whose actual script entrypoint identifies one. Eval/command payloads and arbitrary
later arguments never identify an Agent. Known package entrypoints and executable
aliases are explicit. Provider matching is a diagnostic heuristic, not executable
authentication. It does not create canonical Agent state or establish readiness.

A published run only shows. The Server keeps publishing it until one of its own later samples
ends it, and keeps it under `unknown` results, so input requires current evidence in this order:

1. The sample read before the identity must still hold the anchor's process as that provider;
   otherwise `agent-not-running`, for example just after the Agent exits.
2. The Server's result must be `agent` for the anchor's provider. `unknown` with
   `ambiguous-candidates` refuses as `agent-runtime-ambiguous`; any other `unknown` refuses as
   `agent-identity-unknown`.
3. The shared sample verdict over that sample must name the same anchor as an interactive Agent.
   Another outermost candidate or several candidates refuse as `agent-runtime-ambiguous`; any
   other unresolved verdict refuses as `agent-identity-unknown`.
4. The expected canonical provider must match the admitted one.

`agent get` reports `promptReady` only when this preflight admits its sample.

The anchor is the run Panel reports attribute to the Terminal, so `foreground`, Panel reports and
the TUI Agent display name the same run. An `agent` identity always has that anchor; a published
run can also continue under `unknown` (for example `unclassified-arguments` or an opaque row
outside the anchor), and the identity row explains why. A sample that names an Agent selects
nothing while the Server is still settling. The anchor can name a process the earlier sample no
longer holds; the process rows stay the sample's. Ancestor rules, cycle and reused-parent
handling, and the retained-Job boundary belong to the shared sample verdict the Server consumes.
Ambiguity stays explicit rather than looking like an available shell. No POSIX foreground
process group is claimed.

### Agent identity diagnostics

The discriminator of the `identity` object is `result`, never `kind`, so the value cannot be mistaken for the shared `unsupported` capability shape. A generation the Server has not sampled yet reports `unknown` with `settling`.

Physical Explain receives the identity inside the Server's `explained` result. `panel process-info`
sends one Workbench Panel Agent `identify` request after the process sample, on the same connection
and inside the same budget; `identified` also carries the run `anchor`. A different Host Terminal
fails with `workbench-changed`; the reply's Workbench revision is not a fence, because the identity
belongs to the exact sampled Terminal and each command keeps its own snapshot fences. Other Server
refusals and transport failures use the generic CLI failure contract. The fresh sample and the
identity remain separate observations, but `foreground` is derived from the identity read, so an
Agent chain always names the Server's published run. Text output adds one localized Identity row;
JSON carries the value verbatim.

## Windows Session stop

Cancellation or timeout stops waiting and never rolls back the admitted shutdown. Only the matching strict stopped activation record plus a free existing lifetime lock proves completion; disappearance, invalid records and a different ID never do. A later start consumes this receipt, so `not-recorded` cannot disprove an earlier stop. A Windows stop that failed and was not retried within ten seconds of its first failure ends its Server as an [`exit`](../../orchbee-server/README.md#managed-windows-session-shutdown): every terminal keeps running, a later retry finds that Server gone, and the next start adopts the terminals. On macOS a `SIGTERM` or `SIGINT` to the Server, which a logout or system shutdown sends, records a stop under the Server's own identity; in the CLI those signals keep their inherited action and end a waiting `server stop` at once, as `128 + signo` with no document, so pass `--request-id` or `--detach` to keep a handle for `operation status`. `--machine <selector> server stop` is not this request carried to another computer: the [remote Session stop](machines.md#remote-session-stop) owns it.

A Session whose Server refuses this build's offer is handled by kind; status reads never take this path. A Session from before the handshake gets `unavailable` `predates-handshake`: no shim speaks to it, so the text names the action, running `server stop` with the Orchbee that started it, as `pnpm desktop` does. A Server that refuses as too old, as another development build, or with `stopping` is asked for its terminal count with one `query` control request. With none open, one `exit` request ends that Server and its Worker, and the CLI waits for the Session lifetime lock to be released before reporting `exited` (exit 0); there is no stop record, and the next start recovers the Session from its last checkpoint, counting no crash when that Server recorded its `exit`. With terminals open the answer is `unavailable` `terminals-open`, or `draining` for a Server that refused with `stopping`, because `exit` never ends a terminal and, until Host `exit` arrives in P1, only the build that started a Session can end its terminals. A Server that answers `exit` `unsupported` gets `exit-unsupported` with the same action. An `exit` whose reply was lost after submission, or whose Session still holds its lifetime lock at the deadline, is `unknown` with reason `transport` or `session-lifetime`.

A start that a Server restart's hold answers `not-recorded` recorded nothing, so it reads `unavailable` `draining` (exit 4) with the text that says to try again shortly, as for a Server that refused the offer as draining; the Session takes a stop again once the restart ends.

## Offline schema export

The schema owner resolves the embedded ID and prepares the entire bounded document before any filesystem operation. Unknown IDs preserve existing not-found behavior and create nothing. The shared publication workflow resolves the destination, refuses an existing non-regular/reparse target through the Windows metadata port, creates one exclusively owned random sibling `.orchbee-schema-<nonce>.tmp`, writes and synchronizes the complete document, closes the handle, then publishes with one rename. Existing regular destinations may be replaced; parent directories are never created. There is no truncation fallback or automatic retry.

A process crash may leave a recognizable unpublished sibling temporary; publication does not claim power-loss durability for the parent directory.

## Windows unified Agent projection

Windows cursors use `u1.<base64url>` and are at most 1024 characters. The payload is canonical JSON encoded as base64url. Cursors bind to Session, source revisions, `includeEnded`/Client filters, page offset and fingerprint; they require no retained files. Malformed cursors are rejected as invalid input before live source reads. The CLI must read current sources to determine whether a `u1` cursor is stale, so it can reject a stale `u1` cursor only after those reads; the caller must restart at the first page.

Windows Get, Read, Prompt, Send Keys, Wait and Focus share one resolver. `@agent:<kind>` selects a provider only when the complete current projection proves it unique. `@agent:<n>` selects a one-based ordinal in the complete list order, never a page-relative position; the resolver accepts live rows only, and no selector reaches an ended session in this release. Bare names preserve Server alias precedence before provider fallback. Partial reads cannot establish provider uniqueness. Mutating effects retain the complete physical binding and revalidate it immediately before the write.

The native provider registry now has 20 entries, including Windows Cursor, Qoder and Antigravity JSON hook onboarding and offline diagnosis. The Agent projection documents are registered in API catalog version 1.

Mastracode adds Windows flat hook onboarding, partial-install repair, offline diagnosis and keyed lifecycle observations.

Droid adds Windows JSON hook onboarding and diagnosis at the currently selected user configuration source.

Kimi adds Windows TOML session hook onboarding and diagnosis; it does not yet claim complete working/blocked/idle observation.

Letta adds Windows session onboarding and offline diagnosis. Its hook output stays silent because the provider consumes stdout as context.

Devin adds Windows onboarding and offline diagnosis for six identity hooks. All events preserve turn state; missing or invalid APPDATA refuses setup.

Hermes adds Windows plugin activation and content-free session observation. Its offline inspection reports that provider command inspection is required.

## Programmatic semantic keys

Every semantic key uses one validated stroke record through both native protocols. Worker supplies physical identity, logical text, unshifted scalar and modifiers; shifted printable text consumes Shift for text generation while retaining it for negotiated full key reporting. Host encodes the current terminal mode, including CSI-u for Super character chords in the legacy terminal keyboard mode. Sequence order, at most 64 keys, a shared deadline, input-only attachment, prefix receipts and no replay after partial or unknown outcomes remain unchanged. Function keys may be mode-ignored where the child has not enabled a supporting keyboard protocol; receipts retain the actual encoded-byte count.

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
