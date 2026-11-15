# Automatic Updates

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Proposed on 2026-11-15 from the product owner's request of that day. Decided restates a product-owner decision. Announced marks a pick the owner was told about and may still veto, and Chosen marks this page's own pick, which the owner may also veto. Rule marks a safety or trust rule that follows from an accepted outcome.                                                                                                                                                                                                                                                 |
| Owner             | Maintainers of `apps/orchbee`, `apps/orchbee-server`, `apps/desktop`, `apps/web`, `crates/local-runtime`, the release scripts under `scripts/`, and the release workflow                                                                                                                                                                                                                                                                                                                                                                                                           |
| Current authority | [Windows release installation](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation), the [Windows installer](../../apps/desktop/README.md#windows-installer), [running in the notification area](../../apps/desktop/docs/window-lifecycle.md#running-in-the-notification-area), the [TUI Settings row](../../apps/orchbee/docs/tui-shell-layout.md#sidebar-toggle-and-help), [native settings](../../apps/orchbee/docs/native-settings.md), [release distribution](release-distribution.md), and [versioning and upgrade](versioning-and-upgrade.md) |
| Exit condition    | Delete this proposal once slice 1 and P1 ship and each rule lives in its owning package reference or standard.                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |

Orchbee releases often. This page decides how a release reaches a running installation with no
step from the person but one click, and without stopping anything they run. It builds on the
signed [release feed](release-distribution.md#release-feed), the
[runtime store](release-distribution.md#runtime-store), and the
[restart decision](versioning-and-upgrade.md#restart-decision).
[Host adoption](versioning-and-upgrade.md#host-adoption) is the one mechanism that later moves a
running Session.

## Outcomes

1. **Non-stop.** Decided 2026-11-15, above every other outcome. An update never stops, restarts, or
   interrupts what runs on the person's computer: terminals and their programs, Agents, and the
   background Session that holds them. Every automatic path keeps them running. Anything that would
   end or restart them is at most an explicit choice the person confirms after being told what
   stops. When another goal conflicts with this one, this one wins.
2. **Latest by default.** With automatic updates on, a release reaches every running installation
   within about an hour of its channel moving and installs in the background.
3. **One click.** The Update control switches the window or TUI it sits in to the installed version
   at once, on the same Session and at the same place. Terminals and Agents keep running.
4. **The Session follows.** The background Session moves to the new version by itself once no
   terminal runs. After P1 the same click also moves it, and terminals continue.
5. **Off means off.** With automatic updates off, nothing is checked or downloaded on a schedule;
   `orchbee update` keeps working.

The owner's reason: releases come often so that fixes reach people quickly, and that works only if
updating never costs them their running work.

### What still breaks the non-stop rule

| Behavior today                                                                                                                                              | What the person loses                                                                                   | Removed by                                                                 |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| After a Worker loss the Host keeps every program, but nothing can show, reach, or stop them; Quit All and `orchbee server stop` refuse while the Host lives | Every terminal, until the Host is ended from outside or the computer restarts, which ends every program | P1a: the Server replaces the Worker, and a confirmed stop really stops     |
| A Server or Worker change takes effect only after a Session stop, which ends every terminal; the `initiator-too-new` message asks for `orchbee server stop` | The terminals, or the fix                                                                               | Slice 1 for an idle Session; P1b while terminals run                       |
| A Host change reaches a terminal only after its Host empties; a Host below its security floor changes only by restarting its terminals after a confirmation | Nothing until a security floor is set; then the listed terminals                                        | P1c for new terminals; P3 and P4 for the confirmation                      |
| Setup refuses while terminals started by an older window run from files under `desktop\`, and asks the person to close them                                 | Those terminals, if the person follows the advice                                                       | Slice 1: Setup and `update` add window versions beside running ones        |
| Before the first public release, Session records change format without migration                                                                            | The layout at an idle move; adoption at a restart                                                       | P1b restarts only when the new version reads the running Session's records |

A computer restart still ends every program, as
[Host lifetime](versioning-and-upgrade.md#host-lifetime) says; it is not an update path.

## Comparable products

| Product                                      | Checks                                           | Installs                                                          | The person sees                                    | Running work at the switch                         |
| -------------------------------------------- | ------------------------------------------------ | ----------------------------------------------------------------- | -------------------------------------------------- | -------------------------------------------------- |
| VS Code                                      | 30 seconds after start, then hourly              | In the background on Windows                                      | A badge on the Manage gear, then Restart to Update | Terminal processes end; their text may be restored |
| Zed                                          | Hourly                                           | In the background                                                 | Restart to update in the title bar                 | Terminals end                                      |
| Chrome                                       | Its own updater                                  | In the background                                                 | An Update button in the toolbar                    | Tabs reopen; page state is lost                    |
| Squirrel.Windows apps such as GitHub Desktop | At start                                         | Into a new version directory beside a stub that starts the newest | A restart prompt                                   | The application restarts                           |
| Orchbee                                      | One minute after the Session starts, then hourly | In the background, beside the running version                     | Update to the right of Settings                    | Terminals and Agents keep running                  |

Each of them ends running work when it applies an update. Orchbee can keep it because the Terminal
Host, which owns every program, takes part in no switch.

## Decision record

| #   | Decision                                                                                                                                                | Reason                                                                                                           | Status                                            |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| 1   | The Session Server checks one minute after it starts and then hourly, through a background helper run of the store's active `orchbee`                   | One Server per Session exists for TUI, CLI, and window users alike; Clients come and go, and several may be open | Announced                                         |
| 2   | The helper downloads, verifies, and installs beside the running version and never touches a running file                                                | The store already installs only by adding directories                                                            | Announced                                         |
| 3   | The window application updates too, through versioned window directories behind an unchanged launcher path                                              | A switch becomes an ordinary relaunch; nothing running is replaced; shortcut, identity, and registration stay    | Announced that it updates; Chosen how             |
| 4   | An Update control sits to the right of Settings in the TUI and in the window                                                                            | The owner's request                                                                                              | Announced                                         |
| 5   | A click switches only the Client it is in                                                                                                               | Relaunching a Client the person is not looking at would interrupt their typing there                             | Chosen                                            |
| 6   | The TUI relaunches in the same console through its launcher; without one it leaves with a one-line hint                                                 | Windows has no process replacement; the launcher already owns the console and the exit code                      | Chosen                                            |
| 7   | Before P1b the Session moves by itself only when no terminal runs and no Client is connected; P1b drops the Client condition and lets the click move it | Nothing reconnects on its own yet, so ending a Server under a connected Client strands it                        | Decided by the owner; the Client condition Chosen |
| 8   | Failed checks and installs show nothing and retry on the next hour; only a click that cannot switch says so                                             | The person cannot act on a network failure; `orchbee update` and diagnostics give details                        | Chosen                                            |
| 9   | A Client offers Update whenever a newer version than its own is active, whatever the Client floors say                                                  | A Client relaunch stops nothing, and the owner wants people on the latest version                                | Chosen                                            |
| 10  | Automatic updates is a Settings switch, on by default, with Server scope                                                                                | The owner's pick; the Server is what schedules                                                                   | Announced                                         |
| 11  | `orchbee update` stays                                                                                                                                  | Scripts, people who turn automatic updates off, and the command line alone                                       | Chosen                                            |
| 12  | Only signed archives install; a lower version never installs on its own; disk stays bounded; one update runs at a time                                  | The feed and store rules, extended to the window                                                                 | Rule                                              |
| 13  | P1 follows slice 1 at once and starts now in parallel; its adoption core is shared with Worker-loss recovery                                            | Slice 1 already meets the non-stop rule; P1 is what lets Session fixes reach people without waiting              | Chosen                                            |

## Who checks and installs

- The Server of a release Session that runs from the store schedules the work: one run a minute
  after it is ready, then one every hour while it runs. A test build schedules only when
  `ORCHBEE_UPDATE_FEED` names a feed, so verification can drive it; a development build never
  does.
- Each run reads `update.automatic` and does nothing when it is off. Otherwise it starts the active
  version's `orchbee --automatic-update-v1`, named by `active.json`, in a Job the Server holds, at
  below-normal priority, with standard output bounded to 64 KiB. The Server never waits on it and
  records the outcome in its diagnostic log: version, phase, and error code, never user content.
  The helper ends with the Server; every update step is crash safe, and the next run cleans up.
- `--automatic-update-v1` is `update` with three differences: it never adds the `Path` entry,
  although it refreshes `bin\orchbee.exe` when that file exists; it installs the window part only
  when `desktop\` exists; and it prints only the `orchbee.update` document. Like every helper mode,
  its grammar never changes.
- Clients learn that a version is ready from the files, so no protocol changes: each Client reads
  the store's `active.json`, and the window also its window pointer, when it starts and whenever
  their last-write time changes, checked once a minute.
- Several Clients share their Session's one schedule. Several Sessions of one environment each
  schedule; `download.lock` and `install.lock` already serialize them, a run that meets a held lock
  ends as `busy`, and the next run finds the version installed.
- A remote machine's Session updates that machine's store under that machine's setting. A Client
  switches only to versions in its own computer's store.
- A window whose Session cannot start schedules nothing, and still offers a version an earlier run
  installed.
- Rejected: a Client-side schedule, because several Clients would check at once and a person who
  uses only the command line keeps none running; a Windows scheduled task, because it adds a
  system entry and runs while Orchbee does not; a service, which
  [versioning and upgrade](versioning-and-upgrade.md#out-of-scope) already rules out.

## How the window updates

Decided: the window updates too. Chosen: each window version gets its own directory, behind a small
launcher at the path every shortcut already names.

```text
<store root>\desktop\
├── orchbee-desktop.exe     the window launcher
├── active.json             the window pointer
└── versions\<version>\     one window version: the directory Setup installs today
```

- A release adds `orchbee-desktop-<version>-windows-x86_64.zip`, the packaged window directory.
  Its feed names it in `extensions.desktop` by address, size, and SHA-256, so the feed signature
  covers it, and version 1 readers already ignore `extensions`.
- The helper installs the runtime first, then the window: download into `.download\`, check size
  and SHA-256, unpack, publish by rename into `desktop\versions\<version>\`, then move the window
  pointer under `install.lock`. A running window is never touched.
- The window launcher is `orchbee-desktop-launcher.exe` from the signed release directory, copied
  to `desktop\orchbee-desktop.exe` by Setup and by completion when its bytes differ, as the PATH
  step copies the command launcher. It starts the pointer's version with its own command line.
  Given `--relaunch-after-v1 <process id>`, it first waits up to 30 seconds for that process to
  end, because the window's single-instance lock would otherwise hand the new window to the old
  one. When the active version cannot start, it starts the previous one.
- The Start menu shortcut, AppUserModelID `com.orchbee.desktop`, the uninstall registration, and
  Setup's Launch box keep naming `desktop\orchbee-desktop.exe`, so the identity does not change.
  The window sets its taskbar relaunch command to the launcher, so a pinned window starts the
  active version. Unverified: a pin made while the window runs from a version directory keeps the
  launcher.
- Setup installs the same layout: it adds a version directory beside any running one and moves the
  window pointer, so it no longer needs the window to quit, except once to move a flat layout.
  Uninstall removes `desktop\` with every version.
- Flat layout: a window installed by 0.1.0-beta.3 or earlier keeps its files directly in
  `desktop\`. `update` installs the first version directory beside them and swaps them out only
  when no process runs from them: the launcher takes the place of the flat `orchbee-desktop.exe`,
  and the other flat files go. A running window executable is never renamed or replaced, because
  Electron starts its helper processes from the path it started from (Unverified; the rule stands
  either way). The 0.1.0-beta.4 Setup makes the same move once the window quits, as it asks today.
- The window version and the runtime version move separately; any pair that shares a connection
  generation works together, because [versions coexist](versioning-and-upgrade.md#outcomes).
- Rejected: running the downloaded Setup silently at the click, because the window stays away for
  the whole install and Setup's rule against running files ties updating to Inno Setup, which macOS
  and Linux lack; renaming the running window's files aside, because a half-swapped Electron tree
  breaks the window. VS Code likewise installs in the background and swaps at Restart to Update, and
  Squirrel.Windows keeps each version in its own directory behind a stub.
- macOS and Linux follow the same idea when they ship, a versioned bundle or tree behind a stable
  entry; this page settles Windows.

## What a click does

A click switches the Client it is in; other open Clients keep their own control.

### TUI

1. The TUI checks that the active version can join its Session: the active manifest's `client`
   component lists the `client-server` generation this connection negotiated. Otherwise
   [the new Client cannot join](#when-the-new-client-cannot-join).
2. It restores the console and exits with status 75. Its place is already saved: the TUI keeps the
   [Windows startup preference](../../apps/orchbee/docs/native-context-selection.md#windows-startup-preference)
   current after every successful draw.
3. The launcher that started it, `bin\orchbee.exe` or the npm launcher, reads `active.json` again
   and starts the active version with the unchanged command line in the same console. The new TUI
   joins the same Session, and a bare start opens at the saved place.

- A launcher announces the protocol through a variable that carries its own process id, and the
  TUI uses status 75 only when that process is its parent, so a stale inherited variable does
  nothing. Status 75 lies outside the [CLI exit codes](../../docs/cli/output.md#exit-codes). A
  launcher relaunches at most three times in ten seconds.
- A TUI started without such a launcher exits normally and prints one line: "Run orchbee to open
  version {version}."
- macOS and Linux replace the process in place when they ship, so they need no launcher protocol.

### Window

1. The window makes the same check against its active window version.
2. It starts the window launcher with `--relaunch-after-v1` and its own process id, then quits the
   way Quit Orchbee, Keep Terminals Running does: the Session and every terminal keep running.
3. The launcher waits for it to end and starts the active window version, which joins the same
   Session. The new window restores its bounds as every start does, and the selected Workspace,
   Tab, and Panel through a one-time hand-off from the old window.

### When the new Client cannot join

A newer Client joins an older Session while their `client-server` generations overlap. Every
connection is at generation 1 today, and a newer Client checks each negotiated capability before it
uses one. When the active version shares no generation with the running Session:

- Before P1b the click does not switch, and a notice says "This update applies when no terminal is
  running." The Session then [moves by itself](#how-the-session-moves), and the next start opens
  the new version.
- From P1b the click first restarts the Session's Server and Worker, keeping every terminal, then
  switches the Client.
- The `initiator-too-new` message keeps naming a Session stop only until P1b replaces it with
  Restart Session; a release-build `server stop` already asks the person to type the Session's
  name.

[Open question 1](#open-questions-for-the-product-owner) would keep this case from arising in
practice.

## How the Session moves

Decided: the background Session moves to the new version by itself once no terminal runs.

- Slice 1: a Server older than the store's active version ends itself through its ordinary stop
  once all of these have held for 60 seconds: no terminal runs in the Session, no Client is
  connected, no update helper runs, and no operation is in flight. Nothing is left to end, so the
  stop asks nothing. The next Client to start finds no Session and starts the active version, which
  restores the layout as after any stop.
- The no-Client condition exists because nothing reconnects on its own yet: ending a Server under a
  connected window or TUI would leave it on a Disconnected page. A window hidden in the
  notification area quits when its last terminal ends, so the condition usually follows at once.
- The idle move is the only automatic Session change before P1, and it cannot stop a program,
  because it waits until none runs.
- P1b changes three things. Clients reconnect on their own, so the no-Client condition goes. A
  click restarts Server and Worker when the Session's
  [tier](versioning-and-upgrade.md#restart-decision) is `recommended`, keeping every terminal. A
  `required` tier keeps its confirmation. Automatic restarts while terminals run stay
  [out of scope](versioning-and-upgrade.md#out-of-scope).
- P1b restarts only when the active version reads the running Session's record formats. Until the
  first public release those may change without migration; otherwise the Session waits for the
  idle move, where the worst case is a reset layout with nothing running.

## The Update control

The TUI comes first, and the window follows it under the
[common feature contract](../../docs/INTERFACE-DESIGN.md#common-feature-contract).

| State         | When                                                                      | TUI                                                                | Window                                                     |
| ------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------- |
| Absent        | No version newer than this Client's own is active, or a development build | Nothing                                                            | Nothing                                                    |
| Ready         | A newer version is active; for the window, a newer window version         | `[ Update ]` on the Settings row, its label in `accent-action`     | An Update button in the sidebar footer, in the accent role |
| Switching     | After a click                                                             | The TUI leaves at once and its new version opens; no pending state | The same                                                   |
| Cannot switch | A check before the switch fails                                           | One notice; the control stays                                      | The same, in the window's notice surface                   |

- Place: on the Settings row right after Settings, one blank cell apart as adjacent buttons are.
  Development builds, whose
  [environment marker](../../apps/orchbee/docs/tui-shell-layout.md#environment-marker) uses that
  row, never show the control. When the row is short, Help drops first and then Update, which
  stays reachable in the action menu. The window footer puts it between Settings and Help, where
  its environment marker sits.
- Keyboard: the control invokes one Client Action, Update, which the action menu (`Ctrl+b ?`) lists
  while it is available. It has no default key.
- A failed background check or install changes nothing on screen; the next hour retries, and
  `orchbee update` or the diagnostic bundle shows what went wrong.
- No operating-system notification and no badge elsewhere: the person sees the control where they
  already look for Settings.

| Text                             | `en-US`                                                                                       | `zh-CN`                                                |
| -------------------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Control                          | Update                                                                                        | 更新                                                   |
| Tooltip                          | Switch to Orchbee {version}. Terminals keep running.                                          | 切换到 Orchbee {version}，终端继续运行。               |
| Cannot switch                    | Couldn't switch to the new version.                                                           | 无法切换到新版本。                                     |
| No common generation, before P1b | This update applies when no terminal is running.                                              | 此更新将在没有终端运行时生效。                         |
| TUI without a launcher           | Run orchbee to open version {version}.                                                        | 运行 orchbee 打开 {version} 版本。                     |
| Setting                          | Automatic updates                                                                             | 自动更新                                               |
| Setting description              | Download and install new versions in the background. Nothing restarts until you click Update. | 在后台下载并安装新版本。点击“更新”前不会重启任何内容。 |

The copy is proposed for the owner's review before it ships. The TUI tooltip needs one amendment
to [pointer tooltips](../../docs/tui/overlays-and-status.md#pointer-tooltips), whose action
content is a label and its key: an effect line for a control whose label cannot say what keeps
running. The action menu entry carries the same sentence, so it stays reachable without a pointer.

## Settings and the command

- `update.automatic`: boolean, default `true`, Server scope, under General in both Settings pages,
  applied at once like every setting. The Server reads it before each run. Off stops the schedule
  and its downloads; a version installed by hand still shows the control. Development builds
  ignore it.
- `update.channel` stays file-only. Automatic runs follow the selected channel and never change it.
- `orchbee update` stays and also installs the window part when `desktop\` exists, so a manual
  update leaves no older window behind. The `orchbee.update` document reports the window part
  through added optional members.

## Cadence, cost, and the network

- One check per running Session per hour fetches two files of about 0.7 KB together from
  orchbee.com, which Cloudflare serves as static assets free and without a request limit;
  `run_worker_first` stays off for `/update/*`, because a Worker invocation counts against the free
  plan.
- Archives come from GitHub Releases only when a new version appears: 24.9 MB for the 0.1.0-beta.2
  runtime and, for window users, the window archive, about 100–130 MB (unmeasured). Nothing costs
  the project money.
- Offline or failing: nothing on screen, and the next hour retries. A cut download starts over,
  since neither resume nor retry is frozen.
- Proxy: the helper passes `HTTPS_PROXY`, `ALL_PROXY`, and `NO_PROXY` from the Server's environment
  and otherwise the per-user system proxy, as
  [release distribution](release-distribution.md#commands) already decided; slice 1 builds the
  system proxy read. Automatic configuration scripts are not read.
- A lower version never installs automatically. A channel pointed back after a bad release reads as
  up to date, and the next fix moves people forward.

## Trust and disk

Rule: each step reuses a store rule.

- Only signed archives install. The feed signature covers both archives' size and SHA-256, the
  runtime's `release.json` is verified again after unpacking, and its smoke check runs before
  completion.
- No silent downgrade, no channel change, no elevation, and no service.
- One update at a time: `download.lock`, then `install.lock`. A Setup that meets the lock reports it
  as it does today.
- Bounded disk: after each install the helper deletes, under `install.lock`, every runtime and
  window version that is neither active nor previous, runs no process, and is referenced by no
  Session, Host, or Capsule. A store therefore holds the active and previous versions plus whatever
  still runs. Unmeasured: about 60 MB per runtime version and 300 MB per window version, unpacked.
- A switch never touches a running file, and a failed install leaves both pointers where they were.

## Phases

| Phase   | Content                                                                                                                                        | Size                                                                 | When                             |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | -------------------------------- |
| Slice 1 | Everything above except what names P1                                                                                                          | L: about 18–21 agent-days in three parallel streams, about two weeks | 0.1.0-beta.4                     |
| P1a     | [Host adoption](versioning-and-upgrade.md#host-adoption) with Worker-loss recovery                                                             | 1.5–3 weeks after its decision run                                   | Starts now, beside slice 1       |
| P1b     | Server and Worker restart on adoption: durable binding record, rollback, the click moves the Session, the idle move drops its Client condition | 1–2 weeks                                                            | Right after P1a and slice 1      |
| P1c     | Host rollover: one Worker brokers several Hosts, and new terminals start on the newest                                                         | About 1 week                                                         | After P1b                        |
| P2      | Cross-version tests: slice 1 adds one check to the Update check workflow, and the rest arrives with P1b                                        | Inside each phase                                                    | With slice 1 and P1b             |
| P3      | Live handoff prototype, feasibility report, and Host split evaluation                                                                          | About 1 week                                                         | Right after P1                   |
| P4      | Opt-in live handoff for a required Host replacement                                                                                            | 3–6 weeks                                                            | If the owner accepts P3's report |

| Slice 1 stream | Content                                                                                                                                                                                                                                                                                                                         | Size                 |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| Runtime        | Server schedule and helper mode, outcome record, cleanup of old versions, system proxy, the idle move, `update.automatic` in both Settings pages                                                                                                                                                                                | About 7 agent-days   |
| TUI            | The control, its Action and tooltip amendment, the relaunch status in both launchers, the join check, copy                                                                                                                                                                                                                      | About 4 agent-days   |
| Window         | Window archive and `extensions.desktop` in release tooling, the window launcher, Setup's versioned layout and the flat move, the window part of `update`, the control and relaunch, copy                                                                                                                                        | About 9 agent-days   |
| Evidence       | The [Update check workflow](../../docs/VERIFICATION.md#update-check-workflow) also attaches the new Client to a Session of the previous version. One scripted Windows run keeps a long program in a terminal through an automatic install and a click in each Client, and checks that it keeps running and its output continues | About 1.5 agent-days |

- If 0.1.0-beta.4 must ship sooner, the runtime and TUI streams go first, in about one week, and the
  window stream follows in 0.1.0-beta.5; until then the window keeps today's Setup path.
- P1 does not ship with slice 1, because slice 1 already meets the non-stop rule: no automatic path
  touches a running terminal. Bundling would hold every automatic update back by three to five
  weeks. The cost of shipping slice 1 alone is that a Server, Worker, or Host fix reaches a person
  only once no terminal runs, while a Client fix reaches them at the next click.
- P3 does not move earlier. Inside 1.x only a security fix can force a Host change, and P3 depends
  on the adoption core. Until P4, a required Host replacement remains a confirmation that lists the
  terminals that restart, and declining keeps the old Host and its visible tier. P3 is dated right
  after P1 because its Host split evaluation could also contain a crash in output parsing, which
  today ends every program in that Host.
- macOS and Linux join when their release platforms do.

## Open questions for the product owner

1. Should connections keep their previous generation from 0.1.0-beta.4 on? Today an incompatible
   change before the first public release drops the old generation at once, so after such a
   release a click cannot switch until the Session is idle. Recommended: yes for `client-server` and
   `worker-host`, keeping each previous generation for the next two releases and at least 90 days,
   the window that otherwise starts at the first public release.
2. With releases several times a week, each window update downloads about 100–130 MB. Recommended:
   accept it during the beta; later, a release whose window did not change names the previous
   window archive.
3. The copy in [the Update control](#the-update-control). Recommended: as proposed.
4. A window installed by 0.1.0-beta.3 or earlier reaches the self-updating layout only through the
   0.1.0-beta.4 Setup or a moment when no window runs. Recommended: say so in the 0.1.0-beta.4
   release notes and build nothing more.
