# Desktop Terminal Settings

## Ownership and scope

Desktop and TUI use the same device preference authority. The complete inventory, defaults,
validation, reset semantics and effect horizons belong to the
[shared contract](../../../crates/workbench-protocol/docs/user-preferences.md).
The renderer owns drafts and presentation; Main owns capability admission and Host publication;
the matched `orchbee-settings` helper calls Native Session's `UserPreferencesStore`.

Scope follows the frozen native configuration identity, including explicit `ORCHBEE_CONFIG_FILE`,
independently of Electron `userData`, Workspace and remote terminals.
Desktop no longer imports standalone default-settings or launch-profile files into the shared record.

## Startup and recovery

Main verifies the exact helper filename and SHA-256 manifest before each bounded hidden process exchange. Requests contain no path or arbitrary process command. The shared protocol accepts only reads and source-bound updates, without Desktop legacy-record import.

Main reads confirmed preferences before terminal runtime initialization and applies scrollback to
Host. One-second polling observes TUI and other Desktop writers. Invalid or unreadable shared
storage refuses the read, preserves last confirmed runtime values and exposes the issue.
There is no automatic overwrite, rollback or write replay.

## Update transaction

A draft freezes its source on first edit. One compare-and-set patch commits under the shared OS
lock, with a final source comparison. Only confirmed persisted snapshots publish common values.
Untouched keys remain unchanged; null restores a default while claiming that key.

| Result                                 | Durable value | Application behavior                                          |
| -------------------------------------- | ------------- | ------------------------------------------------------------- |
| Invalid candidate or stale source      | Unchanged     | Keep draft and show refusal                                   |
| Write refused before commit            | Unchanged     | Keep previous confirmed snapshot                              |
| Lost reply or uncertain durability     | Unknown       | Reread; never replay                                          |
| Save confirmed, Host application fails | New value     | Show application issue; retry applying the confirmed snapshot |
| Save and application confirmed         | New value     | Publish complete confirmed values                             |

The controller serializes polling and mutation. Host application may retry; the durable write
never retries implicitly. Reopening reads current authority. Clean controls follow external
changes; dirty drafts retain their frozen source and report conflict.

Main publishes to the callbacks captured for that observation, at most once each, and skips
callbacks removed before their turn. Observer failures cannot reject the native exchange.
Disposal clears and closes subscription admission before awaiting active Host application;
late completion may settle its admitted receipt but cannot revive observers or polling.
The [controller tests](../src/main/user-preferences/__tests__/DesktopUserPreferencesController.unit.test.ts)
exercise callback replacement, removal and disposal during application.

## Terminal lifetime

Scrollback admits integer bytes from `0` through `1_000_000_000`, default `10_000_000`.
Main uses `SetTerminalDefaults` and new Host terminals capture that value. It never sends
`UpdateTerminalConfiguration`, evicts existing rows or recreates running terminals.
The native Session captures scrollback and the selected launch profile from one confirmed
snapshot before it admits each creation; its Worker resolves only device facts.
Restored Desktop bindings retain their concrete launch and existing Host configuration.

## Renderer capability

The eleven-port bootstrap ends with clipboard read and shared user preferences; terminal defaults
retain their narrow compatibility port. The shared port carries bounded correlated requests,
confirmed snapshots and application issues, without filesystem or Host access.
Malformed data or transferred ports retire the capability. Document replacement disposes brokers
and listeners while the Main controller survives. Browser Preview retains independent origin
storage and omits unavailable native launch and scrollback controls.

## Verification

Controller, broker, preload, renderer and lifecycle tests cover admission, conflict, disposal,
strict current-shape admission, failure retention and new-terminal capture. Runtime and real helper process tests
cover competing writers and shared storage. Model and negative controls are described in the
[native contract](../../orchbee/docs/native-settings.md#correctness-and-evidence).
Run the affected root gate. Native process evidence applies only to the host where it ran.
