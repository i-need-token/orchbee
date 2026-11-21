# Custom commands

Custom commands run on Windows and macOS. Their configuration, catalog, invocation and receipts are
shared; [Platform command processors](#platform-command-processors) names what differs. A Linux
Session keeps refusing them at admission, and the CLI answers `unsupported-platform` there.

## Configuration and catalog model

The shared settings owner admits up to 64 `[[keys.command]]` entries. Each has a nonblank NUL-free `command` up to 4096 UTF-8 bytes, `type` (`shell`, `panel`, `popup` or `plugin-action`, default `shell`), optional `key` (one chord or up to 16 chords) and optional `description` up to 512 bytes. Popup-only `width` and `height` accept positive cell counts up to 4096 or percentages 1%-100%. Missing keys produce an unbound catalog entry. If any configured command has an unknown field, duplicate/unsafe/reserved chord, invalid command, or action-incompatible size, the entire configured-command set is disabled and a bounded warning is emitted. No partial or unvalidated command can run; other valid settings and built-in actions remain admitted. The whole settings file/profile remains bounded at 64 KiB.

Custom and built-in keys share normalization, the effective prefix and conflict policy. Runtime publication uses the existing canonical keys profile; configuration reload installs the complete admitted command/binding set with its diagnostics, and a later corrected publication restores configured commands. Fatal file/TOML failure retains the preceding publication. Catalog IDs bind the complete normalized profile and ordered entry index. They are opaque, change when any command/binding definition changes, and never accept executable text or grant process authority. A current command ID resolves one declared action; stale or absent IDs fail before its effect. Each catalog entry also carries `display`, its command line or a plugin action's target, and `description` when one is configured. The Server is the one owner that makes this text safe to show: both pass the chrome rule, which removes control characters, separators, reordering format scalars and whole escape sequences and nothing else. The Server cuts `display` at 512 UTF-8 bytes with an ellipsis, and sends either field as none when nothing visible remains; neither is executable input. Every Client names a command by `description`, else `display`, as sent.

## Invocation ownership model

Native command discovery, CLI invocation and TUI bindings consume the same installed configuration. The GUI binds a command's direct chords that a key can type and leaves prefix chords to the TUI, as its [Workbench](../../web/docs/workbench.md) describes. Invocation validates requested Workspace/Tab/Panel ancestry and the current source member, then uses the existing observed focus path when a target is specified. Plugin selections are read against their exact terminal content revision; unrelated or stale selection is refused. CLI selection coordinates and the Server source observer use one pure text-row extractor in the terminal protocol binding, including wide glyphs, wrapped rows and the plugin context byte limit. The temporary observer owns detach settlement before invocation can proceed. Shell and plugin actions may have no Panel context; panel and popup actions require an admitted current source.

Shell runs the platform's command processor through a Worker-owned process, without terminal input. Panel execution uses ordinary canonical terminal creation and focus. Installing the split steps the invoking Client's view revision, so `command invoke` focuses the new Panel only after it observes that Client at or beyond the split's commit, with only the added Panel changing its selection, and at the view revision it observed, as the creation [Observe and Focus phases](native-terminal-creation.md#workflow-and-guards) do. A definite `stale-view` refusal applied nothing, so the Client is observed again and the focus submitted under a fresh operation identity, at most three more times. A changed selection or an unready Client leaves the Panel created and unfocused, and the exit status names that failure. Popup uses the independent typed terminal lane and its input/resize/close owner. Shell and panel commands start in the source terminal's reported directory, otherwise the Workspace's directory, otherwise the account's home directory; the Server's own working directory never takes part. Plugin action invocation uses the existing action resolver and logs. None impersonates an installed plugin. Source context is display metadata, not launch authority; managed Session routing and actual Worker placement remain authoritative. A submitted effect is not replayed after timeout, cancellation, lost or malformed answers. Waiting cancellation never undoes an accepted effect.

Worker waits up to five seconds for physical shell-start confirmation, within the carrier's
twenty-second response budget. Slow process creation inside that window can still produce a
confirmed receipt. A missing or uncertain confirmation retains the process owner and reports
physical uncertainty; a late result never authorizes replay or silently clears that uncertainty.

The TUI command exchange has a separate cancellable logical lane and joined domain thread on the shared carrier. It does not own physical admission, reconnect or a heartbeat timer; UI processing and popup owners remain independent. Only one command may be queued or awaiting its receipt. Empty Workspaces may invoke global shell/plugin actions; panel and popup actions still require the exact bound source. Shortcut discovery and its search name a command by the Server's catalog text, never by the command line it parsed itself: after each applied key profile with configured commands, the TUI's configuration watcher reads the catalog once, and an entry names only the installed command with its exact ID, so a catalog read against another profile names nothing. Until the catalog answers, or if the read fails, the row keeps its localized generic title. The TUI lays the name out by display width and adds no sanitizing of its own beyond the paint boundary every chrome string passes. Nested receipts must describe the declared action and match its request, Session and popup selector; an unrelated valid protocol result is refused.

A shortcut queues its opaque installed command ID and exact Ready source observation, then publishes
that observation through the normal Client view-publication lane before binding the invocation to the
acknowledged Client/view revision. An older in-flight Busy observation from the armed prefix cannot
reject or authorize that invocation. A newer or different source refuses without retargeting. This
same path applies to initial and reloaded bindings and needs no prior CLI catalog or invocation.
Server-side command, source and permission checks remain unchanged. Refusal feedback includes the
command ID and distinguishes busy, stale source, exited source, changed command/configuration,
permission, unavailability and unknown effects, with the corresponding wait, return-live,
focus-a-running-Panel, reload, reconnect or inspect-before-retry guidance.

A source Panel whose terminal has exited or failed is refused with `source-exited` before any
effect, for every action: its retained output is not a source, and nothing runs. The Server names
that cause only when its source read finds no frame and the Host's inventory says the terminal
ended; a source whose state it cannot learn stays `unavailable`, and a running source is never
refused for it. The Server names the cause only to a Client whose handshake answer lists
`configured-command-source-exited`, which every Client of this release offers; an older Client,
which cannot read the reason, gets `unavailable`. The CLI exits 3 and points to `--panel` with a
running Panel; the TUI and the GUI say the Panel's terminal exited and to focus a running Panel.

## Platform command processors

The command text is one literal argument to the platform's command processor, so it uses that
processor's own syntax. A Panel command shows its output in a split that stays after the command
exits, and a popup shows it in an independent terminal; a `shell` command has no terminal and its
output is discarded. When a `shell` command's processor exits, or the Session stops, everything
else the command started is stopped too, within the Worker's five-second cleanup budget.

Until a Panel command sets a title, its Panel is named by the program its command line starts, as
a terminal names the process it shows: `npm` for `npm test`, on both platforms. A line whose first
word is quoted or expanded, an assignment, an `@` that hides cmd's echo, or a builtin that starts no
program of its name, such as `cd` or `exec` in a POSIX shell and `cd`, `call` or `start` in cmd, and
a compound command are named by the processor that runs the line: the login shell on macOS,
Command Prompt on Windows. Neither platform names the launch adapter around the processor.

### Windows

`shell` runs `ComSpec`, else the system `cmd.exe`, as `/d /c <command>`, so its AutoRun commands do
not run. Panel and popup commands run the same `/d /c` command from an encoded Windows PowerShell
launch, which sets the command's environment, waits for it and exits with its code. `cmd /c` sets
no console title, so the Server publishes the program the line starts, or `ComSpec` for a line
named by its processor, as the Panel's launch program instead of that adapter. A `shell` command's
Job holds every process it starts.

### macOS

Every command runs in the person's login shell, the shell a new terminal starts: `$SHELL` when it
names an absolute executable, else the account's shell, else `/bin/sh`. It runs as
`<shell> -l -c <command>`, as VS Code runs a shell task, so the login files (`/etc/zprofile`,
`~/.zprofile`, `~/.bash_profile`) set `PATH` as in a new terminal, while interactive files such as
`~/.zshrc` do not run. csh and tcsh accept `-l` only alone, so they run `-c <command>` and read
their `.cshrc` or `.tcshrc` as every csh does. Panel and popup commands start
`/usr/bin/env NAME=value… <shell> -l -c <command>`, with the same exception for csh and tcsh, so
environment values, such as the source terminal's folder, arrive as written and never as shell
text; a shell path that holds `=` is refused as unavailable.

A `shell` command leads its own process group, and its owner thread unblocks every signal before it
starts, so the command and its children can be stopped. On macOS it starts through the native
adapter, so no descriptor reaches it except its three standard streams, which are the null device.
When the shell exits, the members left in its group get `SIGKILL`. When the Session stops first, the
group gets `SIGTERM`, then `SIGKILL` once the shell has exited or two seconds have passed. Either
way the command counts as ended only once no member is left. A program that leaves the group, such as one that calls `setsid` to become a
daemon, is not stopped. `plugin-action` commands are refused with `source-unavailable` before any
effect, because plugin stores exist only on Windows.

## Reference and evidence

Custom commands support four configured actions, endpoint-issued IDs, optional target focus and
revision-bound plugin selection. Native configuration profiles supply the command identity boundary;
existing runtime owners supply process and physical terminal authority.

Native command output is `orchbee.command` v1; stale IDs use the closed
`orchbee.cli.failure.command` v1 document. Configured popup controls use v1 and remain invalid under
v1.
