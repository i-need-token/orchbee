# Terminal Input Automation

| Field              | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Proposal under review. It designs `terminal send-text`, `terminal send-keys`, and `terminal wait-output` — the commands the accepted information architecture marks "later explicitly authorized slice" — plus `terminal open`, a first-cohort command that is accepted but unimplemented. Nothing here is admitted or implemented until this slice is accepted; all option spellings below are proposed                                                |
| Owner              | Product command experience and the maintainers of the Rust `orchbee` Client, the Server semantic-command boundary, and the terminal projection data plane                                                                                                                                                                                                                                                                                               |
| Current authority  | [`native-command-surface.md`](native-command-surface.md) holds the accepted information architecture and automation/input/mutation contract; [`apps/orchbee/docs/native-cli-query.md`](../../apps/orchbee/docs/native-cli-query.md) owns the delivered read-only grammar, envelope, failure matrix, and budget conventions that this slice extends; [`schemas/client-server/README.md`](../../schemas/client-server/README.md) owns the OBCS wire model |
| Parent design      | [`native-command-surface.md`](native-command-surface.md) — this proposal owns only the four command contracts below; it changes no accepted decision                                                                                                                                                                                                                                                                                                    |
| Decision authority | The slice owner accepts: the four grammars, the viewer/controller wire mapping, the key vocabulary and its client-side encoding, the bounded-observation model, the output schemas, and the failure-matrix extensions below. Final option spellings, the complete `wait-output` predicate set, agent control-domain ownership, and exact bound values remain reserved                                                                                   |
| Exit condition     | Accepted command contracts for all four verbs, implemented under the delivered CLI conventions with scenario evidence, after which this proposal is consumed into the owning implementation record                                                                                                                                                                                                                                                      |

Simplified Chinese review translation:
[terminal-input-automation.zh-CN.md](terminal-input-automation.zh-CN.md).
English remains canonical.

## Position and scope

These four commands are the agent-drivable semantic face of Orchbee terminal automation: the
CLI-level counterpart of what a control-socket API gives an agent over a terminal multiplexer
(send input, wait for output, attach a live view). They are expressed on Orchbee's own models —
local same-user Server admission, Workbench-exposed terminal bindings, viewer attachments, the
single combined input-and-resize controller grant, and the bounded pull-based frame plane —
rather than on raw PTY bytes or a second control channel. An agent can observe a terminal with
`terminal open` or `terminal wait-output` and act on it with `terminal send-text` or
`terminal send-keys` under the same admission, controller arbitration, request identity, and
at-most-once rules as a human TUI Client.

- `terminal send-text` writes caller-supplied text to a terminal as typed input, or as one
  semantic paste. It requires the terminal's controller grant.
- `terminal send-keys` compiles a closed key-name vocabulary into bytes and writes them as typed
  input. It requires the controller grant.
- `terminal wait-output` performs bounded observation: it attaches a read-only viewer and waits
  until a predicate over the visible screen is satisfied or a mandatory deadline passes.
- `terminal open` attaches a read-only viewer and streams every delivered screen frame as
  bounded output until the stream ends.

Every command is a one-shot Client Session over the accepted Server-relayed path. None requires
a TTY, connects to Terminal Host directly, treats terminal output as Agent or Workbench
authority, creates or terminates terminals, resizes, or holds state across invocations.

Non-goals for this slice: implementing the commands; Session lifecycle operations; remote
targeting; Agent startup or control commands; a resize verb (the controller grant covers it but
no CLI verb is proposed); a semantic-key wire extension; scrollback or byte-stream predicates;
`--detach` support; close/kill verbs.

## Model mapping

All four commands reuse the delivered query state machine for cohort validation, runtime
discovery, socket validation, OBLC admission, and one bounded Workbench snapshot read. Terminal
targeting then continues on the same Client Session through the OBCS viewer/control contract:

