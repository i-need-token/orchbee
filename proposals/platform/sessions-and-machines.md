# Sessions and Machines

| Field             | Value                                                                                                                                                                                                                                                                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Proposed on 2026-11-28 from the product owner's requests of that day. Rows marked Decided restate the owner's decisions; rows marked Chosen are this page's picks, which the owner may veto; [Open decisions](#open-decisions) wait for the owner. Nothing here is implemented until its slice lands                                  |
| Owner             | Maintainers of `apps/orchbee-server`, `apps/orchbee-worker`, `apps/terminal-host` and `crates/local-runtime` for the runtime; of `apps/orchbee` for the CLI, the TUI, the machine hub and the link helper; of `apps/web` and `apps/desktop` for the GUI; `@orchbee/i18n` maintainers for copy                                         |
| Current authority | [Remote machines](../../apps/orchbee/docs/machines.md), the [machine contracts](../../schemas/machine/README.md), the [Distributed Session](native-runtime-topology.md#distributed-session) topology, the [data root layout](data-root-layout.md#tree), and the Session and Machine entries in the [glossary](../../docs/GLOSSARY.md) |
| Exit condition    | Delete this proposal once its [slices](#slices) ship and their rules live in the owning package references, the glossary and the data root layout. [Sharing](#sharing) gets its own proposal                                                                                                                                          |

## Outcomes

- One computer runs several Sessions. Each keeps its own settings, saved machines, Projects and Workspaces, and a person creates and switches Sessions at the top of the sidebar.
- A Session borrows other machines' capacity. Its Server stays on its own computer, and each borrowed machine runs only a Worker and a Terminal Host for it. What the Session places on a machine never appears in that machine's own Sessions, and the Session never sees them.
- Every level of the Workbench, from a Workspace down to a child terminal, has a machine. A person changes it in the GUI or the TUI, and an Agent through the CLI, with the same reach.
- A dropped network ends nothing on a borrowed machine. When the link returns, the Session takes its terminals back and continues.
- Later, another computer, a browser or the mobile app opens a Session on this computer and reaches that Session's machines through it.

## Model

```text
Computer A
└── Session: default, work, …          its settings, saved machines and Server
    └── Project
        └── Workspace @machine         default for what it holds
            └── Tab @machine
                └── Panel @machine
                    ├── main terminal @machine     where it runs
                    └── child terminal @machine    where it runs

Machines B, C, D: one remote node per Session that uses the machine
└── Worker + Terminal Host             no Server, and none of the machine's own Sessions
```

| Term           | zh-CN | Meaning                                                                                                                                    |
| -------------- | ----- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Session        | 会话  | As today, and it owns its settings and its saved machines                                                                                  |
| Machine        | 机器  | A computer a Session borrows capacity from, saved in that Session; `local` is this computer. It no longer names a Session on that computer |
| Remote node    | —     | The Worker and Terminal Host one Session runs on one machine. It is not product copy                                                       |
| Device setting | —     | A setting that belongs to the computer and applies to every Session on it                                                                  |
| Lent           | 借出  | What other computers' Sessions run on this computer, as its own Settings › Machines shows it                                               |

## Today

Verified at `6cb8a5e3c`:

- One `config.toml` at the data root serves every Session. Saved machines live in `internal/state/machines.json`, and one machine hub serves the data root ([tree](data-root-layout.md#tree)).
- A saved Machine is an SSH target plus a Session name on that computer, `default` unless set. Using it attaches to that Session, which that computer's own windows also show, so the two computers' Workspaces mix in both sidebars ([Remote machines](../../apps/orchbee/docs/machines.md)).
- A Workspace has one machine. [Change machine](../../apps/orchbee/docs/machines.md#change-machine) re-creates the Workspace on the other computer and closes the original; a Workspace in a linked worktree cannot change machine.
- Server ↔ Worker admission is local only: OBLW checks same-user peer credentials and an exact cohort ([local Worker protocol](../../crates/local-worker-protocol/README.md)). [Connections](versioning-and-upgrade.md#connections) lists Server ↔ remote Worker as not implemented.
- When the Server goes away, the Worker exits and the Terminal Host keeps every terminal ([process boundary](../../apps/orchbee-worker/README.md#process-boundary)); a replacement Worker adopts that Host ([replacement adoption](../../apps/orchbee-worker/README.md#replacement-adoption)).
- `orchbee session list`, `stop` and `delete` exist. The first use of `--session <name>` creates a Session, nothing renames one, and `default` cannot be deleted. Names are 1–64 ASCII letters, digits, `.`, `_` and `-`.
- Desktop shows one Session. Its notification-area menu counts the terminals on this computer and on every connected machine, and Stop Session… stops that Session ([window lifecycle](../../apps/desktop/docs/window-lifecycle.md#running-in-the-notification-area)).
- A machine without Orchbee reads as `not-installed`. The machine contracts specify an [install channel](../../schemas/machine/README.md#install-channel) that copies a release over the link once the person accepts its offer, but no code implements it.
- The topology's [accepted decisions](native-runtime-topology.md#accepted-decisions-from-product-review) planned a temporary, connection-scoped remote Worker and Host over SSH (decision 20) and a per-user service started at sign-in by `orchbee session enable` (decision 23); neither is built.

## Decisions

| #   | Decision                                                                                                                                                                                                                                    | Kind                                 |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| 1   | Sessions on one computer are isolated, settings and saved machines included                                                                                                                                                                 | Decided                              |
| 2   | Capacity is lent, not shared. A borrowed machine runs no Server for the Session, only a Worker and a Terminal Host, and neither computer sees the other's Sessions                                                                          | Decided                              |
| 3   | The Session switcher sits at the top of the sidebar, and before the Tabs while the sidebar is hidden                                                                                                                                        | Decided                              |
| 4   | A new Session starts from defaults or as a copy of an existing Session                                                                                                                                                                      | Decided                              |
| 5   | Display language, update channel, window placement, notification-area preferences and remembered SSH answers belong to the computer. Plugins are installed per computer and enabled per Session. Every other setting belongs to the Session | Decided                              |
| 6   | The notification-area count and Stop Session… cover only the Session on screen, and Stop asks once                                                                                                                                          | Decided                              |
| 7   | A Session's Server and its remote Workers run the same release, and whichever side is older updates                                                                                                                                         | Decided                              |
| 8   | A level's machine sets the default for nodes added under it later, and changing it moves nothing that runs. Every level down to a terminal can change where it runs, in the GUI, the TUI and the CLI                                        | Decided                              |
| 9   | A Workspace's `@name` follows its name in the sidebar. Tabs and the levels below also show `@name` and offer to change it                                                                                                                   | Decided                              |
| 10  | Orchbee supplies the data for balancing and chooses nothing itself. The machine picker shows each machine's free resources after its name                                                                                                   | Decided                              |
| 11  | The File Bar keeps today's rule: the Workspace's starting directory, on the Workspace's machine. A remote file system should work as a local one does, as in VS Code Remote                                                                 | Decided                              |
| 12  | Workspaces that today live in another computer's `default` Session are not migrated                                                                                                                                                         | Decided                              |
| 13  | Sharing comes later, as a Settings section that is off by default                                                                                                                                                                           | Decided                              |
| 14  | Session names keep today's grammar. An optional display name may use any language, and the switcher shows it                                                                                                                                | Chosen                               |
| 15  | A background Session notifies as usual; the notification names the Session, and the switcher marks it                                                                                                                                       | Chosen                               |
| 16  | A remote node keeps running until its Session stops it or its machine restarts, with no time limit                                                                                                                                          | Chosen                               |
| 17  | A machine's own Settings › Machines lists what other computers run on it, without content, and can reclaim it                                                                                                                               | Chosen                               |
| 18  | Removing a machine that still runs terminals asks first with their count and stops them                                                                                                                                                     | Chosen                               |
| 19  | A node without a machine of its own takes its parent's                                                                                                                                                                                      | Open: [inheritance](#open-decisions) |

## Session isolation

### What a Session owns

- Its Workbench, as today.
- Every setting outside the device list below. Settings edits the Session on screen.
- Its saved machines, with their labels, colors and enabled state.
- Its remote nodes.

### What the computer owns

- Display language, update channel and automatic updates, window placement and notification-area preferences.
- Remembered SSH answers in the operating system's credential store, and OpenSSH's own configuration.
- Installed plugins. Each Session keeps which of them it enables.
- Agent detection data, and the machine hub, which serves every Session and gives each only its own machines.

### Files

Chosen:

- `sessions/<name>/config.toml` holds the Session's settings, and the root `config.toml` keeps only device settings.
- `sessions/<name>/machines.json` replaces `internal/state/machines.json`.
- On the first start with slice 1, `default` takes the Session settings and saved machines that the root files hold, so nobody loses them. Nothing else moves (decision 12).
- The data root layout's [tree](data-root-layout.md#tree) changes with slice 1.

### A new Session

- New Session… asks for a name, an optional display name and a starting point: Defaults, or Copy of an existing Session (decision 4).
- Chosen: a copy takes the settings and the saved machines, not the Projects, Workspaces, terminals or remembered answers, so its first connection to each machine asks once.

## Session management

### Switcher

Decided placement with Chosen details; the owner reviews the visual design before slice 2 is built.

- GUI with the sidebar shown: the switcher is the sidebar's first control, the Session's display name and a chevron. On macOS it starts after the room the traffic lights keep.
- GUI with the sidebar hidden: it follows the sidebar toggle and comes before the Tab Strip or the [title row](../workbench/sub-agents-and-child-terminals.md#title-row).
- TUI with the sidebar shown: a new first row holds the switcher and the hide toggle `[«]`, and New workspace and Search move down one row.
- TUI with the sidebar hidden: the Tab strip leads with `[»]`, then the switcher, then the Tabs ([sidebar toggle](../../apps/orchbee/docs/tui-shell-layout.md#sidebar-toggle-and-help)).
- A long display name truncates. The switcher shows even while `default` is the only Session, since it holds New Session….

### Menu

Chosen:

- The Sessions, by display name. The current one is checked, each reads running or stopped, and a dot marks one whose Agents need the person.
- New Session….
- Per Session: Rename…, Stop Session… and Delete Session…. Delete is offered only for a stopped Session other than `default`.

### Behavior

Chosen, except where marked:

- Switching attaches the window to the chosen Session, and the Session it leaves keeps running. A stopped Session starts, as `--session <name>` starts one today.
- Each window shows one Session, so two windows can show two.
- Rename changes the display name only. The name is the Session's folder and runtime name and never changes after creation.
- Decided: the notification-area count covers the Session on screen, and Stop Session… stops that Session alone, on every machine it uses, after one confirmation with its counts.
- Quit Orchbee closes the windows and leaves every Session running, as today.
- Decision 15 covers background Sessions. Opening such a notification switches the window to its Session.
- Deleting a Session removes its remote nodes. When a machine is unreachable, the confirmation says its terminals keep running there until that machine restarts or reclaims them.

### CLI

Chosen; the grammar follows the [command surface](../../docs/cli/command-surface.md):

- `orchbee session create <name>` with an optional display name and an optional Session to copy.
- `orchbee session list` shows display names, and a command sets a display name.
- `--session <name>` keeps selecting a Session, and its first use still creates one from defaults.

## Borrowed machines

### What runs where

Decided (decision 2):

- The Session's Server, state and settings stay on its own computer. On each machine it uses, the Session runs one remote node: a Worker and a Terminal Host, and the File Bar's `orchbee-files` when a Workspace there needs it.
- The machine's own Sessions never list that node, and the Session never lists the machine's Sessions.

### Link

Chosen:

- The hub's SSH link to the machine's `--machine-link` helper gains a channel for the Server ↔ Worker connection. The SSH sign-in authenticates it, as it authenticates the hub's other channels; OBLW's peer-credential check stays for local Workers.
- The Server opens that channel itself, as a hub client, so its machines stay connected without a window; today only Clients use the hub. The hub keeps SSH sign-in, where decision 20 had a local Worker run SSH.
- A node is named by the Session's computer, through an installation ID its data root creates once, and by the Session's ID. The machine keeps each node's runtime files under `internal/nodes/<installation id>/<session id>/` in its own data root, never under `sessions/`.
- The helper starts the Worker and the Host detached from the SSH connection, as the machine's signed-in user, as its `ensure-session` channel starts a Server today. This replaces decision 20's connection scope: a node outlives a dropped link. It still enrolls nothing, since the machine gains no service and opens no port.

### Terminals on a machine

Chosen:

- The Session's launch settings travel as intent and the machine resolves its own facts, as the [capability node protocol](../../crates/capability-node-protocol/README.md)'s launch capture already allows. A setting that names a program path is kept per platform; without one for the machine's platform, the machine's default applies.
- `orchbee` inside a terminal on the machine reaches the Session, never the machine's own Sessions: the Worker points the terminal at a relay that carries commands to the Server over the link. An Agent there orchestrates the whole Session, not only that machine.
- The machine's Worker serves its terminals' File Bar, Git and Agent observation, as a local Worker does.

### What the machine's person sees

Chosen (decision 17):

- Settings › Machines on the machine gains a Lent section: each computer and Session that runs a node there, its terminal count, and its processor and memory use, without content.
- Reclaim stops that node's terminals after one confirmation, and the Session shows them as stopped by the machine.
- The isolation holds inside Orchbee only. The machine's signed-in user can see and end these processes like any other.

## Network loss

Decided: a dropped link ends nothing on the machine, and the Session resumes when the link returns. Chosen rules:

- The machine's Worker loses its Server and exits. Its Host keeps every terminal running, with screens and scrollback within today's bounds. The Session marks the machine disconnected.
- Each of that machine's terminals shows that its machine is unreachable and refuses typing. Nothing is queued to send later. Terminals on other machines are unaffected.
- A command whose result the Session cannot know reports that its outcome is unknown, and nothing retries it.
- When the hub reconnects, as it does today, the Server has the helper start a replacement Worker, which adopts the Host. Each terminal reattaches with its current screen.
- No time limit applies (decision 16). The Session's computer sleeping, restarting or losing its Server counts as a drop, and the Session reconnects when it runs again.
- A stop or close made while the machine is unreachable is recorded in the Session, and the next connection carries it out before anything else.
- When the machine restarts, its terminals are gone. Once it is reachable, the Session reopens them there, as a restarting Session restores its own, with the [output it kept](../../apps/orchbee-server/docs/workbench-checkpoint.md#terminal-output-replay).
- The node lifecycle, with its drops, adoptions, recorded stops and machine restarts, is distributed state, so slice 3 follows [Modeling and Correctness](../../docs/MODELING-AND-CORRECTNESS.md).

## Versions

Decided (decision 7), with Chosen mechanics:

- The Session's Server and its Workers on every machine run the same release with the same protocol generations. Their executables differ, since the computers may run different platforms. Development and test builds compare their source revision.
- The link's handshake already reports the older side as the Machine's `needs-update` or `local-needs-update` state ([saved machines](versioning-and-upgrade.md#saved-machines)).
- `needs-update`: the Session's computer installs its release on the machine through the [install channel](../../schemas/machine/README.md#install-channel), with the offer the person accepts, and a machine that reads `not-installed` gets its first installation the same way. Slice 3 implements that channel.
- `local-needs-update`: the Session's computer updates itself through its own update path, and the Session moves to the new release with a [restart that keeps terminals](../../apps/orchbee-server/README.md#server-and-worker-restart).
- An update replaces Servers and Workers but no Host, so no terminal stops on either computer, the machine's own Sessions included. A running node keeps its release until its Worker is replaced. A machine that serves computers on two releases makes the older computer update.
- This replaces the negotiated target that [Connections](versioning-and-upgrade.md#connections) gave Server ↔ remote Worker.

## Placement

### Levels

Decided (decision 8):

- The levels are Workspace, Tab, Panel, main terminal and child terminal; the owner's sub-terminal is the [child terminal](../workbench/sub-agents-and-child-terminals.md#scope).
- A level's machine is where nodes added under it go, and changing it moves nothing that runs.
- A terminal's machine is where it runs. Changing it restarts the terminal on the target, since Orchbee moves no process: a new shell in the same folder relative to home, or else in home, after the confirmation Change machine asks today when something runs. Its child terminals keep their machines.
- Chosen: a Workspace in a linked worktree keeps its machine, since its checkout stays with its repository, as Change machine refuses it today. Its Tabs and terminals may still use other machines.

### Inheritance

This page's pick, pending [Open decisions](#open-decisions): a node without a machine of its own takes its parent's. A Tab takes its Workspace's, a Panel its Tab's, a main terminal its Panel's and a child terminal its main terminal's. A new Workspace still takes the machine of the Workspace in focus.

### Display

Decided (decision 9), with Chosen details:

- While the Session has more than one machine, each Workspace row shows `@name` after its name. With this computer alone, no level shows a machine.
- A lower level shows `@name` only where its machine differs from its parent's: after the Tab's title in the Tab Strip and the title row, in the Panel header, and in bottom bar items and the Sidebar's main Agent and sub-Agent rows. It is styled as a machine, never as an Agent type.
- A click on an `@name` opens the machine picker for that level. The Workspace, Tab, Panel, main Agent row and sub-Agent row menus each gain Change machine…, whether or not the level shows `@name`, and the TUI's menus gain the same entries.
- After each machine's name the picker shows its free processor, memory and disk and how many of the Session's terminals run there (decision 10).

### CLI

Chosen:

- Commands that create a Tab, a Panel or a terminal take `--machine <machine>`; without it, inheritance decides.
- `workspace set-machine`, and the same verb for a Tab, a Panel and a terminal, change a level's machine; for a terminal it restarts there. Today's re-creation of a Workspace on the other computer goes.
- `machine list` keeps each machine's free resources and adds the Session's terminal count there, which is what an Agent needs to choose.
- On other commands, `--machine` narrows the command to that machine's nodes. It no longer forwards a command to another computer's Session.

### File Bar

Decided (decision 11): the File Bar shows the Workspace's starting directory on the Workspace's machine. Its `orchbee-files` runs in that machine's remote node, so a remote folder opens, saves and searches as a local one does.

## Computers without a screen

Chosen:

- A Session can start at sign-in without a window, through decision 23's `orchbee session enable` and a matching setting.
- The Server keeps its machines connected, so no window needs to stay open.
- Without a window, only keys and remembered answers sign in. A question that needs a person, such as a new host key or a password that was not remembered, waits for a window, and the machine reads as needing a login until then.

## Sharing

Decided (decision 13), for later:

- A Settings section, Sharing (共享), off by default.
- It may offer a port on this computer that other computers on the local network connect to as Clients of a Session; later, the same view as a local-network web page opened by URL; and the mobile app connecting the same way. These may become one feature.
- A sharing Client keeps no data, and it reaches the Session's machines through this computer.
- Pairing, authentication and encryption stay with [reserved decision 1](native-runtime-topology.md#reserved-decisions-for-review) and Sharing's own proposal.

## Retired

Slice 4 removes:

- A saved Machine's remote Session name (`--machine-session`, `remote_session`), and attaching to a Session on another computer from Settings › Machines.
- The sidebar's per-machine Workspace lists, read-only observers, cached remote surfaces and `internal/state/machine-spaces.json`.
- Forwarding a whole command to another computer's Session with `--machine`.
- Change machine re-creating a Workspace on another computer.
- The glossary's Machine meaning of a target plus a Session.

## Slices

Each slice lands on its own, with the GUI and the TUI together wherever both change ([interface design](../../docs/INTERFACE-DESIGN.md)).

1. **Session settings and machines.** The device and Session split, the per-Session files, `default` taking today's files once, Settings editing the Session on screen, `session create` from defaults or a copy, and the hub giving each Session only its machines.
2. **Session switcher.** Placement, menu, display names, background notifications, and the notification-area count and Stop Session… per Session.
3. **Remote nodes.** The link channel and its admission, the Server as a hub client, node names and runtime files on the machine, the detached lifetime, [network loss](#network-loss), the [version rule](#versions) with remote installation and update, `orchbee` on a machine reaching its Session, the File Bar in the remote node, and the Lent section. Placement stays per Workspace, through today's entries.
4. **Retirement** of everything under [Retired](#retired).
5. **Placement at every level.** A machine per Workbench node with inheritance, Change machine at each level, `@name`, the picker's data, and the CLI's `--machine` and `set-machine`.
6. **Start at sign-in** for computers without a screen.

Sharing follows in its own proposal.

## Open decisions

1. **Where a new node takes its machine from.** The Workspace `api` is on B, and its Tab `tests` was changed to C. With `tests` in focus, the person adds a Tab.
   - Parent, this page's pick: the new Tab goes to B, the Workspace's machine, and C applies only to new Panels and terminals inside `tests`. Changing the Workspace to D then sends every later Tab to D.
   - Sibling: the new Tab goes to C, the machine of the Tab in focus, as a new Workspace takes the focused Workspace's machine. The Workspace's own machine then decides only its first Tab.
2. **A key for switching Sessions.** Neither Client has one. This page adds none until the owner names one, and the action menu offers Switch Session….
3. **Sharing's pairing, authentication and encryption**, with Sharing's proposal.

## Verification

- Every slice runs the [local gate](../../docs/VERIFICATION.md#local-gate).
- Slices 2 and 5: GUI evidence in both themes and both locales, and TUI screens at each [viewport tier](../../apps/orchbee/docs/tui-shell-layout.md#viewport-tiers), for one Session, several, a long display name and a background Session that needs attention, and for `@name` at each level.
- Slice 3, on two computers of different platforms with development or test builds only: a terminal on the machine survives a cut link and reattaches with its screen; a stop made while the machine is unreachable is carried out on reconnect; the machine's restart reopens its terminals with the kept output; each side being older reads as `needs-update` or `local-needs-update` and ends with matching releases; the machine's own `session list` and sidebar show no node.
