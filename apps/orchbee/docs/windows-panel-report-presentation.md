# Windows Panel report presentation

## Scope and ownership

The native Windows TUI reads terminal Agent reports and Panel metadata for the current Tab's primary Panels. These observations supplement Panel chrome; they do not create canonical Agent sessions, change the Agent board, acknowledge attention, or authorize terminal input. Server report and metadata owners remain authoritative. A child presentation never inherits its primary presentation's report.

The UI owns one read lane with its own Client connection, using the existing Agent read lane cancellation and join infrastructure. No network I/O runs in drawing or keyboard handling, and report failures cannot retire the terminal input connection. One round has a five-second absolute deadline and a one-second cooldown. At most one round is outstanding; scopes are bounded by the Workbench context node limit and encoded rows by 8 MiB.

## Coherent observation model

A scope contains exact Site, Workbench revision and ordered Panel IDs with their primary bindings. Read all metadata batches, then all Agent report batches, then fence both independent revisions with empty reads. Every response must match the scope revision, requested Panels and bindings. Metadata guards resolve against the same round's actual report Agent and authority source. Reject a changed revision, malformed binding, missing row, limit failure or partial round as a whole.

| Event                                        | Result                                                                   |
| -------------------------------------------- | ------------------------------------------------------------------------ |
| New Tab, binding, Site or Workbench revision | Remove prior scope observations immediately; begin a new bounded read    |
| Complete correlated round                    | Publish report and effective metadata together                           |
| Ordinary refresh in flight                   | Keep the preceding complete observation for that exact scope             |
| Failed or expired read                       | Hide prior report-derived text and mark status unavailable               |
| Old epoch or old scope completion            | Discard without affecting the current view                               |
| Retirement, replacement or Drop              | Interrupt, disconnect and join the owned lane; do not abandon the worker |

The renderer checks the current binding again. Read-only observation does not alter selection, focus, layout, controller, history position, terminal geometry or canonical attention. The primary-only boundary matches the existing public Panel read API; child status requires a separately scoped presentation observation.

## Header behavior

Explicit custom Panel titles take precedence over guarded metadata titles, then the existing creation/display label. A status badge uses metadata displayAgent or the actual reported Agent, plus the corresponding guarded state label or localized idle/working/blocked/unknown. Global metadata tokens append as literal key/value text after the title; width limits may hide trailing tokens. Absence of a reported Agent does not invent idle or a canonical session.

The status badge reserves at most half the header width so a long title cannot hide a blocking status. The state word precedes the Agent name; only that word uses semantic status color. The remaining title and token text follows existing display-cell truncation and theme separator glyphs; user strings pass through terminal text sanitization. Loading and unavailable states are localized in English and Chinese. Single and zoomed Panels use the existing outer frame title, while split Panels use their existing leaf headers. The Minimal tier keeps its existing unframed single-Panel display. Existing pointer hit regions and header height are unchanged.

## Verification and delivery

Evidence must cover both revision fences, identity changes, stale epochs, cancellation, missing facts, guarded metadata, full-width/control text, narrow headers and an actual Windows native TUI observing live report changes. Render and process evidence do not authenticate a provider or prove final product parity. Ship a matched cohort; source/cohort revert is rollback. No wire version or durable migration changes. Full regression remains deferred until the complete Windows parity implementation is ready.

When native navigation selects a bound child Presentation, its own label and ordinal are displayed. This primary-only lane deliberately returns the existing localized unavailable state for that child. Its primary report/title/tokens must never be shown as child observations. Exact child report queries remain an integration obligation.
