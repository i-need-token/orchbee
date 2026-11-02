# Shared user preferences

This contract owns common device preferences for the Desktop GUI and native TUI.
Rust `user_preferences` and TypeScript `user-preferences-contract` share the
[JSON schema and conformance corpus](../../../schemas/workbench/README.md#shared-user-preferences).
The native persistence owner is [Local Runtime](../../local-runtime/README.md#shared-user-preference-storage).
Client presentation, transport, and host capabilities do not create additional authorities.

## Inventory and effect

The following leaves form one versioned record. The GUI and TUI expose the same
categories, meanings, choices, bounds, and reset defaults. A client may use a
compact category caption instead of a separate page. Every label ships in English
and Simplified Chinese; the record's machine vocabulary stays in English.

| Category                | Key and control                                                           | Default                                                    | Validation and effect after confirmed publication                                                                                                                                                         |
| ----------------------- | ------------------------------------------------------------------------- | ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Language                | `locale`: Follow system / English / 简体中文                              | `system`                                                   | `system`, `en-US`, `zh-CN`; changes application copy, preserving terminal bytes.                                                                                                                          |
| Application appearance  | `applicationTheme`: Follow system / Light / Dark                          | `system`                                                   | `system`, `light`, `dark`; updates existing chrome. System follows GUI appearance or native host defaults; absent native background observations retain terminal-native contrast.                         |
| Terminal colors         | `terminalDisplayTheme`: Host / Light / Dark / Custom                      | `host`                                                     | Updates existing terminal default-color presentation; explicit program cell colors remain unchanged.                                                                                                      |
| Terminal colors         | `customForeground`: Foreground color                                      | `#141414`                                                  | Lowercase `#RRGGBB`; editable and effective only with Custom.                                                                                                                                             |
| Terminal colors         | `customBackground`: Background color                                      | `#fcfcfc`                                                  | Lowercase `#RRGGBB`; editable and effective only with Custom.                                                                                                                                             |
| Selection and clipboard | `copyOnSelect`: Copy on select                                            | `true`                                                     | Applies to later selection completion, including a gesture already in progress; explicit Copy remains available. Clipboard delivery remains a host capability.                                            |
| History                 | `trimTrailingBlankRowsForAgents`: Trim trailing blank rows for CLI agents | `true`                                                     | Existing detected live Agents only. Primary-screen blank suffixes may reveal retained history; prompts, cursor, styled content, graphics, alternate screens and application mouse tracking are protected. |
| Shell integration       | `agentNotificationMode`: Agent status notifications                       | `never`                                                    | `never`, `hidden`; later eligible hidden-Agent transitions only. Enabling does not replay consumed events.                                                                                                |
| Shell integration       | `completionReminderMode`: Completion reminders                            | `never`                                                    | `never`, `unfocused`, `always`; later fresh command-completion facts only. Requires shell integration.                                                                                                    |
| Shell integration       | `completionReminderMinimumSeconds`: Minimum command duration              | `5`                                                        | Integer 1–3600 seconds, inclusive; applies to later completion evaluation.                                                                                                                                |
| Shell integration       | `soundEnabled`: Notification sound                                        | `false`                                                    | Allows sound with eligible Agent/completion reminders; host sound and notification capabilities still govern delivery.                                                                                    |
| History                 | `scrollbackBytes`: Scrollback limit                                       | `10000000`                                                 | Integer 0–1000000000 bytes, inclusive; applies to newly created terminals. Existing/restored terminals keep their captured limit.                                                                         |
| Launch profiles         | `launchProfiles`: catalog and default                                     | One unnamed `system-default`, system shell, `login: false` | Applies to new default launches. Explicit program requests keep their executable/arguments and still capture the current scrollback limit.                                                                |

Launch profiles contain literal arguments, never a shell command string. Catalogs
allow at most 64 profiles, 128 arguments per profile, 512 UTF-8 bytes per name,
32 KiB per executable/argument, and 1 MiB encoded catalog data. IDs are unique and
the default must exist. Names are trimmed and nonempty; only `system-default`
may have a null name. Custom executable paths must be absolute for the local
platform. Windows rejects login shells. Saving does not prove a program is
installed or that a future Host creation envelope can carry the launch.

Rust catalog validation also projects an optional in-memory profile/field location
from the same checks used for admission, including the literal argument index.
Invalid collection structure (version, counts, identities, default membership or
encoded size) has no field location. This diagnostic does not change serialized
catalogs, request/result shapes or the existing refusal vocabulary. Clients still
own localized correction guidance and local-platform path admission.

Desktop and Worker use the same native automatic-shell resolver. A custom launch
does not depend on automatic shell discovery. System-shell discovery runs for a
new default launch; a native Worker runs it once per generation and reuses the result
for that generation's default launches. Failure refuses the default launch without
inventing another executable or rewriting the catalog. Existing terminals retain their launch.
A native Session reads its store once before it admits each creation and sends the Worker the
effective scrollback, default colors and selected profile command. The Worker never opens the
store, so the Session's frozen configuration identity is the only scope, and a saved change
reaches only later creations.

## Scope and persistence

The configuration identity is frozen at process/Session startup. Explicit
`ORCHBEE_CONFIG_FILE` isolates that absolute configuration path. Otherwise the
native platform configuration location selects the record. Clients using the
same identity share one private record beside that configuration. Different
development identities, devices, or browser origins remain separate. This
contract does not introduce a Workspace preference or a remote write route.

Desktop development and local distributions select their isolated data-root configuration when
no explicit override is present; release uses the native platform location. A TUI validating that
development identity must select the same absolute `ORCHBEE_CONFIG_FILE`. This keeps development
preferences isolated from the installed release while preserving one authority within each scope.

Read returns a complete validated snapshot, exact source token and claimed keys. An absent record returns canonical defaults, no claimed keys and the `missing` source token; an existing record uses a lowercase SHA-256 token of its bytes. Invalid, unreadable, unsafe or oversized records are refused rather than replaced. The complete record/exchange bound is 2 MiB.

An update contains an expected source and a nonempty typed patch. Omitted keys
remain unchanged; null resets a key to its canonical default. An OS-held lock
serializes cooperating writers, and the source is checked under that ownership
again at commit. The full candidate is validated before a flushed atomic
replacement. Post-replacement failure may be `outcome-unknown`; it is never
automatically replayed. A saved snapshot describes that commit, not immunity
from a later edit. Noncooperating external editors are outside the shared lock's
transaction boundary.

Desktop windows and native Clients read or update the current shared record through the Server. Explicit edits and resets claim keys even when the reset value already equals the default. Configuration files select the storage scope without supplying preference values.

## Client behavior and capability boundaries

Editors hold a frozen source from the start of a draft. A conflict keeps the draft
visible and requires a fresh read before another intentional edit. Polling updates
confirmed effective values without rebasing an open draft. Closing a submitted
editor does not cancel an accepted commit. Unknown outcomes require observation,
not a retry. Reset-all includes the launch catalog and uses explicit confirmation.

Persistence and host application are separate facts. Desktop reports when new
terminal defaults are saved but its Host has not accepted them, and exposes an
explicit retry of host application. Display/input/feedback settings update current
clients; they do not restart terminals or rewrite terminal output. Native command
completion uses a localized transient status reminder and optional terminal bell;
Agent reminders use the client notification presentation. Native tail folding
requires an exact acknowledged retained window and falls back to the live grid
when that proof is unavailable. User selection or history navigation takes over
the window; automatic following never moves an active selection.

| Surface                                                                                                     | Classification and owner                                                                                                                                        |
| ----------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Font family, font size/weight, bold weight, ligatures, terminal padding, background opacity                 | GUI host presentation settings. A TUI runs inside a terminal whose font, pixels and compositing it does not own; these are configured in that enclosing host.   |
| Physical macOS Option-key handling                                                                          | Desktop macOS keyboard adapter. A TUI receives already encoded terminal input and cannot change physical Option-key processing.                                 |
| Shell integration status and manual fallback instructions                                                   | Capability/status and installation guidance, not another common preference store. Completion settings remain common even when the capability is unavailable.    |
| Agent integration install/repair, plugins, remote connection diagnostics and Ghostty import                 | Management/diagnostic/import workflows with separate effect ownership. They are not preference switches; imported common values must use the common owner.      |
| Native keybindings, sidebars, window title, terminal/OS notification delivery and per-Agent sound overrides | Native host configuration/management. The common Agent mode and global sound switch remain authoritative over automatic reminder enablement.                    |
| Browser Preview                                                                                             | Browser-origin settings and available browser capabilities. It must not claim synchronization with the local device record without the admitted Desktop bridge. |

## Verification ownership

The schema corpus checks Rust/TypeScript vocabulary, bounds and invalid input.
Local Runtime checks source conflicts, claims, private storage and
commit failures; the matched helper tests the real bounded process exchange.
Desktop and native tests check draft/source ownership, publication, disposal,
launch capture, and existing-terminal presentation. The
[native settings model](../../../apps/orchbee/docs/native-settings.md#correctness-and-evidence)
checks two writers, resets, uncertain receipts and publication.
Physical GUI/TUI acceptance remains required for visible synchronization and
host-specific effects; cross-compilation alone is not runtime acceptance.
