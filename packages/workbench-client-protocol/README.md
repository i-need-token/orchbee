# @orchbee/workbench-client-protocol

Strict, runtime-neutral data contracts for one Orchbee Workbench page client.

## Scope

The package owns validated messages for a shared Workbench client session, terminal projection control, and the projected attachment stream opened for one visible terminal viewport. Electron, ordinary Browser pages, and future mobile clients may carry these messages through different transports without changing their Workbench-client meaning.

The package also supplies the independent TypeScript binding for the additive version-1 read-only
Workbench projection kernel. The language-neutral authority and canonical conformance corpus live
under [`schemas/workbench`](../../schemas/workbench/README.md). The binding parses one strict
`workbench.snapshot.read` intent, complete bounded snapshots, and revision-chained aggregate deltas;
`applyWorkbenchDelta` applies a delta atomically or returns one typed recovery reason. This contract
contains Project, Directory, Workspace, Tab, Panel, and Terminal Presentation product semantics,
including an exact bounded POSIX path and required nullable primary Directory reference, but
excludes Environment, Directory discovery or mutation policy, Worktree, Agent, Launch Profile,
terminal content, persistence, transport, and every Client-local view fact. No current
client-session consumer uses this new contract yet.

The separate command binding validates `workbench.command` and correlated command/snapshot results.
It also covers the revision-cursor `workbench.publication.read` intent and its
`workbench.publication.result`, whose `published` outcome carries one through 64 ordered
`workbench.snapshot`/`workbench.delta` documents under the shared snapshot-result byte bound.
It includes Project and Workspace changes, `workspace.primary-directory.accept`, canonical
`tab.rename`, `tab.move` within standalone or Workspace contexts, and
`tab.create` in standalone or configured Workspace contexts with its committed terminal result variants. Move carries exactly
`terminalContextId`, `tabId`, and `direction: previous | next | first | last`; `tab-moved` carries
exactly `revision`, `terminalContextId`, and `tabId`.
The Server owns ordering/no-op semantics; consuming adapters own request/result correlation and
action compatibility. The bindings and both runtime binaries activate and roll back together as one
local unpublished OBWP v1 cohort; old closed decoders reject Move. It reuses projection identities,
labels, revision and
snapshot invariants, rejects unknown fields, and checks encoded request/result limits. The native
Server consumes the independent Rust binding over OBWP; this package owns decoded values and does
not implement OBWP transport, duplicate-key decoding, retries, shared-state mutation or
authorization policy.

Workspace primary Directory acceptance carries only Project, Workspace, and exact path input.
Its accepted result carries revision plus the Server-selected Directory and Worker identities.
The binding keeps `workspace-directory-unavailable` and `workspace-directory-conflict` in the
rejection domain and `directory-unavailable` in the command-unavailable domain. It validates the
closed wire and projection values; filesystem observation, Worker selection, authorization,
set-once admission, commit, and replay remain Server responsibilities.

