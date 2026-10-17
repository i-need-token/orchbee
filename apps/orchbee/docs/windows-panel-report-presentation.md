# Windows Panel report presentation

## Scope and ownership

The native Windows TUI reads terminal Agent reports and Panel metadata for the current Tab's selected Presentation in each Panel. These observations supplement Panel chrome; they do not create canonical Agent sessions, change the Agent board, acknowledge attention, or authorize terminal input. Server report and metadata owners remain authoritative. A child presentation never inherits its primary presentation's report.

The UI owns one read lane with its own Client connection, using the existing Agent read lane cancellation and join infrastructure. No network I/O runs in drawing or keyboard handling, and report failures cannot retire the terminal input connection. One round has a five-second absolute deadline and a one-second cooldown. At most one round is outstanding; scopes are bounded by the Workbench context node limit and encoded rows by 8 MiB.

## Coherent observation model

A scope contains exact Session, Workbench revision and ordered Panel IDs with their selected Presentation IDs and exact bindings. Read all metadata batches, then all Agent report batches, then fence both independent revisions with empty reads. Every response must match the scope revision, requested Presentations, owning Panels and bindings. Metadata guards resolve against the same round's actual report Agent and authority source. Reject a changed revision, malformed binding, missing row, limit failure or partial round as a whole.

| Event                                                                  | Result                                                                   |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| New Tab, selected Presentation, binding, Session or Workbench revision | Remove prior scope observations immediately; begin a new bounded read    |
| Complete correlated round                                              | Publish report and effective metadata together                           |
| Ordinary refresh in flight                                             | Keep the preceding complete observation for that exact scope             |
| Failed or expired read                                                 | Hide prior report-derived text and mark status unavailable               |
| Old epoch or old scope completion                                      | Discard without affecting the current view                               |
| Retirement, replacement or Drop                                        | Interrupt, disconnect and join the owned lane; do not abandon the worker |

The renderer checks the current Presentation ID and binding again. Read-only observation does not alter selection, focus, layout, controller, history position, terminal geometry or canonical attention. Both carriers use `read-presentations` in batches of four unique IDs, including empty revision fences. A `presentations-read` result preserves requested order and carries each Presentation ID with its owning Panel row and exact binding. Two members may share one Panel ID; membership is qualified by Presentation ID. Unknown IDs reject the whole batch; an unbound member returns empty metadata and a null Agent report. Existing primary Panel reads and report writes retain their semantics.

## Header behavior

Explicit custom Panel titles take precedence over guarded metadata titles, then the existing creation/display label. A status badge uses metadata displayAgent or the actual reported Agent, plus the corresponding guarded state label or localized idle/working/blocked/done/error/unknown. Global metadata tokens append as literal key/value text after the title; width limits may hide trailing tokens. Absence of a reported Agent does not invent idle or a canonical session.

Split Panels paint these observations in their existing leaf headers. Single and zoomed Panels have no title/status subheader or top-right Workspace/Agent metadata; their terminal content begins directly below Tabs. Report reads and exact-scope invalidation remain available to the owning observation model. The member selector uses the existing top strip for single/zoomed Panels and reserves leaf-header width for split Panels; its pointer regions remain separate from Tabs, Actions and terminal content. The Minimal tier keeps its unframed single-Panel display and keyboard selection route.

In split headers the status badge reserves at most half the header width so a long title cannot hide a blocking status. A fixed leading symbol and the state word use the same semantic role: working `◐` purple, blocked `!` amber, explicit done `✓` green, error `×` red, idle `○` neutral, unknown `?` neutral. The state word precedes the Agent name. An independent trailing blue solid dot reflects the existing Client unread observation; clearing it never changes the report state. Both symbols remain distinct on selected backgrounds, and ASCII hosts use `~`, `!`, `+`, `x`, `o`, `?` with text. The remaining title and token text follows existing display-cell truncation and theme separator glyphs; user strings pass through terminal text sanitization. Loading and unavailable states are localized in English and Chinese.

## Verification and delivery

Evidence must cover both revision fences, identity changes, stale epochs, cancellation, missing facts, guarded metadata, full-width/control text, narrow headers and an actual Windows native TUI observing live report changes. Render and process evidence do not authenticate a provider or prove final product parity. Ship a matched cohort; source/cohort revert is rollback. The six-state vocabulary changes the matched cohort and uses the versioned public CLI documents listed in [Native CLI Query](native-cli-query.md#schema-versions-for-independent-agent-status); it requires no durable migration. Complete the affected host gate and real-window status/read combinations before delivery.

Selecting a child immediately invalidates the preceding primary observation. Split leaf headers retain its own label and ordinal while the new exact scope loads; single/zoomed views retain the member counter without restoring a metadata header. Only a complete correlated round can supply its report, guarded title and tokens; switching during a read cannot publish the old member’s result. Child report writes remain a separate integration; selector arrows use the existing exact member selection owner.
