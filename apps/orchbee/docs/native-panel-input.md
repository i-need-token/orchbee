# Native Panel input policy

## Scope and ownership

`panel input (<panel> | --panel <panel> | --current) --right-click orchbee|panel`
sets one canonical Panel's right-button owner. Exactly one selector and the policy are required.
`--current` resolves validated caller Terminal/Worker metadata against one fresh snapshot; it never
guesses a Client. Explicit targets bypass caller metadata. Windows is the runtime acceptance platform.
`panel split --right-click orchbee|panel` commits the selected policy on the new Panel as part of
creation. The source Panel retains its own policy. Omission uses `orchbee`.

The Server owns `PanelNode.Panel.rightClick`; it belongs to the Panel, including when its primary
Terminal is still starting. It is not a Presentation property, controller grant, Host setting or
Client-local preference. The default `orchbee` value is omitted when serialized; an absent field
decodes to that default. Present values must be exactly `orchbee` or `panel`; null is invalid.
Moving or swapping the complete Panel preserves its policy. Closing the Panel removes it.
The state has the same Site-process lifetime as canonical layout; restart persistence is not added.

`orchbee` reserves the right button to the Client. `panel` permits an unmodified right press inside
the content viewport when the program reports mouse tracking and existing input admission allows
delivery. Shift remains Host-owned. This contract establishes routing; Windows
[action menus](native-action-menus.md) own Client-reserved right presses. Configurable modifier
bypass remains outside these surfaces. The existing terminal encoder owns reporting format,
coordinates and the cell-input fallback; see [mouse input](native-mouse-input.md).

## Canonical command model

| Transition | Rule                                                                                                                                                                         |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Resolve    | One snapshot fixes Site, expected revision and exact context/Tab/Panel membership.                                                                                           |
| Set        | Prepare the complete candidate, publication and retained result before committing one successor revision and `panel-input-configured`. Only the target Panel policy changes. |
| Equal      | Return `unchanged` at the current revision; no identity, publication or Host effect.                                                                                         |
| Refuse     | Authorization, stale revision, missing membership, exhaustion and capacity failures preserve all state.                                                                      |
| Replay     | The existing bounded ledger returns the original result for the same semantic request and refuses conflicting identity reuse.                                                |
| Lost reply | The Client retires an ambiguous submission and reports unknown; no automatic retry or compensation.                                                                          |
| Split      | The new Panel policy is committed with its topology and identities, before terminal-start settlement. There is no second policy-setting mutation.                            |

The setter never changes focus, zoom, ratios, attachments, controller ownership or terminal bytes.
Its `orchbee.panel.input` v1 receipt records request/Site/base, exact target, requested policy,
known revision and `configured` or `unchanged`. It acknowledges canonical state, not that every
Client has refreshed or drawn it. Panel reads expose the effective policy. Creation keeps its
existing staged receipt and does not conflate canonical creation with later Client effects.

## Pointer state model

| State   | Transition and invariant                                                                                                                                                                                                                                                         |
| ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Idle    | Resolve the last-drawn hit region against a fresh exact Pane incarnation. A reserved or modified right press opens no terminal gesture and causes no focus or byte effect.                                                                                                       |
| Pending | A permitted press on an unfocused controlling Panel may request local focus. Before the first byte, recheck policy and the existing readiness/identity/deadline guards. A now-reserved right press drops the complete queued gesture.                                            |
| Pane    | After admitting a press, capture its button and exact Pane incarnation. Matching motion and release stay with that owner even if the policy changes. A different button cannot inject a new right press or finish this gesture. Generic legacy release ends the captured button. |
| Cancel  | Existing capture release, modal ownership, focus/incarnation loss, missing frame or non-live viewport rules apply. No tail migrates to another Pane.                                                                                                                             |

A policy change applies to the next gesture; it does not send an unmatched release elsewhere or
synthesize terminal bytes. Wheel handling remains with its existing owner. Client projection
refresh updates policy without replacing retained attachments, frames or geometry. Equal-revision
contradictions remain invalid; greater-revision policy changes do not change member identity.

## Evidence and compatibility

Maintain this reference with the shared Rust/TypeScript/schema Panel and command contracts, pure
Workbench authority, split commit, native carrier/CLI, projection and pointer owners. Concrete
evidence covers strict policy decoding, single-field footprint, no-op/replay/refusal, pending
creation and move/swap preservation, projection identity, button ownership and policy changes
before and after forwarding. Windows command-process and two-TUI witnesses cover caller targeting,
lost replies, shared policy and actual terminal mouse bytes. Help, localization, schema discovery
and all five completion shells stay synchronized. The final root merge gate is required.

The finite policy/gesture model covers admission and captured ownership, not arbitrary terminal
protocol behavior. TLA execution remains deferred at the user's request; model assets and inventory
are maintained without claiming a run. Review is by the same author.

This is an additive development-cohort contract. Older strict readers may reject the nondefault
field or new command, so mixed cohorts are not supported. There is no durable migration. Rollback
uses a matched cohort; never erase policy silently to accommodate an old reader. No new macOS or
Linux runtime is implemented.

Read contracts: `orchbee.panel.list` v2, `orchbee.panel.get/current` v3, `orchbee.tab.get` v2 and `orchbee.api.snapshot` v4; catalog v36 includes 69 schemas. Older versions reject the added field.