| Stage   | Wire semantics                                                                                                                                                                                                                                                                                                   |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Target  | `<target>` accepts only a complete typed terminal ID (exactly 32 lowercase hexadecimal characters, nonzero), the same rule as `terminal get`; label matching is not offered. Resolution maps the snapshot binding to `{sessionId, workerId, terminalId}`; an unbound or absent terminal is `not-found`           |
| Attach  | `terminal.viewer` `attach-binding` `{sessionId, workerId, terminalId}` → `attached` `{attachmentId, hostEpoch}` — a new read-only viewer owned by this Session                                                                                                                                                   |
| Acquire | `terminal.control` `acquire` `{attachmentId}` → `acquired` `{controllerId}` — requests the terminal's single combined input-and-resize grant. Contention returns `rejected` `{reason:"controller-busy"}`; the Server holds no wait queue, so the CLI reports a denial rather than blocking                       |
| Input   | `terminal.control` `input` `{attachmentId, controllerId, sequence, bytes}` with 1–4,096 raw bytes → `input-accepted`, or one `paste` `OBCQ` record with 1–262,144 bytes → `pasted` `{acceptedBytes}`. Acceptance means admission to Host's bounded input queue, not that the application consumed or executed it |
| Observe | `terminal.viewer` `poll` `{attachmentId}` → `empty` or one binary `OBSR` record; the client applies the record, then acknowledges it with `frame-applied` `{sequence, generation, revision}`; a sequence or decode divergence is recovered with `need-snapshot`                                                  |
| Release | `terminal.viewer` `detach` `{attachmentId}` → `detached` — releases the controller grant before retiring the viewer. Session loss performs the same cleanup, so a crashed command never retains a grant or viewer                                                                                                |

Two properties of this mapping drive the command designs:

- Input authority is attachment- and revision-scoped on the Server side and grant-scoped on the
  wire: a command must attach a viewer before it can acquire the controller, and every input or
  paste action names both identities plus a strictly increasing positive `sequence` per grant.
- The frame plane is ordered and lossless per viewer: the Worker buffers at most one record per
  viewer and closes the attachment if a second arrives before the first is consumed, while
  `poll` returns `empty` immediately whenever no record or outstanding feedback is pending. A
  CLI observer therefore sees every delivered revision provided it acknowledges each frame, and
  waits by polling on a bounded client-side interval — there is no server-side long-poll.

## Identity, sequences, and at-most-once submission

- Every wire request carries a fresh `requestId` — 32 lowercase hexadecimal characters from
  fresh random bytes — generated and correlated exactly as the delivered commands do.
- A control `sequence` starts at `1` for each newly acquired grant and strictly increases per
  `input` or `paste` action. Input actions are submitted serially: each is submitted only after
  the previous one's result arrives, so acceptance of the final action certifies the whole
  submitted prefix.
- No request is ever resubmitted. The only admitted retry is the wire's bounded `server-busy`
  retry for a request known to have committed no effect — identical action values, a fresh
  `requestId`, inside the same logical deadline — which may apply to `attach-binding`,
  `acquire`, `poll`, `frame-applied`, `need-snapshot`, and `detach`, and never to `input`,
  `resize`, or `paste`. A definite rejection grants no retry.
- An ambiguous result — wire `outcome-unknown`, or `SubmissionError::OutcomeUnknown` after a
  write, read, deadline, correlation, or result-shape failure — retires the Session. The
  command stops, reports `outcome-unknown` carrying the affected `requestId`, and never retries
  the effect as a new request. The `terminal.viewer`/`terminal.control` request family these
  commands ride keeps no request ledger: `requestId` correlates a live request with its result
  but is not a Server-ledger identity. (The Server does retain a bounded `workbench.command`
  ledger that replays an identical settled resubmission and rejects a different command under the
  same `requestId` as an identity conflict; it does not cover this family.) Reconciliation is
  therefore observational: the operator verifies the terminal through `terminal read`,
  `terminal wait-output`, or `terminal open` before deciding to re-issue intent.
