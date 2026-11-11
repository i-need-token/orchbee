# TUI platform boundaries

## Ownership and shared behavior

The native Client uses one TUI implementation on Windows, Linux and macOS. `workbench_client`
owns shared Workbench state, command admission and view projections. Input dispatch, navigation,
action menus, settings drafts, Agent reads, retained terminal content, copy-mode search, pane
layout and rendering compile on every supported platform. A Server still authorizes each request;
compilation never asserts that a Server or Host implements a requested effect.

`tui_capabilities` owns the immutable host capability snapshot used by the composition layer.
The snapshot enters the Client view and native UI explicitly. Pure state and protocol modules
have no operating-system gates and perform no environment, filesystem or process probes.
The old Unix-only compact renderer and context-click route are retired; every platform uses the
same completed-frame hit geometry, release ownership and shared interaction rules.

`agent_runtime` owns the shared Agent process interpretation, observation facts, send outcomes,
request identity and cancellation wait. Its `startup/{snapshot,observation,processes,send,binding}`
modules perform the typed Snapshot and OBAP reads, exact binding resolution, attached Host process
reads with viewer cleanup, and single terminal submission consumed by native startup and CLI
adapters. Callers retain discovery, selectors, presentation and signal ownership; operations use
the original absolute deadline and wake. The OS startup adapter only admits Agent launch:
Windows accepts, while Unix returns typed `Unsupported` before connection, reservation, viewer
or input admission. The pure startup workflow and its receipt reasoning remain compiled.

`local_client::cancellation` privately owns one-shot cancellation for observation jobs, event
watches, saved-machine observers and selection attempts. Creation finishes before thread spawn;
creation failure follows the caller's existing unavailable result. The owner publishes `0 -> 1`
before notifying and remains alive through cancellation and join. Worker completion stays in the
join/result channel. Consumers borrow the readiness source without draining or resetting it.
Collectors cancel every active and retired job before joining any of them.

The Unix adapter writes one sticky byte to a real nonblocking pipe and keeps native FD polling.
An already-full pipe remains ready; interrupted writes have a bounded retry budget. A terminal
write failure or poisoned notification lock closes the sole writer so blocked readers receive a
wake fault and can settle before join. The Windows adapter retains exact expected-byte matching;
the CLI's independent `0/1/2` signal lifecycle is unchanged. Borrowed CLI signal descriptors and
existing copy/link lane wakes pass through unchanged. Readiness inspection is a nonconsuming
zero-time poll: readable means cancellation, only HUP/error/invalid means a wake fault, and no
readiness means pending. Blocking transport waits use the native wake directly; no periodic
Unix cancellation poll or new transport thread substitutes for it. Existing observer cooldowns
and SSH-guard retirement scheduling keep their original cadence.

## Current adapters

| Responsibility                                                                       | Windows                                      | Linux and macOS                     |
| ------------------------------------------------------------------------------------ | -------------------------------------------- | ----------------------------------- |
| Inherited terminal admission, raw mode, output, signals and pointer capture          | Retained console adapter                     | Existing POSIX TTY adapter          |
| Workbench protocol, local selection, pane layout, menus, help and Agent observations | Shared Client implementation                 | Shared Client implementation        |
| Settings drafts and confirmed settings                                               | Server settings snapshots and edits          | Server settings snapshots and edits |
| Keyboard copy-mode navigation, selection and search                                  | Shared Client implementation                 | Shared Client implementation        |
| System clipboard writes                                                              | Hidden Win32 owner                           | Unsupported                         |
| Browser link activation and saved-machine SSH carriers                               | Windows shell and owned OpenSSH processes    | Unsupported                         |
| Saved-machine catalog, remembered position and first-run dismissal storage           | Private Windows cache                        | Unsupported                         |
| Local directory entry and Agent launch/provider probing                              | Existing Windows adapters                    | Unsupported                         |
| Legacy TOML configuration and plugin registry persistence                            | Existing Windows adapters                    | Unsupported                         |
| Host window title, terminal/system notification transport and terminal graphics      | Existing Windows adapters and host admission | Unsupported                         |
| Notification cards and ordinary terminal bell                                        | Shared TUI / admitted terminal               | Shared TUI / admitted terminal      |
| Login-shell preference                                                               | Unsupported by the Windows shell adapter     | Existing POSIX shell adapter        |

