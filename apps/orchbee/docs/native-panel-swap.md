# Native Panel swap

## Invocation and ownership

`panel swap [source] --with <target>` exchanges two leaf Panels in one Tab. The alternative
`panel swap [source] --direction left|right|up|down` resolves one neighbor using a Windows TUI's
unzoomed cell geometry. `--with` and `--direction` are exclusive; one is required. `--panel`
and `--current` are alternate source selectors and conflict with a positional source and each
other. Panel operands use exact IDs or `@panel:0x<hex>`, never labels. Explicit source and `--with` without `--client` use the headless shared path. An omitted source uses
validated caller context, then the selected Client's current Panel. Explicit `--client` or the
existing unique-Client resolver selects the TUI; ambiguous Clients are refused before mutation.

This is an existing-Session Windows workflow. Its default total timeout is 30000 ms; `--timeout`
admits 1 through 3600000 ms. There is no detached mode or caller-supplied workflow request ID.
The CLI does not start a Session, create a terminal, or own durable workflow state.

The Server owns canonical positions and ratios. The TUI owns focus and zoom. The CLI owns the finite
sequence connecting those operations and its receipts. `local_client/workbench/panel_placement.rs`
owns one structural command exchange; the Server's `terminal_panel_placement` authority owns
shared move/swap admission. The existing Client control protocol carries layout reads and focus.
No new TUI mutation mailbox or global selection authority is introduced.

## Canonical exchange

`panel.swap` carries an exact Session, expected revision, request ID, context, Tab and source/target
Panel IDs. It exchanges the complete two leaf nodes at their existing positions in a cloned
candidate. All Split and slot identities, orientation and structure remain exact. Panel and
Presentation identities, creation/custom labels, child Presentations and terminal bindings travel
with the leaves. No identity is allocated, no Host lifecycle effect occurs, and other Tabs and
contexts remain unchanged. Existing shared split ratios therefore retain their values.

Admission uses the navigation grant, retained-command precedence, canonical revision and exact Workspace Context/tree checks. Aggregate Close fences a changed exchange. Candidate encoding, projection admission and ledger capacity precede the one commit. Equal source/target is unchanged without revision or identity allocation. Missing targets, stale revision, denied grants and exhausted capacity preserve the tree. Exact replay returns its original result; conflicting reuse cannot replace it.

The committed result is `panel-swapped`, with the new revision and exact context/Tab/Panel pair.
The carrier checks that action-specific kind, all identities and the one-step revision; a move
receipt cannot acknowledge a swap. A lost or invalid reply retires the carrier as unknown outcome,
without replay or an inverse operation.

Existing native tree admission still refuses child Presentations; this operation does not widen
that supported topology. Exchanging whole nodes never strips metadata to bypass admission.

## CLI workflow

An explicit pair without `--client` resolves the snapshot and exchanges once without any Client
lookup, control or focus. Known commit completes this shared workflow. The following table describes
the Client workflow used by directional/implicit targets or explicit `--client`.

| Phase          | Guard, effect and next result                                                                                                                                                                                                      |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Preflight      | Validate all grammar, arm interruption, admit one existing session, read one canonical snapshot and select one current, ready Client. Freeze its selection and exact source.                                                       |
| Resolve        | An explicit pair uses that snapshot. Direction reads the same Client's unzoomed layout without changing selection. Its canonical revision must equal the snapshot. Pin one exact neighbor; do not resolve it again after a change. |
| No-op          | Same Panel, cross-Tab pair or no neighbor returns an explicit unchanged reason. No canonical command or focus is submitted. A missing or unavailable source is a refusal, not a no-neighbor success.                               |
| Exchange       | Submit exactly one `panel.swap` at the frozen revision. A definite refusal or unknown outcome stops the workflow. A known unchanged result does not focus.                                                                         |
| Observe Client | After a known commit, read the original Client until it is ready at the committed revision. A changed selection, unavailable Client, cancellation or deadline stops here with the commit preserved.                                |
| Focus          | Submit source-Panel focus once using that freshly observed Client view revision. Existing UI admission, interruption, attachment convergence and draw acknowledgment apply.                                                        |
| Finish         | Only a known canonical commit and applied focus are complete success. Preserve canonical and focus facts independently for every other result. Never compensate or retry.                                                          |

All reads and submissions share the original absolute deadline. Polling never renews it. Signals
stop this invocation's waiting and future phases; they do not undo an accepted swap or cancel an
accepted focus command. Output is one complete bounded document, including known effects even
when interruption determines the exit code. Each submitted phase has its own correlation identity.
Read-only layout failure cannot be reported as an unknown canonical mutation.

Focus uses the original Client and the existing Panel focus operation. It does not change another
Client's selection. Same-Tab swap retains local zoom and shared ratios. Selecting an inactive target
uses the existing context/Tab selection lifecycle. Canonical exchange and Client focus are separate
commits: there is no cross-owner atomicity claim. Selection is compared at observations; an ABA
selection change between those observations is not detected. The final focus submission additionally
uses the fresh view revision, and input during accepted control uses existing interruption guards.

## Receipts and recovery

`orchbee.panel.swap` v2 identifies headless exchange with `scope:shared` and null Client/focus/geometry
fields. It preserves the canonical request and outcome, including unknown results, independently.
The Client workflow uses v1, which separates target resolution, canonical submission and focus observation.
It identifies the Session, selected Client, exact pair and submitted request IDs when known. Canonical
not-submitted, refused, unchanged, committed and unknown outcomes remain distinct. Focus is either
null before focus, or a compact record containing operationId, observation and wait. The observation
retains the authoritative focus status (queued/delivered/applied/rejected/incomplete), direct refusal,
not-submitted or unknown state; it does not duplicate the full control command.
Timeout, cancellation or post-commit transport failure cannot erase a known canonical commit.

A nonzero result after commit requires inspecting the receipt and current layout before deciding
what to do next. Retrying the whole command would exchange positions again; it is never automatic.
The existing Client control-status command can observe a submitted focus identity. This workflow
adds no persistent journal or restart recovery guarantee. The schema catalog describes both versions.

## Evidence and compatibility

Herdr reference `e0507237ad8d41f60aece10d00ef29ce2339ca34` supplies the two swap modes, same-Tab
atomic exchange, successful source focus and no-op behavior. Native public identities, Client
routing, error envelopes and process-lifetime canonical state remain native contracts; wire-level
compatibility and Herdr session persistence are not claimed.

Independent tree permutation/involution tests check the complete leaf and fixed topology footprint,
including nested trees, metadata, retained replay and commit refusals. Protocol conformance runs
through Rust, TypeScript and JSON schema. Carrier cases reject cross-action acknowledgments and
preserve unknown outcomes. Receipt-state matrices check terminal error/cancellation cuts and known-effect retention. A carrier
wake after request admission proves cancellation retires an uncertain exchange without replay. Windows process tests
exercise actual parser, output/schema, ordered phases, lost replies, stale layout/selection, refusal,
no-op and total deadline; two real TUIs check nested exchange,
unchanged terminal identities, ratios, zoom and source focus without affecting the other Client.