- `--detach` is not offered on any command in this slice: none of these operations owns a
  durable Server-side identity for later observation, and the accepted contract forbids turning
  terminal input into a fire-and-forget effect.

## Grammar additions

Command paths extend the delivered grammar unchanged — option scan, `--` termination,
`--format text|json`, `--help` suffixes, and `help` paths behave exactly as today; completion
projects the new verbs and flags and never enumerates content or runtime state:

```
orchbee terminal send-text <target> (--file <path> | --stdin) [--paste]
orchbee terminal send-keys <target> <key> [<key>...]
orchbee terminal wait-output <target> (--text <literal> | --regex <pattern> | --silence <duration>)... [--timeout <duration>]
orchbee terminal open <target> [--timeout <duration>] [--frames <count>]
```

- `<target>` is the terminal's typed ID; only presentation-bound terminals visible in the
  Workbench snapshot are reachable, matching `terminal list`/`terminal get`.
- `<duration>` is a positive integer with an optional `ms`/`s`/`m` unit suffix (bare number =
  milliseconds). Exact spellings and value ranges are reserved; proposed ranges appear below.
- Repeated predicate flags on `wait-output` are allowed and combine with OR semantics; the wait
  ends when any single predicate is satisfied.
- No command in this slice takes positional content, so `send-text` content can never appear in
  `argv` by construction.

## `terminal send-text`

`orchbee terminal send-text <target> (--file <path> | --stdin) [--paste]` writes caller-supplied
text into the terminal's input queue as if typed — or, under `--paste`, as one semantic paste
that Host encodes under the terminal's current bracketed-paste mode.

Input source and bounds:

- Exactly one of `--file <path>` or `--stdin` is required; supplying both or neither is
  `invalid-usage`. There is deliberately no inline text option: content never appears in `argv`,
  process listings, human output, diagnostics, or shell completion.
- Content must be nonempty valid UTF-8 and at most 262,144 bytes (256 KiB, aligned with the
  largest single-input bound on the wire). An empty, oversized, non-UTF-8, unreadable, or
  missing source is `invalid-usage` before any runtime access.
- `--stdin` reads stdin fully and only when selected; no stdin or file input is ever read
  implicitly.

Delivery mapping:

- Default (`input` mode): the byte content is split into `terminal.control` `input` actions of
  at most 4,096 bytes each — chunk boundaries fall anywhere in the byte stream because Host
  concatenates the queue byte-exactly — with `sequence` `1..N`. Success means every chunk
  returned `input-accepted`.
- `--paste`: the whole content is submitted as one `paste` `OBCQ` record; `pasted`
  `{acceptedBytes}` reports the encoded byte count Host admitted. Paste is the safer delivery
  for multi-line or sensitive content because a bracketed-paste-aware application receives it as
  one paste event rather than executable keystrokes.
- The command attaches and acquires first; if another Client holds the controller the command
  fails `controller-busy` — a denial, never a wait.

Result `orchbee.terminal.send-text`:
`{terminal:{terminalId,workerId},sent:{mode:"input"|"paste",bytes:<n>,requests:<n>,requestId:"<hex32>"}}`
where `requestId` is the final confirmed effect request — its acceptance certifies the
submitted prefix under the ordered-sequence rule — `requests` counts effect actions, and
`bytes` counts submitted source bytes. Human text reports only the byte count and terminal
identity; it never echoes content.

Consistency with the noninteractive/secret rules: content arrives only through an explicitly
selected bounded source; output reports counts and identities, never bytes; a partial send after
a mid-stream failure reports `bytes` actually confirmed; an ambiguous effect reports
`outcome-unknown` with the request's identity for reconciliation.

## `terminal send-keys`

`orchbee terminal send-keys <target> <key> [<key>...]` compiles each key token to its byte
encoding and writes the concatenated stream as typed `input` actions, chunked at 4,096 bytes
exactly like `send-text`. Key tokens come from a closed vocabulary; an unrecognized token is
`invalid-usage` — there is no literal-text fallback, so a typo cannot inject unexpected text and
literal content keeps a single secret-safe home (`send-text`).