The staged Close binding adds `tab.close`, immutable `tab-close-accepted`, effect-free
`workbench.tab-close.read` and explicit `workbench.tab-close.resume` with their strict results.
`WorkbenchTabCloseAttemptGenerationSchema` validates a nonzero canonical decimal u64 domain
separate from revision. Closed status/phase/reason unions preserve known disposal, blocked cleanup,
unknown effects and final removal as different facts. Resume failure target echoes only repeat
submitted identities, including for Denied; they do not confirm an existing operation.
The [Close wire contract](../../schemas/workbench/README.md#accepted-tab-close-wire-contract)
owns shapes and the shared oracle. These decoded-value parsers do not activate a lifecycle effect
or prove Server/Worker/native integration; that cohort must move together.

Every `terminal-projection-open-requested` command carries one complete positive
`TerminalGeometry`. It is the caller's measured mounted geometry for a visual surface, not an
optional resize hint or fallback. Request identity therefore includes presentation identity and
geometry: an exact duplicate may replay, while reuse with different geometry is conflicting control
traffic. The transport contract supplies no timeout or 80x24 default.

The projected attachment stream carries semantic bell events without choosing a sound, visual
flash, or notification policy. Each event retains the Host attachment identity and terminal-owned
sequence so the projected terminal can suppress stale or duplicate delivery.

The same stream carries a complete attachment-scoped resize-lease ownership change. A renderer
validates both attachment and terminal identities before dynamically enabling or disabling Host
resize requests; the immutable attachment bootstrap is not rewritten when ownership changes.

Live input authority uses a separate `attachment-input-capability-changed` event containing the
complete Host-committed attachment capability state and positive revision. The transport cannot
request or synthesize this mutation. A client validates the attachment and terminal identities and
applies only an exact replay or exact-next changed input value; stale delivery is inert and a
conflict or revision gap fails closed. Downgrade makes PTY Actions unavailable synchronously while
history, search, selection, and copy remain local output interactions. Upgrade carries no queued
input and grants only future input stamped with the new revision.

Feature failures retain their semantic operation and retryability across both Electron and Gateway
transports. A Host response additionally retains its exact correlation ID, while client-owned copy
and link effects retain their request sequence. These values identify consequences and diagnostics;
they never authorize automatic replay of input or another user effect.

Local filesystem effects are an optional attachment capability, absent by default. When present,
`link-open` carries a discriminated URL or file target instead of granting a generic opener. URL
targets retain the shared scheme policy; file targets are meaningful only to the owning local
Desktop broker. Browser, Gateway, archived, and source-neutral attachments do not acquire local
application-launch or filesystem-read authority from terminal text.

Image preview is a separate request-bound flow. A capable renderer may request one detected local
image path and cancel the exact request sequence. Desktop replies with either an exact bounded
`ArrayBuffer` plus one admitted raster media type, or an unavailable result that reveals no file
detail. The protocol caps preview content at 8 MiB and transports no file handle, directory listing,
cache key, or persistence instruction. Request identity lets the renderer reject late content after
hover replacement or attachment retirement; it does not authorize retry or remote file transfer.

Projection release remains correlated after the attachment stream closes. The client retains the
request identity until Main returns either `terminal-projection-released` or the exact typed cleanup
failure. The success acknowledgement transfers no port; a late cleanup result therefore settles only
its released request and cannot be mistaken for an unknown message that retires unrelated projections.
Because command delivery and asynchronous open completion are independently scheduled, a correlated
`terminal-projection-opened` may reach the client after that client requested release. The client
closes exactly the transferred attachment port, keeps the released request identity until cleanup
settles, and leaves the shared control transport and unrelated requests available. A missing or extra
transferred port, conflicting identity, or duplicate open remains invalid control traffic and retires
the affected control transport.

It contains no Electron, DOM, UI, broker, coordinator, transport lifecycle, network framing, Gateway, authentication, PTY, or VT implementation. Current shared-document and legacy TerminalPresentation semantics remain with `@orchbee/workbench` until their accepted migration; the additive version-1 projection semantics come from the language-neutral schema authority. Terminal Host wire semantics remain with `@orchbee/terminal-protocol`.

Applications parse every untrusted IPC or network value with these schemas at the first trusted boundary. A transport owner remains responsible for framing, authentication, binary transfer, backpressure, and connection retirement.

Each client-session bootstrap carries `terminalHostName`, supplied by the application runtime that
owns the projected terminal machine. The shared Web client uses that trusted value to validate OSC
7 current-directory metadata; it never substitutes the Browser URL hostname, Gateway peer name, or
renderer input. Projected frames carry the complete nullable natural title and current-directory
observation. The protocol transports those values without selecting Tab-title precedence,
persistence, or URI trust policy; `@orchbee/workbench` owns those decisions.

Projected frame and row-window events accept either an exact `ArrayBuffer` or a bounded `Uint8Array` view so in-process adapters can preserve a validated payload view. The visible payload is bounded by `maxFrameBytes` for frames and `maxRowWindowBytes` for row windows; a view's unrelated backing bytes do not count toward that limit. Before structured-cloning a view into a less-trusted capability, the transport owner must copy its visible bytes into an exact-size `ArrayBuffer`; structured clone carries the complete backing buffer, including bytes outside the view. Receivers still honor the declared byte offset and length, while transport trust and terminal-protocol validation remain separate responsibilities.

Selection export remains attachment-scoped and plain-text-only. The projected command has no format
selector. A Desktop-effect attachment receives `selection-copied` after Main commits the clipboard
effect; a client-effect attachment receives `selection-copy-content` with bounded UTF-8 bytes and
commits those bytes through its own platform clipboard before reporting local completion. Neither
event grants HTML, raw VT, or a generic clipboard capability.

Host-originated clipboard-write commit is a separate, request-bound flow. Main forwards at most one pending bounded request per attachment, and the renderer automatically returns only an exact `clipboard-write-decision` containing the original attachment, sequence, and text plus the commit result. Desktop Main rejects mismatched and duplicate decisions and performs the privileged write only for a positive Desktop-effect decision. A client-effect attachment performs its local Browser write first and uses the same exact decision only to consume Main's pending request, so Main cannot duplicate the effect.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/workbench-client-protocol build
pnpm --filter @orchbee/workbench-client-protocol typecheck
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol test:exports
```

## Worktree operation binding

`workbench-worktree-contract.ts` implements the [retained Worktree wire contract](../../schemas/workbench/v1/workbench-worktree.schema.json). Request/response unions include Start/Read/Resume and retained partial outcomes, with their own 96 KiB encoded budget. Strict objects, UTF-8 reference bounds, attempt bounds, checkout/path equality, Git status/diagnostic constraints and reused/launch-outcome agreement match the independent Rust binding. Both consume the [shared corpus](../../schemas/workbench/v1/workbench-worktree-conformance-v1.json). The shared `workbench-terminal-launch-contract.ts` leaf owns terminal geometry and launch outcomes; the command module keeps its existing exports without a cycle. No Desktop runtime command is added by this binding.

## Worktree source-query binding

`workbench-worktree-query-contract.ts` adds the separate read request/result to the shared unions, with the same 32 KiB result cap, strict nullable fields, closed failures and cross-entry invariants as Rust. It consumes the [query conformance corpus](../../schemas/workbench/v1/workbench-worktree-query-conformance-v1.json) together with the [JSON Schema](../../schemas/workbench/v1/workbench-worktree-query.schema.json). This binding adds no Desktop command or mutation authority.
