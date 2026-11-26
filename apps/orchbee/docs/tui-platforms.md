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
the original absolute deadline and wake. The pure startup workflow and its receipt reasoning are compiled on every platform.

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

| Responsibility                                                                 | Windows                                     | Linux and macOS                                                                          |
| ------------------------------------------------------------------------------ | ------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Inherited terminal admission, raw mode, output, signals and pointer capture    | Retained console adapter                    | Existing POSIX TTY adapter                                                               |
| Workbench protocol, local selection, pane layout, menus and Agent observations | Shared Client implementation                | Shared Client implementation                                                             |
| Settings drafts and confirmed settings                                         | Server settings snapshots and edits         | Server settings snapshots and edits                                                      |
| Keyboard copy-mode navigation, selection and search                            | Shared Client implementation                | Shared Client implementation                                                             |
| Find in terminal                                                               | Shared Client implementation                | Shared Client implementation                                                             |
| System clipboard writes                                                        | Hidden Win32 owner                          | Desktop clipboard program or OSC 52, by [session route](#linux-and-macos-session-routes) |
| Browser link activation                                                        | Windows shell                               | `open` or `xdg-open` in a local desktop session                                          |
| Saved-machine SSH carriers                                                     | Owned OpenSSH processes                     | macOS and Linux: system `/usr/bin/ssh`                                                   |
| Saved-machine catalog storage                                                  | Private Windows cache                       | macOS and Linux: same as Windows                                                         |
| Remembered Workspace and Tab position storage                                  | Private record in the Session folder        | Same as Windows                                                                          |
| Local directory entry                                                          | Existing Windows adapter                    | POSIX grammar, from the launch folder                                                    |
| Worktree create and open                                                       | Existing Windows adapter                    | Same as Windows                                                                          |
| Agent launch and provider probing                                              | Existing Windows adapters                   | Managed zsh and bash, login-shell probe                                                  |
| `config.toml` location, `config default` and the `user-config` check           | Existing Windows adapters                   | Same as Windows                                                                          |
| Plugin registry persistence                                                    | Existing Windows adapters                   | Unsupported                                                                              |
| Host window title                                                              | Console title, original restored on exit    | OSC 0, original restored from the terminal's title stack on exit                         |
| Terminal notification transport                                                | Ghostty/iTerm/WezTerm OSC 9 or Kitty OSC 99 | Same as Windows                                                                          |
| System notification transport                                                  | Notification-area balloon                   | Terminal sequence, `osascript` or `notify-send`, by session route                        |
| Terminal graphics                                                              | Existing Windows adapter and host admission | As Windows, not [Terminal.app](#outer-terminal-queries)                                  |
| Notification cards and ordinary terminal bell                                  | Shared TUI / admitted terminal              | Shared TUI / admitted terminal                                                           |
| Login-shell preference                                                         | Unsupported by the Windows shell adapter    | Existing POSIX shell adapter                                                             |

The optional-capability list does not gate the whole TUI. In particular, an absent saved-machine
adapter does not disable the local Workspace/Agent sidebar or repeatedly retire its observations.
Settings drafts and edits use the Server's settings snapshots on every platform.
Local activation retains the admitted runtime directory on every platform. Remembered-position
storage derives its record path from the data root and the Session the runtime directory is named
after.
[Directory startup](native-directory-startup.md) takes the launch directory from the operating
system rather than from the directory-entry adapter, so an empty Session creates its first
Workspace there on every platform; interactive directory entry and quick creation still need it.

Activation restore feedback is one optional typed fact on every platform, consumed by the same
TUI notice and CLI warning projections. Every platform keeps a checkpoint, so the reader retains
its activation-generation checks and I/O failures, and never fabricates a recovered or empty
checkpoint.

## Outer-terminal queries

Terminal.app (`TERM_PROGRAM=Apple_Terminal`) prints the body of an APC string and the final byte of
a DECRQM request instead of ignoring them (observed with Terminal 2.14 on macOS 14), as Neovim,
yazi and chafa also special-case it. It therefore receives no graphics probe and neither DECRQM
query; it still receives the OSC appearance query. Its graphics settle to the cell raster without
cell pixels, pointer any-motion to unknown and grapheme clustering to withheld, exactly as when
those probes go unanswered. The record does not apply under `TMUX`, `ZELLIJ`, `STY`, `HERDR_ENV`
or `ORCHBEE_TERMINAL_ID`, where the multiplexer or host pane answers for itself. Any other host
that leaves a probe unanswered gets one in-place rewrite of every cell after the last startup
probe settles, so text it printed for the query does not stay on screen.

## Admission and unsupported outcomes

Unsupported optional actions are absent from menus and shortcut discovery. The sidebar omits
New workspace without directory input. Without a launch adapter, Start Agent is absent from menus
and the status line; its binding opens the picker only to show the
localized unsupported explanation. Copy-mode navigation
and search remain usable without a clipboard; the copy button and copy hint are absent. The UI
revalidates capability at invocation, so a stale or explicitly entered operation reports an
`en-US` or `zh-CN` unsupported-platform notice, or the session notice of a
[session route](#linux-and-macos-session-routes), through the ordinary noncapturing notice owner.
Settings receive the login-shell capability as draft input, including presentation and edits.

Adapters independently refuse unsupported effects before opening files, launching processes or
writing clipboard/window state. `io::ErrorKind::Unsupported`, the typed browser error, clipboard
`Outcome::Unsupported`, and registry `PluginRegistryError::Unsupported` preserve a definite
no-effect result. Missing support never becomes success or an unknown commit, never requests a
replay, and never clears a copy selection as if clipboard delivery had succeeded. Existing accepted
plugin ambiguity still cannot fall through to a browser launch.

Terminal notification selection and OSC framing read the terminal capability variables and write
through the lifecycle output in one workflow on every platform. Clipboard jobs share text
selection, cancellation admission and terminal settlement. The Windows hidden-owner pump supplies
the actual write; the Linux and macOS port runs the session's clipboard program on one owned
thread or hands the job back for the lifecycle writer's OSC 52 write after the frame flush, like
the bell, and settles it as unsupported when the session has no route. Cancellation before
dispatch never invokes a delivery port; a begun job settles without replay.

## Linux and macOS session routes

Clipboard, browser, window title and system notification depend on the session as well as the
platform. The Client detects their routes once, at first use, from the terminal capability
variables and the programs present, and admits each effect only when a route exists. An effect
without a route is absent like an unsupported one, and an invoked one reports that it is not
available in this session; the Windows wording and admission do not change.

- A remote login is `SSH_CONNECTION`, `SSH_CLIENT` or `SSH_TTY`. Its desktop belongs to the remote
  machine, so only the outer terminal and an X11 clipboard program over X forwarding reach the
  user's screen.
- A desktop session is macOS outside a remote login, or Linux with `WAYLAND_DISPLAY` (local only)
  or `DISPLAY`. WSL without WSLg has neither.
- macOS uses `/usr/bin/pbcopy`, `/usr/bin/open` and `/usr/bin/osascript`. Linux resolves
  `wl-copy`, `xclip`, `xsel`, `xdg-open` and `notify-send` from absolute `PATH` entries only.

| Effect              | Route, in order                                                                                                                                                                                | Without a route                                                       |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Clipboard           | Local: `pbcopy`, or `wl-copy` under Wayland, else `xclip` then `xsel` under X11; then OSC 52. Remote: OSC 52, then an X11 program over X forwarding                                            | Copy button and hint absent; copy reports the session notice          |
| Browser link        | `open` or `xdg-open` with the URL as its only argument, in a local desktop session                                                                                                             | A link click reports the session notice                               |
| Window title        | OSC 0, after saving the terminal's own title with `CSI 22;0 t`; restored with `CSI 23;0 t` on exit. Not on the Linux virtual console or a dumb terminal                                        | The title notice                                                      |
| System notification | The outer terminal's notification sequence when terminal delivery can frame one, which also reaches a remote login's own desktop; else `osascript` or `notify-send` in a local desktop session | The notification notice; card presentation follows the delivery rules |

Ghostty (`TERM_PROGRAM=ghostty` or `TERM=xterm-ghostty`) keeps no title stack and ignores both
title-stack sequences. After the pop it also gets an empty OSC 0 title, which it treats as a title
never set, so its own title returns; other terminals get only the pop. The empty title does not
apply under `TMUX`, `ZELLIJ`, `STY`, `HERDR_ENV` or `ORCHBEE_TERMINAL_ID`.

OSC 52 is admitted only for an outer terminal that applies an application's clipboard write
under its default settings: Ghostty, kitty, WezTerm, foot, Alacritty or Windows Terminal, named by
`TERM_PROGRAM`, `KITTY_WINDOW_ID`, `ALACRITTY_WINDOW_ID`, `WT_SESSION` or `TERM`, with no
multiplexer or host pane (`TMUX`, `ZELLIJ`, `STY`, `HERDR_ENV`, `ORCHBEE_PANEL_ID`) in between.
iTerm2 leaves clipboard access off by default, Terminal.app and VTE terminals have none, and tmux
ignores an application's OSC 52 under its default `set-clipboard external`, so none of them is
admitted. A copy above 1 MiB on the OSC 52 route fails before any byte is written; a written
sequence settles as delivered, like the bell.

Each program receives an exact argument vector with no shell; the clipboard text goes to its
standard input, and its output is discarded. A clipboard program has three seconds before it is
killed and delivery is unconfirmed, and a notification program five. An opener gets two seconds
for its verdict; one still running then is showing the page, keeps running under a detached
reaping thread, and the link counts as opened. The Client never reads the system clipboard on any
platform: paste is the outer terminal's bracketed paste. Without a Unix plugin store, a Client
link click selects no plugin handler and an `http` or `https` link takes the browser route; the
command-line link selection keeps reporting the missing store.

The shared runtime exposes settings/manifest types, command outcome types, Windows path syntax
and literal command encoding on every platform. A Windows path is parsed with the Windows wire
grammar rather than the host `std::path` grammar; the execution owner supplies frozen environment
and current-directory inputs. This does not add Unix Windows-path filesystem access, plugin
installation or SSH implementations.

## Verification ownership

The Client crate has no macOS-wide `dead_code` exemption. Unreachable implementations and companions
are removed instead of keeping a test-only copy of a retired renderer.

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