Proposed vocabulary and encodings (final spellings reserved):

| Token                                               | Bytes                                                                                                                                                                                                                                                      |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `C-<c>`                                             | Control byte: `C-a`…`C-z` → `0x01`…`0x1a` (case-insensitive); `C-@`/`C-Space` → `0x00`; `C-[` → `0x1b`; `C-\` → `0x1c`; `C-]` → `0x1d`; `C-^` → `0x1e`; `C-_` → `0x1f`; `C-?` → `0x7f`                                                                     |
| `M-<key>`                                           | `0x1b` plus the encoding of `<key>` (meta/alt)                                                                                                                                                                                                             |
| `C-M-<c>`                                           | `0x1b` plus the `C-<c>` control byte                                                                                                                                                                                                                       |
| `Enter`, `Escape`, `Tab`, `Space`, `BSpace`, `BTab` | `0x0d`; `0x1b`; `0x09`; `0x20`; `0x7f`; `0x1b 0x5b 0x5a` (shift-tab)                                                                                                                                                                                       |
| `Up`, `Down`, `Right`, `Left`                       | `0x1b 0x5b 0x41`/`0x42`/`0x43`/`0x44` (CSI normal cursor mode)                                                                                                                                                                                             |
| `Home`, `End`                                       | `0x1b 0x5b 0x48`/`0x46`                                                                                                                                                                                                                                    |
| `Insert`, `Delete`, `PageUp`, `PageDown`            | `0x1b 0x5b 0x32 0x7e`; `0x1b 0x5b 0x33 0x7e`; `0x1b 0x5b 0x35 0x7e`; `0x1b 0x5b 0x36 0x7e`                                                                                                                                                                 |
| `F1`–`F4`                                           | `0x1b 0x4f 0x50`/`0x51`/`0x52`/`0x53` (SS3)                                                                                                                                                                                                                |
| `F5`–`F12`                                          | `0x1b 0x5b <n> 0x7e` with `<n>` = `15`,`17`,`18`,`19`,`20`,`21`,`23`,`24`                                                                                                                                                                                  |
| `C-`/`M-`/`S-` + named special                      | xterm modifier form: arrows/Home/End → `0x1b 0x5b 0x31 0x3b <m> <final>`; `Insert`/`Delete`/`PageUp`/`PageDown`/`F5`–`F12` → `0x1b 0x5b <n> 0x3b <m> 0x7e`; `F1`–`F4` → `0x1b 0x5b 0x31 0x3b <m> P/Q/R/S`, where `<m>` = `1 + shift(1) + alt(2) + ctrl(4)` |

Relationship to the existing input encoding: the Host's semantic `KeyInput` path (W3C-style key
codes with mode-aware encoding through Ghostty VT) is not reachable through OBCS version 1 —
`terminal.control` `input` carries raw bytes only. The CLI therefore compiles key names to the
same normal-mode sequences a real keypress produces in the terminal's default modes. This is
deterministic and adequate for shells and line-oriented tools, but it cannot express
mode-dependent encodings (application cursor keys, kitty keyboard protocol, `modifyOtherKeys`):
a terminal application in such a mode would see different bytes from a physical keypress. A
future wire extension carrying semantic keys is a reserved decision, not part of this slice.

Result `orchbee.terminal.send-keys`:
`{terminal:{terminalId,workerId},sent:{keys:<n>,bytes:<n>,requests:<n>,requestId:"<hex32>"}}`.
Key names are `argv` command specifiers by design — they appear in the invocation and are
completable, like flag names; they are not content. Anything that could carry a secret belongs
to `send-text`, whose design admits no `argv` content.

## `terminal wait-output`

`orchbee terminal wait-output <target> <predicate>... [--timeout <duration>]` attaches a
read-only viewer and waits until one predicate is satisfied or the deadline passes. It never
acquires the controller and cannot contend for it.

Predicates (at least one required; combining is OR):

| Flag                   | Proposed bound             | Semantics                                                                                                                                                                      |
| ---------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `--text <literal>`     | 1–4,096 UTF-8 bytes        | Satisfied when the visible screen text contains the literal substring                                                                                                          |
| `--regex <pattern>`    | 1–4,096 bytes, compiles    | Satisfied when the pattern matches anywhere in the screen text; the Rust `regex` linear-time syntax (no backreferences/lookaround); an uncompilable pattern is `invalid-usage` |
| `--silence <duration>` | 1–600,000 ms               | Satisfied after `<duration>` passes with no newly applied frame since the last applied frame (or since attach)                                                                 |
| `--timeout <duration>` | 1–600,000 ms, default 30 s | The mandatory overall bound on the observation phase; reaching it without a satisfied predicate ends the wait                                                                  |

Evaluation plane: predicates evaluate over the visible screen text — the frame's visible rows
joined as text, the same projection plane `terminal read` exposes — evaluated after every
applied frame, including the first post-attach snapshot. This command deliberately does not
promise a byte-stream or scrollback semantic: content that appears and scrolls off within one
delivered frame revision is not observable in this slice; stream-scope predicates are reserved.

Why this is bounded observation, not infinite polling:

- The deadline is mandatory: `--timeout` always bounds the observation (with a proposed default
  and a hard cap), so no invocation can wait forever.
- One Session, one snapshot read, and one attachment cover the whole wait — it is not
  `terminal read` re-run in a loop; snapshots taken that way would be independent samples that
  can miss intermediate screen revisions and would rebuild a Session and attachment each time.
- Delivery is ordered and lossless per viewer: the CLI acknowledges each record with
  `frame-applied` before the next is produced, so every revision the Host emits is evaluated —
  sampling cannot skip it.
- Recovery is the existing semantic path (`need-snapshot` on sequence or decode divergence), not
  re-attachment.
- The poll interval has a fixed floor (proposed 25 ms) and runs under the deadline; a record
  that reports `blocked` (`capture-allocation-failed`/`projection-too-large`) counts as activity
  for `--silence` but carries no screen and is never predicate-evaluated.

Result `orchbee.terminal.wait`:

- Satisfied, exit 0:
  `{terminal:{terminalId,workerId},wait:{matched:true,predicate:{kind:"text"|"regex",value:"<pattern>"}|{kind:"silence",durationMs:<n>},revision:"<u64>",sequence:"<u64>",matchedText:"<line>",elapsedMs:<n>}}`
  — `matchedText` is the first matching screen line, bounded to 4,096 bytes; `revision` and
  `sequence` identify the frame in which the match was observed.
- Deadline reached, exit 1:
  `{terminal:{terminalId,workerId},wait:{matched:false,reason:"deadline",elapsedMs:<n>}}`
  — a determinate negative result carried in the success schema (the same pattern as
  `config check` reporting `ok:false`), not a failure envelope. Scripts branch on the exit code;
  agents read the typed `matched`/`reason` fields.
- A terminal that disappears mid-wait makes the wait unsatisfiable: `not-found`
  `{resource:"terminal",target}` or `service-unavailable` by the wire outcome, not a
  `matched:false` result.

`matchedText` is terminal output the caller is already authorized to read (the same authority as
`terminal read`); it is bounded and appears only in the result document, never in diagnostics.

## `terminal open`

`orchbee terminal open <target> [--timeout <duration>] [--frames <count>]` attaches a read-only
viewer and streams every delivered screen frame until the stream ends. "Open" is the accepted
product verb for connecting a view onto a terminal; this command is a viewer attachment plus
streamed output, not the wire `terminal.open` operation — the OBCS `terminal.open` request is
the Server-owned standalone-terminal create-or-rejoin flow used by the bare-TUI bootstrap and is
unrelated to this command. The command never acquires the controller and never sends input.

Stream boundaries:

- `--format json` emits NDJSON: one `{schema:"orchbee.terminal.open",version:1,source,event}`
  object per line, where `source` is the `{kind:"workbench-snapshot",sessionId,revision}` used for
  target resolution. Events, in stream order:
  - `attached` — `{kind:"attached",terminal:{terminalId,workerId},attachmentId,hostEpoch}`
    exactly once, first.
  - `frame` — `{kind:"frame",sequence:"<u64>",generation:"<u64>",revision:"<u64>",cursor:{column:<n>,row:<n>},rows:[{index:<n>,text:"<row>"}],truncated:<bool>}` —
    the complete visible screen at that revision (every event is self-contained; no consumer
    state is required), emitted per applied record. Emitted `rows` text is capped at 256 KiB per
    event; overflow drops tail rows and sets `truncated:true`.
  - `blocked` — `{kind:"blocked",reason:"capture-allocation-failed"|"projection-too-large",revision:"<u64>"}`
    when the projection cannot deliver a frame; delivery resumes with a fresh snapshot.
  - `ended` — `{kind:"ended",reason:"detached"|"terminal-unavailable"|"timeout"}` exactly once,
    last, then exit 0. `terminal-unavailable` ends the stream cleanly when the observed terminal
    disappears; the reason field reports how the stream ended.
- `--format text` prints the complete visible screen text after every applied frame, separated
  by a single form-feed (`\f`) line, under the same per-record text cap. It is a faithful
  rendering of the screen-state data plane — this surface does not provide a byte stream — and
  human output is never an automation API.
- `--timeout <duration>` bounds total stream time; reaching it emits `ended` `timeout` and
  exits 0. `--frames <count>` bounds the number of `frame` events; reaching it emits `ended`
  `detached`. Both are optional; with neither, the stream runs until the terminal detaches or
  the caller interrupts (`SIGINT` → exit 130 with no guaranteed final event).

The stream is unbounded in duration but every emitted event is byte-bounded and the transport
itself is bounded — one Session, one attachment, ordered lossless records acknowledged
one-by-one. A command that wants to stop on content uses `wait-output` instead.

## Output contract

- Machine data stays on stdout; localized diagnostics stay on stderr; text mode is the default
  and `--format json` selects the versioned machine documents above. `open`'s machine document
  is NDJSON because the operation is a stream; the others emit one pretty JSON document.
- Success envelopes keep the delivered `{schema,version:1,source,result}` shape with
  `source:{kind:"workbench-snapshot",sessionId,revision}` from the resolution read. `source`
  describes the resolution snapshot, not effect-time state.
- New schema IDs: `orchbee.terminal.send-text`, `orchbee.terminal.send-keys`,
  `orchbee.terminal.wait`, and `orchbee.terminal.open` (NDJSON event envelope).
  `orchbee.cli.failure` is unchanged in shape; only its `kind`/`details` vocabulary grows below.
- One-shot documents are prepared in the existing bounded buffer and keep the 8 MiB
  `output-too-large` ceiling. `open` events are bounded individually (256 KiB emitted text)
  rather than by the document cap, because the stream is incremental.
- Secret class: `send-text` content is terminal-input payload — it never appears in `argv`,
  stdout, stderr, diagnostics, or completion; only counts and identities are reported.
  `send-keys` tokens are command specifiers in `argv` by design, like flags; the closed
  vocabulary prevents content smuggling. `wait-output` predicates are caller-chosen selectors in
  `argv`, like `grep` patterns; `matchedText` and `open` frame text are terminal output under
  the caller's existing read authority.

## Failure matrix

Exit codes keep the delivered table: `0` success; `1` local determinate outcome (input-source,
internal, output failures, and the `wait-output` deadline-miss result); `2` usage/invalid
target; `3` rejection; `4` service unavailable; `5` transport loss/protocol invalid; `6`
incompatible; `130` `SIGINT`. `7` remains reserved for the failed-effect category (no producer
in this slice); `8` is assigned to the unknown-outcome category as `outcome-unknown`.

New or extended vocabulary:

| Kind                  | Exit | Details                        | Produced by                                                                                                                                                                                                                 |
| --------------------- | ---- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `controller-busy`     | 3    | `{resource:"terminal",target}` | `acquire` → `rejected{controller-busy}`: another Client holds the combined input/resize grant. Determinate denial — the Server queues nothing                                                                               |
| `denied`              | 3    | `{reason}` added               | Other `rejected` outcomes: `not-controller`, `host-rejected`, `host-input-invalid`, `host-input-limit-exceeded`, `host-input-backpressure`, `host-input-unavailable`, `feedback-mismatch` — `reason` carries the wire value |
| `outcome-unknown`     | 8    | `{stage,requestId,sentBytes?}` | Wire `outcome-unknown` or `SubmissionError::OutcomeUnknown`; `sentBytes` reports confirmed input bytes when an input sequence dies mid-send                                                                                 |
| `not-found`           | 3    | `{resource:"terminal",target}` | Target unresolvable in the snapshot, `attach` → `rejected{terminal-unavailable}`, or mid-operation `rejected{viewer-unavailable}`                                                                                           |
| `service-unavailable` | 4    | `{stage,reason}`               | `unavailable{server-busy\|terminal-runtime-unavailable\|workbench-unavailable}` at any stage after the bounded retry is exhausted                                                                                           |
| `internal-failure`    | 1    | omitted                        | `rejected{sequence-mismatch}` or `already-attached` (unreachable if sequence/attachment tracking is correct), encoding errors, deadline-construction failure                                                                |
| `transport-loss`      | 5    | `{stage}`                      | `RequestNotSubmitted` — a request that provably never reached the transport carries no ambiguous effect                                                                                                                     |

Stage names extend the delivered set (`cohort`, `runtime-base`, `runtime-directory`, `socket`,
`discovery`, `admission`, `snapshot`, `platform`) with `attach`, `acquire`, `input`, `observe`,
and `stream`. The existing kinds — `invalid-usage`, `invalid-identifier`, `ambiguous`,
`check-failed`, `connection-capacity`, `protocol-invalid`, `incompatible`, `output-too-large`,
`output-failed` — apply unchanged. `ambiguous` has no producer in this slice because terminal
targets accept only typed IDs.

## Session state machine and budgets

Each command performs the delivered ordered stages — cohort (5 s checked deadline), discovery,
socket, admission — then exactly one snapshot read under the admission deadline, then its
command phase on the same Session:

- `send-text` / `send-keys`: `attach-binding` → `acquire` → effect actions → `detach`, all under
  one checked effect-phase deadline (proposed 15 s; a 256 KiB chunked send is ≤64 sequential
  local-socket actions). On success, `detach` is attempted once; on any failure the Session is
  dropped, which performs the same cleanup.
- `wait-output`: `attach-binding` → observe loop (`poll` → apply → evaluate → `frame-applied`)
  → `detach`, bounded by the `--timeout` deadline covering the whole phase.
- `open`: `attach-binding` → stream loop → `detach`, unbounded in duration with each request
  individually bounded by the standard request deadline.

A syntactically invalid invocation — grammar, format, target shape, missing or dual input
source, oversized or non-UTF-8 content, uncompilable regex — fails before any filesystem or
network access, exactly as the delivered commands do.

## Invariants

- `send-text` content never appears in `argv`, stdout, stderr, diagnostics, or completion; only
  counts and identities are reported. No stdin or file input is read implicitly; exactly one
  explicit source flag is required.
- Every command holds at most one Client Session, one snapshot read, one viewer attachment, and
  one controller grant for its lifetime; `detach` is best-effort and Session loss performs
  equivalent cleanup.
- `input`/`paste` actions are submitted at most once, serially, under strictly increasing
  sequences; an ambiguous outcome stops the command and reports `outcome-unknown` with the
  request identity; the effect is never retried as a new request.
- Controller contention is a `controller-busy` denial, never a queue or a wait; nothing in this
  slice waits for a grant.
- `wait-output` always runs under a mandatory deadline, evaluates every delivered revision, and
  treats `blocked` records as activity without screen content.
- `terminal open` and `terminal wait-output` never acquire the controller; `send-*` never emits
  terminal content.
- `--detach` is absent from every grammar in this slice.
- The CLI submits semantic intent to the Server and validates typed results; it never connects
  to Terminal Host directly, never parses raw PTY bytes as authority, and never treats terminal
  output as Agent or Workbench truth.

## Reserved decisions and open questions

Reserved for the owning stage: final option spellings and value ranges; the complete
`wait-output` predicate set (including any stream-scope or scrollback predicates); agent
control-domain ownership and any separate agent-scoped policy; exact bound values (byte caps,
poll interval, default/cap timeout, per-event text cap); whether a future wire extension carries
semantic `KeyInput`; and any `--frames`/`--timeout` spelling changes.

Decisions taken in this proposal: denial rather than queueing on controller contention;
client-side key compilation to normal-mode bytes; the visible-screen predicate plane; NDJSON
`open` events with self-contained screen rows; deadline-miss as a determinate `matched:false`
result at exit 1; `outcome-unknown` at exit 8 with request identity; no `--detach`.

## Scenario matrix and evidence

| Scenario                                                                                                           | Expected                                                                   | Evidence                                                        |
| ------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Grammar: input-source exclusivity, `--`, formats, help forms, predicate requirements, unknown key tokens           | exact `invalid-usage`/typed-ID failures before any runtime access          | `src/invocation` tests + `tests/cli_query.rs`-style integration |
| Key vocabulary: every name/modifier combination                                                                    | golden byte-encoding vectors; unknown tokens rejected                      | key-table unit tests                                            |
| Chunking: 0/1/4,096/4,097/262,144/262,145-byte sources, UTF-8 boundary splits                                      | correct chunk count, `invalid-usage` at bounds, byte-exact stream          | send-path unit tests                                            |
| Scripted OBCS peer: acquire `controller-busy`; input rejections; `unavailable` reasons; `outcome-unknown` mid-send | typed kinds per matrix; effect never retried; `requestId` reported         | scripted-peer integration tests                                 |
| Viewer lifecycle: `attach-binding` rejection/`unavailable` precedence, poll/`frame-applied`, `need-snapshot`       | typed mapping; single attachment; ordered acknowledgement                  | scripted-peer integration tests                                 |
| `wait-output`: match on first snapshot, mid-stream match, `--silence`, deadline, `blocked` records, terminal loss  | `matched`/`deadline` semantics; every revision evaluated; typed end states | scripted-peer tests with fabricated frame sequences             |
| `open`: NDJSON event order, self-contained rows, truncation flag, `ended` reasons, `--timeout`/`--frames`          | event grammar and bounds hold under fabricated streams                     | scripted-peer + golden NDJSON tests                             |
| Secret non-leakage: content bytes absent from every output/diagnostic/completion surface                           | scan-based assertions on stdout/stderr/completion under both locales       | output tests                                                    |
| Non-TTY execution, `SIGINT` mid-wait/mid-stream, stdout write failure                                              | documented exits; Session dropped before blocking                          | integration tests                                               |
| Real staged-cohort Server: send-*/wait/open against a live terminal alongside a TUI holding the controller         | `controller-busy` denial; successful paths after release; no leaked grants | `tests/local_startup_process`                                   |

Model obligations: this slice adds no new Server state machine, persistence, or retry — it
composes the registered OBCS viewer/control models and one client-side bounded observation
loop — so no new TLA+ model is required. Registered models whose triggers cover `apps/orchbee`
source and test paths must still actually run; the inventory output is not a substitute.