The optional-capability list does not gate the whole TUI. In particular, an absent saved-machine
adapter does not disable the local Workspace/Agent sidebar or repeatedly retire its observations.
A local Client on Unix does not read the Windows catalog or invent an empty persisted catalog.
Settings drafts and edits use the Server's settings snapshots on every platform.
Local activation retains the admitted runtime directory on every platform. Remembered-position
storage derives its cache path only inside the Windows adapter; the Unix adapter returns the
typed `PositionStorage` refusal without reading Windows environment variables or inventing a path.

Activation restore feedback is one optional typed fact on every platform, consumed by the same
TUI notice and CLI warning projections. The Windows reader retains its activation-generation
checks and I/O failures. The Unix reader reports an unsupported capability without probing storage;
activation continues with no restore notice. Capability absence never hides a supported reader's
I/O failure or fabricates a recovered or empty checkpoint.

## Admission and unsupported outcomes

Unsupported optional actions are absent from menus and shortcut discovery. The sidebar omits
New workspace without directory input. An empty Agent board offers Refresh and the localized
unsupported explanation when no launch adapter exists, without presenting a Start form. Copy-mode navigation
and search remain usable without a clipboard; the copy button and copy hint are absent. The UI
revalidates capability at invocation, so a stale or explicitly entered operation reports an
`en-US` or `zh-CN` unsupported-platform notice through the ordinary noncapturing notice owner.
Settings receive the login-shell capability as draft input, including presentation and edits.

Adapters independently refuse unsupported effects before opening files, launching processes or
writing clipboard/window state. `io::ErrorKind::Unsupported`, the typed browser error, clipboard
`Outcome::Unsupported`, and registry `PluginRegistryError::Unsupported` preserve a definite
no-effect result. Missing support never becomes success or an unknown commit, never requests a
replay, and never clears a copy selection as if clipboard delivery had succeeded. Existing accepted
plugin ambiguity still cannot fall through to a browser launch.

Terminal notification selection and OSC framing consume frozen host facts through one shared
workflow. Its OS port owns environment capture and output; Unix refuses before either access.
Clipboard jobs likewise share text selection, cancellation admission and terminal settlement.
The Windows hidden-owner pump supplies the actual write, while the Unix port supplies the typed
unsupported outcome without a worker or clipboard effect. Cancellation before dispatch never
invokes either delivery port; a begun job settles without replay.

The shared runtime exposes settings/manifest types, command outcome types, Windows path syntax
and literal command encoding on every platform. A Windows path is parsed with the Windows wire
grammar rather than the host `std::path` grammar; the execution owner supplies frozen environment
and current-directory inputs. This does not add Unix Windows-path filesystem access, named-Session
registration, plugin installation, SSH, clipboard or browser implementations.

## Verification ownership

Pure state, rendering, input, capability refusal and receipt-settlement traces run on every
supported platform. Real Win32, process and private-cache witnesses stay with their Windows
adapter. The Client crate has no macOS-wide `dead_code` exemption. Unreachable implementations and
companions are removed instead of keeping a test-only copy of a retired renderer.
The Linux startup and peer-publication process witnesses share one bounded Workbench peer helper;
fixture preparation does not import another scenario's test module.
Cancellation evidence retains the existing in-flight read/no-retry and joined-retirement witnesses,
and adds the readiness gap: sticky independent borrowers, borrowed wake faults, failed notification
closure and exact Windows signal-byte matching. Unix FD witnesses still require a native Unix run.

Run `cargo check -p orchbee --all-targets --all-features --locked` and
`cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings` on each native
platform, with `CARGO_BUILD_JOBS=6`. The owning repository gate remains
[Verification](../../../docs/VERIFICATION.md): Windows uses `pnpm verify:rust-native`; Linux and
macOS use the applicable host gate. A foreign-target compile is compile evidence only.

When a native runner is unavailable, retain the exact revision, unexecuted commands and required
manual handoff. Native host evidence records OS, emulator/version, geometry, locale, theme and
capabilities while exercising shared navigation, menus, resizing and copy-mode search, as well as
localized refusals of missing adapters. No remote connection failure or Windows check establishes
macOS or Linux success. [TUI verification](../../../docs/tui/verification.md) owns the evidence classes.
