# Windows semantic event subscriptions

## Accepted behavior

Windows scripts observe actual Workspace, Tab, Panel, worktree, Agent and Client changes, or wait for a specific change through the owned event stream. Current snapshots cannot substitute for retained history.

Native names and payloads follow canonical Workspace/Tab/Panel identities and Client-local focus
and viewport ownership. A Panel is the observed terminal unit; a source may explicitly identify a child
Presentation. Native typed receipts and structured terminal rows remain authoritative. This change
does not add Desktop, macOS or Linux event consumers.

## Event ownership and ordering

One Session event journal receives accepted semantic changes at their real commit or observation
point. Topology events derive from the before/after canonical projections inside the same commit;
every intermediate commit is retained until journal eviction, including commits published as full
Snapshots. Mere revision advance, refused commands and repeated unchanged observations emit no
change. Moving an identity does not create or close it. Workspace metadata, worktree settlement and
physical Agent transitions publish from their existing owners after admission. Client focus/layout
and viewport observations retain the Client identity and do not claim global focus.

Terminal output changes carry the admitted Host frame generation and content revision, rather than
the temporary viewer's capture sequence. Terminal exit requires an exact current Host lifecycle
observation of settlement; process-read failure or Agent departure never proves terminal exit.
The existing periodic observer retains bounded last-observed versions, so unobserved physical
transitions may coalesce. A newly observed source establishes a baseline; observed later changes
enter the Session journal. Requested output-match probes independently inspect their admitted window.

Client observations may coalesce UI steps before publication. An optional full-width `splitRevision`
tracks the actual Client split-ratio owner, including direct mouse/keyboard adjustments; omitted
legacy observations provide no ratio-change evidence. Refused and unchanged adjustments never
advance it. Accepted Client selection/zoom/ratio observations publish their own semantic changes,
without claiming every UI transition happened to be observed.

The journal has a fresh boot epoch, increasing full-width sequence, at most 512 records and an
8 MiB retained-byte budget. Each admitted event is at most 256 KiB. Eviction, an oversized event or
sequence exhaustion cannot silently produce a complete history: readers receive an explicit gap or
unavailable result. Cursors pin both epoch and sequence. Wrong-epoch and future cursors refuse;
filtered-out records still advance the scan cursor. No durable journal or replayed runtime effect is
created.

## Subscription and wait lifecycle

An authenticated Client connection owns at most one subscription with up to 32 immutable filters.
Opening captures the journal head or validates an explicit cursor before installing the owner.
An identical repeated open identifies the same owner; a different live subscription refuses.
Read and bounded wait consume the same retained scan cursor and probe state. Replies contain at
most 32 events and remain bounded independently of the global journal. Closing or EOF drops the
subscription, compiled matchers and observation state. A new connection cannot claim its identity.

| State  | Input              | Guard                                                      | Result and next state                         |
| ------ | ------------------ | ---------------------------------------------------------- | --------------------------------------------- |
| Vacant | Subscribe          | Read grant, valid Session, bounded filters/cursor/matchers | Capture baseline; Open                        |
| Open   | Subscribe          | Same ID and immutable definition                           | Existing receipt; Open                        |
| Open   | Subscribe          | Different ID or definition                                 | Conflict; Open                                |
| Open   | Read / Wait        | Same subscription, valid bounds                            | Events, current, gap or timed-out slice; Open |
| Open   | Close              | Same subscription                                          | Closed; Vacant                                |
| Any    | EOF / cancellation | Connection owner ends                                      | Release owned state                           |

Wait checks its admitted baseline before testing a deadline. CLI wait uses one filter and retains
the same subscription across bounded wait slices; a slice timeout is not the overall timeout. A
known final timeout is distinct from unavailable source, history loss, malformed reply and user
interruption. Subscription/wait never changes terminal input, navigation, scroll position or Agent
state. Cancelling observation cannot cancel a separately accepted runtime effect.

## Physical and Client probes

Output substring/regular-expression matching uses an actual revision-admitted read-only terminal
window, the existing temporary-viewer cleanup owner and a bounded match line. Invalid regex fails
before subscription installation. A match emits on the unmatched-to-matched edge; unchanged
matching text does not flood subscribers. Recent/history probes use the requested bound. Viewport
and scroll probes require a current exact Client/source observation and use the existing read-only
Client control path, preserving request, membership and content-version correlation. They never
substitute a newly attached viewer's bottom viewport for the selected Client's viewport.

Visible means the selected Client viewport, recent-unwrapped joins proven soft wraps, and detection reads
the actual live screen. Native history is an additional retained-scrollback source. Plain and SGR
reads share physical source/version admission; SGR is synthesized from validated cell styles rather
than claiming an exact copy of historical PTY input bytes.

Physical window capture admits at most 65,536 rows and 8 MiB of row storage/text. Bounds refuse
without emitting a partial match. Recent-unwrapped discards an incomplete older prefix only when
the requested number of complete retained logical lines follows it; an incomplete requested line
refuses instead of inventing its missing prefix.

Agent status probes may emit the admitted initial current state;
later changes come from actual admitted Agent observations. Probe records identify their physical
or Client observation origin separately from journal records. A journal cursor alone does not prove
unobserved physical output/scroll transitions, and opening a new probe establishes a new baseline.
Source loss or uncertain temporary-viewer cleanup remains an explicit refusal/unknown ownership
condition; no replacement observer is attached until the old owner settles.

Physical output capture runs in at most one connection-owned observation actor. Bounded waits poll
its correlated completion instead of blocking on Worker I/O. Close/EOF cancels and joins the actor;
an already-required Worker completion is consumed before cleanup, and uncertain viewer cleanup
remains with the existing Session observation owner. Read-only Client query receipts retain the existing
Client registry's bounded delivery/settlement ownership after the subscription ends.

## Correctness and evidence

The deterministic core checks ordering, filters, cursor gaps and topology change classification;
the runtime checks subscription ownership, actual publishers, physical/Client fences, deadline and
EOF cleanup. Shared Rust/TypeScript/JSON cases establish closed shape, full-width identities and
bounds. Executable CLI stream/wait witnesses must cover match, timeout, gap, refusal, interruption
and lost/malformed correlation. Windows integration must prove actual mutations and terminal
output arrive at an already-open subscription. These claims require their respective witnesses;
protocol admission alone does not complete R089/R090 or final composed acceptance.

CLI event failures after admission use the closed `orchbee.cli.failure.events` envelope. Existing
admission and target-resolution failures retain their own published details. A timeout never
fabricates a Server receipt: wait returns the last actually received timed-out slice; the stream
ends with its own closed record and known-close or session-release cleanup disposition.

A physical or Client-scroll probe must establish its first admitted baseline before a known
no-match timeout. A wait slice that ends earlier returns Busy, retaining the subscription and
actor. CLI retries Busy on that same owner while its overall deadline permits; an expired
deadline with no observed baseline returns the actual Busy receipt, not a fabricated timeout.
