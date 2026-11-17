# Automatic Updates

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Proposed on 2026-11-15 from the product owner's request of that day, and revised the same day after the owner asked that upgrades across several releases count as much as single ones, and again after the owner asked that a Host change reach running terminals by [Host handover](host-handover.md). Decided restates a product-owner decision. Announced marks a pick the owner was told about and may still veto, and Chosen marks this page's own pick, which the owner may also veto. Rule marks a safety or trust rule that follows from an accepted outcome.             |
| Owner             | Maintainers of `apps/orchbee`, `apps/orchbee-server`, `apps/desktop`, `apps/web`, `crates/local-runtime`, the release scripts under `scripts/`, and the release workflow                                                                                                                                                                                                                                                                                                                                                                                                           |
| Current authority | [Windows release installation](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation), the [Windows installer](../../apps/desktop/README.md#windows-installer), [running in the notification area](../../apps/desktop/docs/window-lifecycle.md#running-in-the-notification-area), the [TUI Settings row](../../apps/orchbee/docs/tui-shell-layout.md#sidebar-toggle-and-help), [native settings](../../apps/orchbee/docs/native-settings.md), [release distribution](release-distribution.md), and [versioning and upgrade](versioning-and-upgrade.md) |
| Exit condition    | Delete this proposal once slice 1, the window self-update release, and P1 ship and each rule lives in its owning package reference or standard.                                                                                                                                                                                                                                                                                                                                                                                                                                    |

Orchbee releases often. This page decides how a release reaches a running installation, from any
recent version, with no step from the person but one click, and without stopping anything they
run. It builds on the signed [release feed](release-distribution.md#release-feed), the
[runtime store](release-distribution.md#runtime-store), and the
[restart decision](versioning-and-upgrade.md#restart-decision).
[Host adoption](versioning-and-upgrade.md#host-adoption) is the one mechanism that later moves a
running Session.

Two releases are named by role, not by number: **slice 1**, the first automatic-update release, and
the **window self-update release** after it. Each is whichever release is ready to carry it.

## Outcomes

1. **Non-stop.** Decided 2026-11-15, above every other outcome. An update never stops, restarts, or
   interrupts what runs on the person's computer: terminals and their programs, Agents, and the
   background Session that holds them. Every automatic path keeps them running. Anything that would
   end or restart them is at most an explicit choice the person confirms after being told what
   stops. An update may stall; it never interrupts. When another goal conflicts with this one, this
   one wins.
2. **From any recent version.** Decided 2026-11-15. An upgrade that skips releases is as much a
   first-class case as one from the release right before. Inside the
   [compatibility window](#compatibility-counts-time), every release of the last 90 days, it
   behaves exactly the same.
3. **Latest by default.** With automatic updates on, a release reaches every running installation
   within about an hour of its channel moving and installs in the background.
4. **One click.** The Update control switches the window or TUI it sits in to the installed version
   at once, on the same Session and at the same place. Terminals and Agents keep running.
5. **The Session follows.** The background Session moves to the new version by itself once no
   terminal runs. After P1 the same click also moves it, and terminals continue. From H4 of
   [Host handover](host-handover.md#phases), running terminals then move to the new Terminal Host.
6. **Off means off.** With automatic updates off, nothing is checked or downloaded on a schedule;
   `orchbee update` keeps working.

The owner's reasons: releases come often so that fixes reach people quickly, and that works only if
updating never costs them their running work. On 2026-11-15 the owner added that single upgrades and
upgrades across several releases both need care, that a backup plan must at least keep updates from
interrupting anything, and that the person is told only in the worst case, when an update cannot go
on without interruption.

### What still breaks the non-stop rule

| Behavior today                                                                                                                                              | What the person loses                                                                                   | Removed by                                                                                                                             |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| After a Worker loss the Host keeps every program, but nothing can show, reach, or stop them; Quit All and `orchbee server stop` refuse while the Host lives | Every terminal, until the Host is ended from outside or the computer restarts, which ends every program | P1a: the Server replaces the Worker, and a confirmed stop really stops                                                                 |
| A Server or Worker change takes effect only after a Session stop, which ends every terminal; the `initiator-too-new` message asks for `orchbee server stop` | The terminals, or the fix                                                                               | Slice 1 for an idle Session; P1b while terminals run, by [make before break](#make-before-break)                                       |
| A Host change reaches a terminal only after its Host empties; a Host below its security floor changes only by restarting its terminals after a confirmation | Host fixes, for a terminal that never ends; at a security floor, the listed terminals                   | P1c for new terminals; [Host handover](host-handover.md) for running ones                                                              |
| Configured `shell` commands, plugin jobs, and plugin hooks run under the Worker, unseen by the Server, so a Server and Worker change would cancel them      | Those programs                                                                                          | Slice 1 and P1b wait while one runs; B1 of [Host handover](host-handover.md#background-programs-the-worker-runs) runs them in the Host |
| A crash in output parsing ends every program in its Host, and one panic that crafted output can cause is still open                                         | Every terminal in that Host                                                                             | Recovery, H2 of [Host handover](host-handover.md#what-a-crash-or-hang-costs)                                                           |
| Setup refuses while terminals started by an older window run from files under `desktop\`, and asks the person to close them                                 | Those terminals, if the person follows the advice                                                       | The window self-update release: Setup and `update` add window versions beside running ones                                             |
| Session records change format in place, without migration                                                                                                   | The layout at an idle move; adoption at a restart                                                       | Slice 1: [records read forward](#records-follow-the-window) from every format since then                                               |
| A new version that keeps crashing after it takes over has no way back but a manual reinstall                                                                | Access to the terminals until a fix ships                                                               | Slice 1: the bounded [fallback](#falling-back-after-a-failed-move)                                                                     |

A computer restart still ends every program, as
[Host lifetime](versioning-and-upgrade.md#host-lifetime) says; it is not an update path.

## Comparable products

Applying an update:

| Product                                      | Checks                                           | Installs                                                          | The person sees                                    | Running work at the switch                         |
| -------------------------------------------- | ------------------------------------------------ | ----------------------------------------------------------------- | -------------------------------------------------- | -------------------------------------------------- |
| VS Code                                      | 30 seconds after start, then hourly              | In the background on Windows                                      | A badge on the Manage gear, then Restart to Update | Terminal processes end; their text may be restored |
| Zed                                          | Hourly                                           | In the background                                                 | Restart to update in the title bar                 | Terminals end                                      |
| Chrome                                       | Its own updater                                  | In the background                                                 | An Update button in the toolbar                    | Tabs reopen; page state is lost                    |
| Squirrel.Windows apps such as GitHub Desktop | At start                                         | Into a new version directory beside a stub that starts the newest | A restart prompt                                   | The application restarts                           |
| Orchbee                                      | One minute after the Session starts, then hourly | In the background, beside the running version                     | A primary Update at the end of the Settings row    | Terminals and Agents keep running                  |

Each of them ends running work when it applies an update. Orchbee keeps it: the Terminal Host, which
owns every program, takes part in no Client or Session switch, and a Host change moves each
terminal to the new Host by [Host handover](host-handover.md).

Across versions:

| Product        | A newer client meets an older server                                                                                                                                                                                                                                            | Running work                                                                            |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| tmux           | The client exits with "protocol version mismatch". tmux's change notes say to restart the server after an upgrade; people instead reattach through the running server's old binary, as in `/proc/<pid>/exe attach`, a community workaround                                      | Kept only while people use the old binary; restarting the server ends it                |
| herdr          | The client negotiates an endpoint generation, and a missing server feature disables only the affected action (#3509). Replacing a server asks before stopping its pane processes, with No as the default; live handoff stays an opt-in experiment and is unsupported on Windows | Compatible servers and their agents stay untouched                                      |
| VS Code remote | The VS Code CLI installs one server per exact commit, named `<quality>-<commit>`, side by side, and prunes only servers that are not running                                                                                                                                    | The old server keeps its processes until it stops; the new client starts its own server |
| Orchbee        | Every generation superseded in the last 90 days stays supported, a missing capability disables one action, and a Session restarts only after the new pair has taken over every Host                                                                                             | Terminals and Agents keep running                                                       |

Orchbee takes herdr's capability fallback and its No-by-default confirmation, keeps each version in
its own directory as VS Code does, and replaces tmux's refusal with waiting: outside the window an
update waits for the idle move instead of asking for a restart. Where herdr's live handoff stays an
opt-in Unix experiment, Orchbee moves terminals to a new Host by default on every platform;
[Host handover](host-handover.md#comparable-products) compares the long-running services it
borrows from.

## Decision record

| #   | Decision                                                                                                                                                                                                    | Reason                                                                                                                    | Status                                                                                           |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| 1   | The Session Server checks one minute after it starts and then hourly, through a background helper run of the store's active `orchbee`                                                                       | One Server per Session exists for TUI, CLI, and window users alike; Clients come and go, and several may be open          | Announced                                                                                        |
| 2   | The helper downloads, verifies, and installs beside the running version and never touches a running file                                                                                                    | The store already installs only by adding directories                                                                     | Announced                                                                                        |
| 3   | The window application updates too, through versioned window directories behind an unchanged launcher path, from the window self-update release on                                                          | A switch becomes an ordinary relaunch; nothing running is replaced; shortcut, identity, and registration stay             | Announced that it updates; Chosen how; its own release Decided by the owner's release-often rule |
| 4   | An Update control ends the sidebar's Settings row in the primary button style, in the TUI and in the window                                                                                                 | The owner's request; place and style revised 2026-11-17                                                                   | Announced                                                                                        |
| 5   | A click switches only the Client it is in                                                                                                                                                                   | Relaunching a Client the person is not looking at would interrupt their typing there                                      | Chosen                                                                                           |
| 6   | The TUI relaunches in the same console through its launcher; without one it leaves with a one-line hint                                                                                                     | Windows has no process replacement; the launcher already owns the console and the exit code                               | Chosen                                                                                           |
| 7   | Before P1b the Session moves by itself only when no terminal runs and no Client is connected; P1b drops the Client condition and lets the click move it                                                     | Nothing reconnects on its own yet, so ending a Server under a connected Client strands it                                 | Decided by the owner; the Client condition Chosen                                                |
| 8   | Failed checks, installs, and fallbacks show nothing and retry on the next hour; the only text is the waiting notice while an update waits                                                                   | The person cannot act on a network failure; `orchbee update` and diagnostics give details                                 | Decided 2026-11-15                                                                               |
| 9   | A Client offers Update whenever a newer version than its own is active, whatever the Client floors say                                                                                                      | A Client relaunch stops nothing, and the owner wants people on the latest version                                         | Chosen                                                                                           |
| 10  | Automatic updates is a Settings switch, on by default, with Server scope                                                                                                                                    | The owner's pick; the Server is what schedules                                                                            | Announced                                                                                        |
| 11  | `orchbee update` stays                                                                                                                                                                                      | Scripts, people who turn automatic updates off, and the command line alone                                                | Chosen                                                                                           |
| 12  | Only signed archives install; the feed never installs a lower version, and the one move back is the bounded fallback; disk stays bounded; one update runs at a time                                         | The feed and store rules, extended to the window                                                                          | Rule                                                                                             |
| 13  | P1 follows slice 1 at once and starts now in parallel; its adoption core is shared with Worker-loss recovery                                                                                                | Slice 1 already meets the non-stop rule; P1 is what lets Session fixes reach people without waiting                       | Chosen                                                                                           |
| 14  | Upgrades that skip releases are first class: from slice 1, compatibility counts 90 days, not releases, capabilities come before generations, and `worker-host` grows only by Host capabilities              | Single and cross-version upgrades both need care                                                                          | Decided 2026-11-15                                                                               |
| 15  | Every release passes upgrade paths from up to four earlier releases before Publish                                                                                                                          | A broken path must stop the release, not reach people                                                                     | Decided 2026-11-15                                                                               |
| 16  | Make before break: a Session restart never ends the old pair before the new one has taken over, and any failure leaves the old pair while the update waits                                                  | An update may stall but never interrupt                                                                                   | Decided 2026-11-15                                                                               |
| 17  | A new version that fails three times within ten minutes of its move falls back to the previous version                                                                                                      | The backup plan for a release that passes the gate but fails on one computer                                              | Decided 2026-11-15; its bound is a Rule                                                          |
| 18  | Text appears only while an update waits; the only confirmation stays the required Host replacement, No by default                                                                                           | Notify only in the worst case                                                                                             | Decided 2026-11-15                                                                               |
| 19  | Session and Client records read forward from every format since slice 1                                                                                                                                     | An idle move or a restart never resets the layout, even across skipped releases                                           | Chosen                                                                                           |
| 20  | The copy in [the Update control](#the-update-control) as proposed                                                                                                                                           | Short words that say what keeps running, the same in both Clients                                                         | Chosen                                                                                           |
| 21  | A window installed by a release before the window self-update reaches the new layout only through that release's Setup or a moment when no window runs; its release notes say so, and nothing more is built | One Setup run per old window costs less than a migration tool                                                             | Chosen                                                                                           |
| 22  | Before the idle move, the Server asks its Worker how many plugin jobs and configured commands still run, through a same-version read-only query; any failure of the query keeps the Session                 | A configured command runs in the Worker with no terminal, so nothing else shows it, and the move must never end a program | Chosen                                                                                           |

Nothing waits on the owner; each Chosen row stays open to a veto.

## Who checks and installs

- The Server of a release Session that runs from the store schedules the work: one run a minute
  after it is ready, then one every hour while it runs. A test build schedules only when
  `ORCHBEE_UPDATE_FEED` names a feed, so verification can drive it; a development build never
  does.
- Each scheduled run reads `update.automatic` and does nothing when it is off; a run a Client asks
  for ([manual check](#manual-check)) does not read it. Otherwise it starts the active
  version's `orchbee --automatic-update-v1`, named by `active.json`, in a Job the Server holds, at
  below-normal priority, with standard output bounded to 64 KiB. The Server never waits on it and
  records the outcome in its diagnostic log: version, phase, and error code, never user content.
  The helper ends with the Server; every update step is crash safe, and the next run cleans up.
- `--automatic-update-v1` is `update` with three differences: it never adds the `Path` entry,
  although it refreshes `bin\orchbee.exe` when that file exists; it installs the window part only
  when `desktop\` exists, from the window self-update release on; and it prints only the
  `orchbee.update` document. Like every helper mode, its grammar never changes.
- Clients learn that a version is ready from the files, so the schedule needs no protocol: each Client reads
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

## Upgrades across releases

Decided 2026-11-15: an upgrade that skips releases is a first-class case, and five layers keep any
upgrade from interrupting anything. The first four are decided; the fifth is this page's pick.

### Compatibility counts time

- From slice 1, every connection generation superseded in the last 90 days stays supported, however
  many releases came in between; the
  [compatibility window](versioning-and-upgrade.md#compatibility-window) holds the rule. A
  skip-version upgrade inside the window therefore behaves exactly like an adjacent one: the new
  Client joins the running Session, and the Session moves as it would after one release.
- A change prefers a new negotiated capability to a generation bump. A peer without the capability
  loses only the action that needs it, which the Client shows as unavailable, as herdr does.
- `worker-host` changes only by addition, through capabilities the Host advertises, because a Host
  can run for weeks. A `worker-host` generation bump is a last resort and needs its own decision.
- Releases before slice 1 kept every connection at generation 1 while shapes changed in place, so
  the window cannot reach back past slice 1: a Session of an earlier release moves only at the idle
  move, and so does a Session outside the window. Either way the update stalls; it never
  interrupts.

### The release gate

Every release proves the window before Publish.

- Paths: one from each of the previous release, the one before it, the release nearest the middle
  of the window, and the oldest release inside the window. The window holds the releases published
  in the last 90 days, from slice 1 on. Picks that coincide count once, so the set never exceeds
  four however many releases the window holds, and a window of four or fewer tests every release.
- Each path runs on a fresh Windows runner. It installs the older release from its published
  archive into a scratch store, starts its Session, and opens a terminal running a probe that
  prints a numbered line every 100 ms. The older release's own automatic helper installs the new
  release from the gate feed, and the new Client joins the running Session and shows the probe's
  output. The probe then ends, and the idle move brings the Session up on the new version with its
  layout unchanged. From P1b the probe keeps running instead, and a click restarts the Session: the
  new pair adopts the probe's Host, and the layout survives.
- Each path checks that the probe kept its process id, that its numbers have no gap, and that the
  new Client joined.
- From H4 of [Host handover](host-handover.md#phases), a path whose new release changes the Host also
  checks that the Keeper was replaced, that the probe moved to the new Host, and that the old Host
  exited.
- A failing path stops the release before Publish: `release-publish` needs the gate, so nothing
  reaches npm, GitHub Releases, or the update channel.
- The feed is signed in a job of its own before the gate, which also signs a gate copy naming the
  archive by its file name; a release build already installs from a local feed whose archive sits
  beside it. The `release` environment requires no reviewer, so the extra job adds no approval, and
  the repository is public, so the runners cost nothing. The paths run in parallel, about 15
  minutes each (unmeasured).
- The gate first runs for the release after slice 1, the first with an earlier release that updates
  itself. The [update check workflow](../../docs/VERIFICATION.md#update-check-workflow) still runs
  after publication, over the public route.

### Make before break

An update can stall but never interrupt.

- A Session restart never ends the old Server and Worker before the new pair has adopted every
  running Host and accepted the Clients;
  [Server and Worker restart](versioning-and-upgrade.md#server-and-worker-restart) gives the steps.
- Preconditions come first: the new version shares a generation with every running Host and every
  connected Client, and it reads every Session record. Until B1 of
  [Host handover](host-handover.md#background-programs-the-worker-runs), the old Worker must also run
  no configured `shell` command, plugin job, or plugin hook, and the drain checks again, so the
  restart waits while one runs.
- Any failure, expected or not, leaves or puts back the old pair, and the update waits for the idle
  move. A Session running a release before P1b, which cannot take part, always waits for it.
- Before P1b, a new Client that cannot join keeps the Client the person has: the click checks the
  join first, and a launcher whose new Client fails to start or to join starts the previous version
  again, at the same place.
- From P1b, an old Client that cannot join the restarted Session switches itself to the active
  version, as a click does, because its terminals are safe.

### Falling back after a failed move

- A new version that fails three times within ten minutes of taking over sends the store back:
  `active.json` returns to its `previous` version, which is still on disk, and the ordinary start,
  with Host adoption from P1, brings the Session up on it. A failure is a Server or Worker that
  fails to start, crashes, or fails to adopt, or a Client that fails to start or to join;
  [what counts](#what-counts-and-who-records-it) names who records each.
- A record that the failed version migrated to a higher format version is restored from that
  version's upgrade backup, so the previous version opens the Session as it was before the move;
  changes made in those minutes are lost. A change kept in a record's `extensions` object needs no
  restore; [restoring records](#restoring-records-after-a-fallback) says when and by whom.
- Rule, the bound. This is no downgrade served by the feed, which stays forbidden: it installs
  nothing and returns only to the version that ran minutes before. It moves only to `previous`,
  never below the `securityFloor` of any component in the failed version's own verified manifest,
  never to a release before slice 1, only within ten minutes of the failed version taking over, and
  at most once per move: the failed version is held, so nothing in the store offers, installs, or
  activates it again until a higher version has become active. Each fallback is logged. A peer
  that can crash the new version therefore gains at most the version that ran before it, never one
  below a security fix, because the fix's release raises the floor in its own manifest and every
  later release states it again.
- Where the bound forbids the fallback, the Session behaves as after a Worker loss past the
  [restart cap](versioning-and-upgrade.md#host-adoption): the programs keep running, and only a
  confirmed stop ends them. `orchbee update` still installs the next fix.
- The mechanics below are Chosen, which the owner may veto; the bound stays a Rule.

#### The fallback record

The store keeps one record in the pointer's open slot, `extensions.fallback` in `active.json`. Here
it is a few minutes into a move to 0.5.0, and then after that version's fallback to 0.4.0:

```text
{
  "held": [],
  "move": {
    "entry": { "product": "0.5.0", "manifest": "<SHA-256 of its release.json>" },
    "back": true,
    "client": { "since": 1794000000000, "failures": 1 },
    "session": { "since": 1794000000000, "starts": ["<32 hex>"], "failed": [] }
  }
}

{
  "held": ["0.5.0"],
  "move": {
    "entry": { "product": "0.4.0", "manifest": "<SHA-256 of its release.json>" },
    "back": false,
    "restore": true
  }
}
```

- `held` and `move` are always present; `restore`, `client`, and `session` appear only as below.
- `held` lists the versions that nothing may make active, at most eight, in the order they were
  held. Every write keeps only those above the active version, and a ninth drops the earliest.
- `move` describes the move that made `entry` active, with `product` and `manifest` exactly as in
  `active`. It counts only while `entry` equals `active`; otherwise the pointer has no counting
  move.
  - `back` says whether this move may fall back; it is settled when the move is written.
  - `restore`, true, appears only on the `move` a fallback writes.
  - `client` is the Client window, absent until opened: `since`, in Unix milliseconds UTC, and
    `failures`, 0 to 3.
  - `session` is the Session window, absent until opened: `since`; `starts`, the activation
    identifiers of at most eight generations this version started, 32 lowercase hexadecimal digits
    each; and `failed`, those of them that failed, at most three. A ninth start drops the oldest
    identifier not in `failed`.
- Every object is closed and every value bounded, so the record stays under 2 KiB, inside the
  pointer's 16 KiB bound. A `fallback` member of any other shape counts as absent. The shape never
  changes; a later need adds another member of `extensions`, which every writer keeps.
- Each write takes `install.lock`, which every pointer write already takes, reads `active.json`,
  applies one transition, and replaces the file through the store's flushed rename. No two writes
  interleave, each starts from the file the last one left, and a crash at any point leaves the old
  file or the new one. Readers still take no lock.
- Everything in this section runs only where the local runtime's
  [dark-launch switch](../../crates/local-runtime/README.md#automatic-update-switch) is on. A build
  where it is off behaves as a release before slice 1: it records nothing and keeps `extensions`
  verbatim, as every rewrite does, so a move it makes leaves a `move` that no longer counts.
- A recorder tries the lock every 50 ms for at most 2 s, then drops the record and logs
  `fallback-record-dropped`. A dropped failure or Session start can only delay a fallback, and a
  dropped TUI start only lets the Client window open at the first failure instead. The Session
  start notice is written just before the Server starts, each Session failure on the path that
  found it, and the helper below writes as its only work; the TUI start is written on a background
  thread that its process waits for before it exits, so it never delays the TUI.
- Rejected: a file of its own under its own lock, because a fallback would then change two files
  under two locks, and a crash between them could move the pointer without holding the version.

#### What counts and who records it

| Event                            | Recorder                                                                                                                                        | Effect                                                       |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| Session start                    | The activator of the moved version, after `begin_activation` and before it starts the Server                                                    | Opens the Session window and adds the generation to `starts` |
| TUI start                        | The first TUI of the moved version to start                                                                                                     | Opens the Client window                                      |
| Server or Worker fails to start  | The activator that started the Server, when starting it fails or it ends before its endpoint appears, as it does when its Worker fails to start | A Session failure of that generation                         |
| Damaged version                  | The activator of the moved version, when its own cohort check finds a file changed or missing, before any generation exists                     | A Session start and failure of a fresh generation            |
| Server crash                     | Any activator whose crash recovery proves the generation dead                                                                                   | A Session failure of that generation                         |
| Worker crash                     | The Server, when its Worker exits outside a stop                                                                                                | A Session failure of the Server's generation                 |
| Client fails to start or to join | Either launcher, as it classifies the outcome, through the previous version's helper below; on macOS without a launcher, the TUI itself         | A Client failure of the version it started                   |

- A recorder writes only the store its own executable runs from, so a process outside a store
  version folder records nothing. It first reads the pointer without the lock and goes on only
  when the `move` counts, `back` is true, and the role's window is open or not yet opened, and it
  checks the same under the lock. A Session or TUI start also needs the recorder's own version to
  be the moved one, and a TUI start a Client window not yet opened. A start long after a move
  therefore never takes the lock.
- A Session failure counts once, for a generation in `starts`: its identifier joins `failed`. A
  generation another version started, or whose start notice was dropped, counts nothing, and so
  do a Server still alive at the activator's deadline and a generation that never started its
  Server. A damaged version is the exception: a Client whose own files fail the cohort check as
  changed or missing can start no Server from them, so each such start counts as a generation that
  started and failed, and a store file damaged after an install falls back like a crash. A
  deadline or a refused open during that check counts nothing.
- A failed adoption counts from P1b, which adds the fallback after a restart.
- `--record-client-failure-v1 <version>` is a hidden helper mode whose grammar never changes, left
  out of help, completion, and the Agent guide like the other helper modes. It records one Client
  failure of `<version>` in the store it runs from, counted only while `<version>` is the entry of
  the counting `move`, and that write may commit the fallback.
- A launcher tells a failed start or join from a normal exit after every start it makes, the first
  one or a [relaunch](#tui). When its Client fails, it reads the pointer again; when the pointer
  still names the entry it started, with a counting `move`, `back` true, and a `previous`, it runs
  `<root>\versions\<previous>\<name>.exe --record-client-failure-v1 <version>`
  outside the Client's Job with null standard streams and waits for it at most 5 s. Then it goes on
  as it would without the record: after a relaunch it starts the version it replaced, and after a
  first start it ends with the Client's status. The previous version runs the helper because
  neither launcher links a store writer and the failed Client may be what broke; `back` true proves
  that the previous version records.

#### When the ten minutes start

- Each role has its own window: 600 000 ms of UTC wall-clock time from its first record after the
  move. The Session's opens with the first start notice, the Client's with the first TUI start or
  Client failure. A version that an automatic run installs while nobody uses Orchbee thus gets its
  ten minutes when it first runs.
- The third failure of one role inside its window falls back; the two roles count separately.
- A failure dated before `since`, as after a clock change, counts nothing. From `since` plus
  600 000 ms the window is closed, and nothing more is recorded for that move.
- Rejected: counting from the pointer move, which an automatic install may make hours before the
  version first runs; and one count for both roles, whose windows open at different moments.

#### The move

Every completion from slice 1 on works under `install.lock` in this order:

1. It refuses a held target with exit status 3, the status of the lower-version refusal, so a
   caller before slice 1 takes it for that refusal; a slice 1 caller reads the pointer again and
   reports `version-held` when the target is held.
2. It moves the pointer as today, the replaced entry becoming `previous`, and drops held versions
   at or below the target.
3. On a real move it replaces `move` with a fresh one for the target, with `back` settled as
   below; activating the active entry again keeps the record as it is.

A build whose switch is off moves the pointer as today and keeps `extensions` verbatim. It never
needs the refusal, because it only ever activates itself, and only a build with the switch on
records, so only such a build can be held.

`back` is true only when all of these hold:

- the move is real: the target differs from the active entry;
- the record's `move` counts for the replaced entry, which proves that the replaced version
  records, since only a build with the switch on writes a `move`;
- the replaced version's `release.json` still hashes to its entry's `manifest`;
- for every component `securityFloor` in the target's own verified manifest, the replaced
  manifest's component with the same `id` has a version at or above it; a component missing from
  either manifest imposes nothing.

The completion runs in the target's own binary, so the target's code settles `back`. The floor
check relies on every release stating each floor still in force, as the
[restart decision](versioning-and-upgrade.md#restart-decision) already does; a floor that a newer
release publishes later applies once that release installs, which the next hourly run does.
Rejected: the floors of the newest manifest the feed served, which only a network read knows and
which can change after the move.

#### The fallback

The write that records a role's third failure also commits the fallback when `back` is true and
the version folder that `previous` names exists:

- `active` becomes `previous`, and `previous` is removed;
- the failed version joins `held`, which keeps only versions above the new active one;
- a fresh `move` for the new active entry has `back` false and `restore` true, so that version
  keeps its proof for its next move and falls back no further;
- the process that commits logs `fallback` with both versions.

After a fallback `previous` stays absent until the next move, so cleanup may delete the failed
version's folder once nothing runs from it. Rejected: swapping the two, which would make the failed
version the next fallback target. The fallback restarts nothing: the next start of a Client or of
the Session reads the pointer and runs the previous version. A Server of the failed version that
still runs keeps its Session until it ends, because the [idle move](#how-the-session-moves) moves
only older Servers.

Without the folder that `previous` names, the third failure is recorded, nothing falls back, and
that role records nothing more for the move.

#### Held versions

A held version never becomes active again until a higher version has, which drops it from `held`:

- every completion refuses it, as above;
- the update engine's `decide` treats it as up to date, from the lock-free pointer read it already
  makes, so nothing downloads, and a completion's `version-held` refusal in a race reads as up to
  date too;
- `install` refuses it before copying, with exit status 3 like `downgrade-refused` and the error
  code `version-held`, which its result schema and localized messages gain;
- Setup treats `version-held` as it treats `downgrade-refused`: a log line, no message box, and the
  window part still installs;
- the npm launcher runs the active version when its carried version is held, and when its install
  meets `version-held`.

The active version and `previous` are never held, so the [Update control](#the-update-control)
needs no check of its own. Cleanup may delete a held version's folder once nothing runs from it;
the hold outlasts the folder. The hold covers the release store: a window that Setup installed
above the store's active version still starts its own Session, as it does today. Rejected: letting
an explicit install clear the hold. The npm launcher installs its carried version on every start,
so it would need a signal of its own to keep from reinstalling the failed version in a loop, and a
race between its pointer read and a fallback would remain.

#### Restoring records after a fallback

While the active version's `move` carries `restore`, a process that writes a record restores it
from the held versions' upgrade folders when it finds the record in a newer format, as the
[data root layout](data-root-layout.md#persisted-data-versions-and-migration) states. The next move
writes a `move` without `restore`, which ends restoring, so a later downgrade the person chooses
never brings back an old copy.

#### Where the code lives

- `crates/release-records`: the `fallback` record's format and its pure transitions, which the
  Rust launcher also reads.
- `crates/local-runtime`: the recorder behind `install.lock`, which the helper, the activator, the
  TUI, and the Server share. It sits beside the store's read side, which thereby owns these writes
  besides the Client's release store. The restore lives in the migration module.
- `apps/orchbee`: the completion's refusal and verdict, the manifest reader keeping
  `securityFloor` instead of dropping it after validation, `decide` and `install` for held
  versions, the `version-held` code, and the helper mode.
- The activator, crash recovery in the Session catalog, and the Server's watch of its Worker call
  the recorder; both launchers and Setup change as above.

#### Model

Chosen: the record's transitions get a TLA+ model, registered with the code that first writes the
record.

- Claim: under any interleaving of the writers above, with lock-free decisions between them and a
  crash at any point, no recorded start or failure is lost, no write undoes a move, a failure
  counts only toward the move it belongs to, and no fallback goes below a floor, to a release
  before slice 1, outside its window, twice in one move, or into a held version. Implementation
  tests check each writer's own steps; they cannot enumerate these interleavings.
- Files: `crates/local-runtime/formal/tla/ReleaseStoreFallback.tla` with one configuration,
  `ReleaseStoreFallback.cfg`.
- Constants: `Versions = {0, 1, 2, 3}`, ordered, where 0 stands for a release before slice 1 or a
  build whose switch is off; `Floor`, 2 for version 3 and 0 otherwise; `Gens = {1, 2, 3}`;
  `Writers = {w1, w2}`; `Threshold = 2`, standing for three; `Window = 1`; `MaxTime = 2`;
  `MaxMoves = 3`; and seven flags, each TRUE in the configuration.
- Flags, each a guard that its negative control below turns off:
  - `LockedUpdate`: a commit computes its write from the pointer it reads under the lock, not from
    `snap`;
  - `BindRecord`: a record counts only toward a counting `move` whose entry is its source version;
  - `HoldInSameWrite`: the fallback holds the failed version in its own write, not in a later
    `HoldLater`;
  - `SkipHeld`: `Offer` skips a held version, and every completion refuses one;
  - `AttributeStarts`: a Session failure counts only for a generation in `starts`;
  - `ProofRequired`: `back` needs the record's `move` to count for the replaced entry;
  - `FloorChecked`: `back` needs the replaced version to be at or above the target's `Floor`.
- Variables: `active`; `previous`, a version or none; `disk`, the version folders present; `rec`,
  the record, with `held` and a `move` holding its entry, `back`, each role's `since`, the Client
  `failures`, and the Session `starts` and `failed`; `lock`, a writer or none; per writer `pc`
  (idle, waiting, or holding), `op` (a move to a version, a Client start or failure of a version,
  or a Session start or failure of a generation), and `snap`, the pointer and record read before
  the lock, used only when `LockedUpdate` is FALSE; `now`, in `0..MaxTime`; `gver`, the version
  that started each generation, or none; and `moves`, the moves that `CommitMove` and
  `LegacyInstall` made. `Offer`, `ExplicitInstall`, and `LegacyInstall` are enabled only while
  `moves` is below `MaxMoves`.
- Ghost variables: `inc`, raised by every fresh `move`; `hist`, each fallback's failed version
  that lies above its target, from which every change of `active` drops only the versions below
  the new active one, so that a move into a held version stays visible; the Session starts and the
  failures committed under the current `inc`, with each failure's source version; and a log of
  fallbacks with source, target, time, role, and `inc`.
- Initial state: version 0 active with no `previous`, `disk = {0}`, no record, every writer idle,
  `now = 0`, and no generation started.

| Action                           | Stands for                                                                                                                                                                                      |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Offer(w, v)`                    | The update engine's `decide`: an idle writer reads the pointer without the lock and, when `v` is above `active` and not held, queues a move to `v`                                              |
| `ExplicitInstall(w, v)`          | Setup, the npm launcher, or a channel switch: queues a move to any `v` from 1                                                                                                                   |
| `LegacyInstall(w)`               | A completion of version 0, in one step while the lock is free: makes 0 active with the replaced entry as `previous`, and keeps the record verbatim                                              |
| `StartClient(w)`                 | A TUI start: queues a Client start of the active version                                                                                                                                        |
| `FailClient(w)`                  | A launcher's helper call: queues a Client failure of the active version                                                                                                                         |
| `StartGeneration(w, g)`          | The activator: queues the start notice of a new generation `g` of the active version; `gver[g]` is set once the notice is committed or dropped, never after a crash, and only then can `g` fail |
| `FailGeneration(w, g)`           | The activator, crash recovery, or the Server: queues a failure of a started generation; several observers may report the same one                                                               |
| `Acquire(w)`, `Drop(w)`          | Taking the free lock; ending a wait at its bound, which loses the record                                                                                                                        |
| `CommitMove(w)`, `RefuseHeld(w)` | The completion, as in [the move](#the-move); the replaced version's folder in `disk` stands for its intact `release.json`                                                                       |
| `CommitRecord(w)`                | A recorder's write below the threshold, or its release of the lock without a write when the check under the lock fails                                                                          |
| `CommitFallback(w)`              | The write with a role's third failure, as in [the fallback](#the-fallback)                                                                                                                      |
| `HoldLater(w)`                   | Enabled only when `HoldInSameWrite` is FALSE: the writer that committed a fallback takes the lock again and holds the failed version in a write of its own                                      |
| `Crash(w)`                       | A waiting or holding writer stops; the lock is released, and nothing it had not written survives                                                                                                |
| `Cleanup`                        | The cleanup after an install: deletes a folder that is neither `active` nor `previous`                                                                                                          |
| `Tick`                           | `now` advances while below `MaxTime`                                                                                                                                                            |

Every commit and refusal releases the lock.

| Property                                | Holds that                                                                                          | Negative control                              |
| --------------------------------------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `NoLostRecord`                          | Every start and failure committed under the current `inc` is still counted in `rec`                 | `LockedUpdate = FALSE`                        |
| `MoveNeverReverted`, an action property | Only `CommitMove`, `CommitFallback`, and `LegacyInstall` change `active` or `previous`              | `LockedUpdate = FALSE`                        |
| `RecordBound`                           | Every counted failure's source version is the `move` entry                                          | `BindRecord = FALSE`                          |
| `NoReturn`                              | `active` is not in `hist`                                                                           | `SkipHeld = FALSE`; `HoldInSameWrite = FALSE` |
| `Attribution`                           | `failed` is a subset of `starts`                                                                    | `AttributeStarts = FALSE`                     |
| `TargetProven`                          | No fallback goes to version 0                                                                       | `ProofRequired = FALSE`                       |
| `FallbackAboveFloor`                    | Every fallback's target is at or above its source's `Floor`                                         | `FloorChecked = FALSE`                        |
| `HeldNeverActive`                       | Neither `active` nor `previous` is held                                                             | None                                          |
| `BackImpliesPreviousOnDisk`             | A counting `move` with `back` true has `previous` on disk                                           | None                                          |
| `FallbackWithinWindow`                  | Every fallback happens before its role's `since` plus `Window`                                      | None                                          |
| `AtMostOncePerMove`                     | No two fallbacks share an `inc`                                                                     | None                                          |
| `EveryOpEnds`, progress                 | Every queued operation ends, under weak fairness of `Acquire`, `Drop`, and every commit and refusal | None                                          |

- Assumptions: every component carries the product version, as packaging writes it today; the
  model's clock never goes back, and the implementation ignores a failure dated before `since`; a
  crash releases the lock and leaves no partial write, as the flushed rename ensures; cleanup keeps
  `active` and `previous`; the completion runs in the target's binary; readers read whole files;
  the dark-launch switch is fixed per build, so version 0 stands for every build where it is off.
- Registration: id `release-store-fallback`, owner `orchbee-local-runtime`, workspace
  `crates/local-runtime`, claims `crates/local-runtime/README.md`, whose
  [release store reading](../../crates/local-runtime/README.md#release-store-reading) section then
  states the protocol; triggers `crates/release-records/src/**`,
  `crates/local-runtime/src/release_store*` and the recorder, `apps/orchbee/src/release_store/**`,
  `apps/orchbee/src/cli/update/**`, `apps/orchbee-launcher/src/**`,
  `apps/orchbee/npm/orchbee/lib/launcher.mjs`,
  `apps/orchbee/crates/workbench-client/src/local_activation*`,
  `apps/orchbee-server/src/local_worker_service/managed/stop_windows.rs`, and
  `crates/local-runtime/src/session_catalog/**`; required coverage: every action but `HoldLater`,
  which the configuration never enables.
- The registration adds seven configuration mutation cases, one per flag, updates the model and
  mutant counts in the [TLA tooling guide](../../scripts/tla/README.md) and in
  [verification](../../docs/VERIFICATION.md#change-triggered-tla-verification), and measures three
  complete fresh `pnpm test:tla` runs inside the 60-second budget. It may narrow the bounds above,
  stating the narrowed limits.

### Notify only in the worst case

- The person sees text only while an update waits under the backup plan: the existing notice "This
  update applies when no terminal is running." A failed check, install, or fallback shows nothing;
  the diagnostic log records it, and a held version is simply not offered.
- The only confirmation stays the
  [required Host replacement](versioning-and-upgrade.md#required-host-replacement) at a security
  floor: it lists the affected terminals, defaults to No, and refusing keeps the old Host. From H4 of
  [Host handover](host-handover.md#phases) it lists only the terminals that cannot move.

### Records follow the window

Chosen, which the owner may veto: from slice 1, Session and Client records, the rows the
[data root layout](data-root-layout.md#persisted-data-versions-and-migration) marks Migrated, read
forward from every format since slice 1, by migration on read. An idle move or a restart then never
resets the layout, even across skipped releases. The migration module already exists without a
step; from slice 1 a format change adds a step instead of changing a record in place, steps are
never removed, and a stored sample of each released format must keep reading, as frozen generation
fixtures must.

## How the window updates

Decided: the window updates too, from the window self-update release, which follows slice 1 under
the owner's release-often rule; until then the window keeps today's Setup path. Chosen: each window
version gets its own directory, behind a small launcher at the path every shortcut already names.
Citations name today's code; `orchbee.iss` is `apps/desktop/installer/windows/orchbee.iss` and
`entry.ts` is `apps/desktop/src/main/composition/entry.ts`.

```text
<store root>\desktop\
├── orchbee-desktop.exe      the window entry: a copy of the active version's window launcher
├── active.json              the window pointer
├── install.lock             the window writer lock
└── versions\<version>\      one window version: the packaged window directory
    ├── orchbee-desktop.exe  the window
    ├── desktop-files.json   the window file list
    └── resources\cohort\    its bundled release directory, which holds the window launcher
```

- The window version and the runtime version move separately; any pair that shares a connection
  generation works together, because [versions coexist](versioning-and-upgrade.md#outcomes).
- The launcher's switches `--relaunch-after-v1`, `--fallback-from-v1`, `--select-terminal-v1`, and
  `--select-workspace-v1` are a contract between builds, like the helper modes: a change adds a new
  name, so every launcher starts every window version from the window self-update release on.
- The uninstall registration keeps the version of the last Setup run; nothing rewrites it.
- macOS keeps the same `desktop/` folder under its store root, with one application bundle per
  window version, `desktop/versions/<version>/<name>.app`, and the same pointer. The installed
  bundle is the entry: a window that switches starts the launcher in the new version's bundle with
  `--replace-application-v1=<its bundle>`, which replaces that bundle in place once the window has
  ended, as Squirrel.Mac does, or installs into `~/Applications` when its folder cannot be written,
  and leaves the Session running ([the launcher](../../apps/orchbee-desktop-launcher/README.md#macos)).
  The window's bundled Client hands its Session start to the store as on Windows, so the swap
  changes no program the Session starts later; with no active version it first installs its own,
  as Setup would. The macOS window archive, its file list, the window
  part and install.sh follow in [macOS window versions](#macos-window-versions). Linux follows when
  it ships.

### Window archive and file list

- Chosen: every window version carries `desktop-files.json` at its root:
  `{"schemaVersion": 1, "version", "files": [{"path", "size", "sha256"}], "extensions"?}`, listing
  every other file of the directory once by a `/`-separated path. It takes the release manifest's
  file rules: the release path grammar, uniqueness ignoring case, at most 4096 files, 512 MiB each,
  2 GiB in all, and 1 MiB for the list itself (`apps/orchbee/src/cli/update/manifest.rs:13-16`,
  `:100-110`; `apps/orchbee/src/cli/update/signature.rs:12`). A current test package passes: 126
  files, the longest path 90 characters, no links.
- `apps/desktop/scripts/package.mjs` writes the list from the final bytes, after the packager
  returns and before Setup is compiled (`package.mjs:193-234`), and refuses a link or a name outside
  the grammar. The release zips that same directory, list included, as
  `orchbee-desktop-<version>-windows-x86_64.zip` under one top-level folder, the shape the runtime
  archive has (`apps/orchbee/src/cli/update/archive.rs:27-30`); Setup carries the same directory.
- The feed names the archive in `extensions.desktop`, which version 1 readers skip
  (`apps/orchbee/src/cli/update/feed.rs:6-8`). It is a closed object of `version`,
  `"manifest": {"sha256"}`, `"archive": {"format": "zip", "url", "size", "sha256"}`, and an optional
  `extensions`, each bounded as the feed's own member of that name (`feed.rs:41-107`).
  `manifest.sha256` is the SHA-256 of the archive's `desktop-files.json`, so the feed signature
  covers every file of the window. A release whose window did not change may name an earlier window
  version here.
- After unpacking, the list must match that digest and name the offered version. The install then
  copies exactly the listed files, each checked by size and SHA-256, and refuses a listed path that
  is missing or not a regular file.
- Rejected: the archive digest alone, because Setup installs without an archive and a stored version
  must stay checkable on disk; a signature on the list, because the signed feed already binds it and
  Setup stands on the person's own download.

### Window pointer

- Chosen: `desktop\active.json` has the store pointer's grammar and its one parser, `Pointer` in
  `crates/release-records/src/pointer.rs:46-106`. `active.product` is the window version,
  `active.manifest` the SHA-256 of its `desktop-files.json`, so the digest binds every file of the
  directory, and `previous` is the version `activate` replaced. The module's doc gains this second
  meaning.
- The window step opens `desktop\` as a second store root through `ReleaseStore`
  (`apps/orchbee/src/release_store/store.rs:28-67`), which brings `desktop\install.lock`,
  `desktop\versions\`, the flushed pointer rename through `active.pending`, the removal of a crashed
  writer's `active.pending`, private stages, and publishing without replacing, under the store
  root's private rule (`store.rs:7-12`). It holds the store root's `install.lock` first, after
  `download.lock` when it downloads, so [one update runs at a time](#trust-and-disk).
- Readers take no lock. The launcher and the window read at most 16 KiB, as the command launcher
  does (`apps/orchbee-launcher/src/windows.rs:15`, `:52-61`).
- Rejected: binding the archive digest, which Setup lacks; binding the bundled
  `resources\cohort\release.json`, which leaves the window's own files unbound.

### Window launcher

- Chosen: its own application, `apps/orchbee-desktop-launcher`, a Windows GUI-subsystem binary that
  depends on `orchbee-release-records` and on `winsafe` with `kernel` and `user`, in the required
  crate shape (`docs/RUST.md:58-67`) and without `package.json` (`docs/APPLICATIONS.md:32`). Its
  `build.rs` writes a resource file holding `resources/brand/ico/orchbee.ico` into `OUT_DIR` and
  links it on Windows MSVC targets (`docs/RUST.md:93-94`), so the entry keeps the icon that the
  Start menu shortcut and the uninstall list take from it (`orchbee.iss:31`, `:79`).
- `scripts/stage-native-windows.mjs` stages it as `<stem>-desktop-launcher.exe` the way it stages
  `<stem>-launcher.exe`: no cohort role, listed by the release manifest like every file
  (`stage-native-windows.mjs:170-176`). `scripts/package-native-release.mjs` requires it
  (`:284-285`). Every window version therefore holds its own launcher under `resources\cohort\`.
- Rejected: a second binary of `apps/orchbee-launcher`, because a crate has one crate root
  (`docs/RUST.md:67`) and the two share only the pointer parser and two messages; a resource
  compiler crate, because a new dependency is a reserved decision (`docs/RUST.md:109-114`).

Its decision is one pure function of the pointer, the arguments, the parent check, and which
programs exist:

1. The launcher's root is its own folder. It reads `active.json` there and starts
   `versions\<active>\<its own file name>`, as the command launcher resolves its Client
   (`apps/orchbee-launcher/src/resolve.rs:18-37`).
2. When its first two arguments are `--relaunch-after-v1 <process id>`, it drops them. When that
   process is its parent, confirmed by start times as `ancestors` confirms a parent
   (`apps/orchbee/src/cli/update/uninstall/windows.rs:251-253`), it first waits up to 30 seconds for
   that process to end, because the window's single-instance lock would otherwise hand the new
   command line to the old window, which only comes forward (`entry.ts:380-401`). After 30 seconds
   it starts anyway: the second instance exits with status 0, and nothing breaks.
3. It passes the remaining arguments one by one and the environment unchanged, gives the window its
   version folder as working folder, so no window holds another version's folder, and starts no
   Job.
4. When the pointer names a `previous` version whose program exists, it sets the
   [launcher protocol](#tui) variable to its own process id and watches the window for 120 seconds.
   If the window cannot start, or ends within them with a non-zero status, it starts the previous
   version once, with the same arguments plus `--fallback-from-v1=<active version>` and without the
   variable, as [make before break](#make-before-break) asks. Without such a version it exits once
   the window has started.
5. When nothing starts, it shows the matching message of the command launcher
   (`apps/orchbee-launcher/src/localization.rs:29-50`) in a message box, in the Windows display
   language as Setup chooses its own (`orchbee.iss:44`), and exits with status 1.

- The launcher moves no pointer and records no Client failure: a window failure says nothing about
  the runtime, whose [fallback](#falling-back-after-a-failed-move) keeps its own count.
- Rejected: moving the window pointer back after a failure, because a window that cannot join this
  Session joins the next one, and a held window version would need the bound and the records of the
  runtime fallback.

### Installing a window version

- The window part runs in `update` and `--automatic-update-v1` when `desktop\` exists, after the
  runtime part ends. It installs a window version only when the feed has `extensions.desktop` and
  the store's active runtime version is not lower than that window version, so the window's bundled
  Client hands its Session start to the store
  (`apps/orchbee/docs/windows-cli-configuration.md:345`) and a failed runtime part leaves the
  window for the next run.
- It decides from the feed, the window pointer, and `desktop\versions\` alone, in the runtime's
  order (`apps/orchbee/src/cli/update/engine.rs:212-216`): the offered version is active or lower
  than the active one (up to date); it is stored with other list bytes (`version-conflict`); it is
  stored with the same list bytes (every file verified, then activated); otherwise it is
  downloaded.
- The download uses `.download\` after the runtime part has finished with it, and nothing is
  unpacked before size and SHA-256 match (`archive.rs:1-10`). Under the store root's `install.lock`
  and `desktop\install.lock`, the step checks the list, copies the listed files into a private
  `desktop\.stage-*` and flushes each, verifies each, publishes the stage as
  `desktop\versions\<version>` without replacing, and moves the window pointer with `activate`, as
  the runtime publish does (`apps/orchbee/src/cli/update/install.rs:220-302`). A running window is
  never touched.
- Then every run that finds `desktop\`, whatever its version step did, places the entry,
  [moves a flat layout](#moving-a-flat-layout) on, and [cleans up](#window-cleanup).
- Placing the entry: when the entry differs from the active version's
  `resources\cohort\<stem>-desktop-launcher.exe`, the step writes a flushed `desktop\.new-<nonce>`
  and renames it over the entry in one step. Windows refuses that rename while the entry runs,
  because it neither deletes nor overwrites a running executable
  (`apps/orchbee/src/cli/update/path_step.rs:90`); the step then removes its copy, and the next run
  tries again. An equal entry is left alone. The runtime's completion never touches `desktop\`, as
  the store never opens it (`store.rs:1-5`).
- The `orchbee.update` result gains an optional `desktop` member that carries the runtime members of
  the same meaning for the window part: `phase`, `releasePath`, `reused`, `activated`, `upToDate`,
  `releaseVersion`, `cleanupPending`, and `error` (`apps/orchbee/src/cli/update.rs:278-299`). It is
  absent when the run had no window part; an added optional member keeps the document's `version`
  (`docs/cli/output.md:54`). Text output adds one line from the copy below.
- Rejected: the PATH step's move aside (`path_step.rs:61-112`), because in a flat layout the entry
  is the window itself, and Electron starts its helper processes from the path it started from
  (Unverified; the rule stands either way).

### What Setup does

- Setup extracts the packaged window into `{tmp}\desktop` instead of `{app}\desktop`
  (`orchbee.iss:76`), so it writes no file under `desktop\` itself.
- Built (vetoable): Setup keeps no copy of the window switch. Before it installs, it extracts its
  bundled Client alone and runs it with `--installer-window-v1` from its temporary folder. A build
  that installs window versions answers `not-release-directory` with exit 4, because that Client
  sits in no window's bundled cohort there, and changes nothing; a build whose window updates are
  off refuses the word as unknown, and a development build answers `development-build`. Setup then
  installs as this section says only after the first answer, and otherwise writes the flat
  `desktop\` of earlier releases as before, with its move to `desktop.previous` and its two refusals.
  Release builds answer the first way from the window self-update release on, which sets the
  switch.
- After its files, the bundled Client in `{tmp}\desktop\resources\cohort\` runs
  `install --format json` as today (`orchbee.iss:255-274`) and then the new helper mode
  `orchbee --installer-window-v1`. That mode installs the window directory two levels above its own
  folder as the window part installs an unpacked archive, then places the entry, moves a flat layout
  on, and cleans up. It answers like `install`: exit 0, or exit 3 with `downgrade-refused` when a
  higher window version is active and stays active, which Setup accepts (`orchbee.iss:266-270`).
  It runs whatever `install` answered, because the window is what Setup installs; a store lower than
  the window only makes its bundled Client start the Session from the window's folder, as today
  (`windows-cli-configuration.md:345`), which no longer blocks a later install. Release
  distribution's helper table gains its row.
- Setup then shows at most one message: WindowFailed when the window step failed, with exit code
  11; otherwise CommandFailed when `install` failed, with exit code 10 as today
  (`orchbee.iss:70-71`, `:98-99`, `:298-303`), whose words stay true because the window is in place.
  An entry left for a later run because it runs is no failure.
- Setup refuses with WindowInUse (`orchbee.iss:68-69`) only while a flat window runs:
  `desktop\resources\app.asar`, which only a flat layout has, exists, and the entry, which is then
  the flat window's executable, is in use by Setup's own probe (`orchbee.iss:165-181`).
  SessionInUse, `SessionRunsFrom`, the move to `desktop.previous`, and its restore go
  (`orchbee.iss:66-67`, `:211-253`, `:276-296`): no file a process runs from is removed, so a
  Session started by an older window blocks nothing. A leftover `desktop.previous` goes once no
  program runs from it.
- Uninstall still asks the store's active version through `bin\` and falls back to the flat bundled
  Client (`orchbee.iss:310-331`), then to any window version's bundled Client.
  `--installer-uninstall-v1` already removes `desktop\` with every version
  (`windows-cli-configuration.md:355`).
- The Start menu shortcut, AppUserModelID `com.orchbee.desktop`, the uninstall registration and its
  icon, and Setup's Launch box keep naming `desktop\orchbee-desktop.exe` (`orchbee.iss:31`, `:79`,
  `:84`), so the identity does not change.
- Rejected: installing the window in Pascal, which would repeat the locks, the pointer replacement,
  and the file checks of the Rust step.

### Moving a flat layout

- A window installed by a release before the window self-update, such as 0.1.0-beta.3, keeps its
  files directly in `desktop\`. The first window step that finds them moves the layout in four
  steps. Each step is one rename or can resume, and the locks make a second run end as `busy`, so a
  crash, a power loss, or a second instance at any point leaves a state that still starts a window
  and that the next run continues from:
  1. Publish the first window version beside the flat files.
  2. Move the window pointer. The entry is still the flat window, which starts as before.
  3. Place the entry. Its refusal while the flat window runs is the check that the window quit; from
     here every start runs the active window version.
  4. Remove the leftovers, never a file a process runs from. The flat window's own files, everything
     outside `resources\cohort\` and the layout's names, go file by file after the check below,
     which passes at once after step 3. The flat bundled release directory `resources\cohort\` goes
     whole, as a window version goes in the cleanup, because a Session an older window started may
     still run from it (`orchbee.iss:211-224`).
- No process runs from a set of files when the shared process scan
  (`apps/orchbee/src/cli/update/running_programs/windows.rs:34-78`) finds no process whose program
  lies inside, and every `.exe`, `.dll`, and `.node` inside opens for writing, as Setup's probe
  checks (`orchbee.iss:165-209`). The second check catches what the scan cannot query (the same
  file, `:32-33`); a folder rename proves nothing, because Windows moves a folder while a program
  runs from it (`orchbee.iss:183-184`).
- The layout's names, which no step removes: `versions\`, `active.json`, `active.pending`,
  `install.lock`, the entry, and `.stage-*`, `.new-*`, and `.remove-*`. No packaged window file uses
  them.
- Who moves: Setup, once the flat window quits; `update` and the automatic helper, on any run that
  finds no flat window running. A person who uses only the window and never runs Setup may offer no
  such run, because the Server outlives the window only while a terminal or another Client keeps
  it; [decision 21](#decision-record) accepts that, and the release notes say to run Setup once.
- Rejected: a helper that waits for the flat window to quit, polling every 10 seconds for up to 55
  minutes, which would move that person at their next quit but is the migration tool decision 21
  declines.

### Window cleanup

- After each window step, under both locks, the [bounded disk rule](#trust-and-disk) removes window
  versions as it removes runtime versions: neither active nor previous, no process runs from it, and
  nothing refers to it. Each goes by a rename to `desktop\.remove-<nonce>` first, so `versions\`
  only ever holds whole versions, and then a deletion; whatever cannot be deleted stays for the next
  run. A crashed run's `.stage-*` and `.new-*` leftovers go the same way.

### macOS window versions

Proposed (vetoable), after Squirrel.Mac and Sparkle: an archive of the signed bundle, checked after
unpacking, and a bundle the launcher swaps at the click.

- A window version folder, `desktop/versions/<version>/`, holds `desktop-files.json` and the signed
  `<name>.app`. The list sits beside the bundle, because a file at a bundle's root lies outside its
  signature's seal, which `codesign --verify --strict` refuses.
- The window archive is `<stem>-desktop-<version>-macos-<arch>.zip`, a zip as on Windows and as
  Squirrel.Mac's update archive, under one top-level folder holding exactly those two entries. The
  system bsdtar writes it with `--no-mac-metadata`, so it carries no AppleDouble entries.
  `extensions.desktop` names it as on Windows.
- The macOS list adds three things that Windows lists never carry:
  - paths start with `<name>.app/`, and a part also admits spaces and parentheses, since Electron
    names its helper applications and frameworks so (`<name> Helper (GPU).app`,
    `Electron Framework.framework`);
  - a file its owner may run carries `"executable": true`, since the bundle holds many programs and
    the list is the installer's only record of modes;
  - the bundle's symbolic links, which its frameworks need for their signature, form the optional
    `links: [{"path", "target"}]` in path order. Each target is relative, stays inside the bundle,
    and names a listed file, a listed link or a folder holding listed entries.

  Paths stay unique ignoring case across files and links, at most 4096 entries in all.

- The window part installs it under `<store root>/desktop/` with the Windows part's locks, stage,
  checks and pointer. It makes each listed link, gives an executable file mode 0700 and every other
  file 0600, and never touches the application the person runs; the launcher swaps that at the click.
  The launcher's copy takes the access of the application it replaces, its group, owner where the
  system allows it, and folder mode, as Sparkle does, so only the store's copy stays private.
- The arrangement after each window step places and moves nothing on macOS. Since no window runs
  from a stored version and Orchbee starts every program it runs from one by absolute path, a
  window version or leftover goes once a complete process listing shows no process whose start path
  lies inside it; runtime versions keep the Windows rule, which macOS cannot tell, and stay.
- install.sh `--desktop` takes the window archive the feed names in place of the disk image, checks
  its size and SHA-256 as it checks the runtime archive, has the unpacked Client install it with
  `--installer-window-v1`, and copies the unpacked bundle into the Applications folder as it copied
  the disk image's, so the person's copy has the archive's modes. A release build whose window
  updates are not switched on answers the helper word as a usage error; its bundle is copied alone.
- Chosen: an application copied from the disk image plays Setup's part at its first start. While
  the store has no active runtime version, each Session start of its bundled Client first runs that
  Client's `install`, which installs the bundle's release directory, links the command with the
  PATH step as Setup does, and makes an empty
  window store, and then hands the start to the store as above. An install that fails leaves the
  start with the bundle's own cohort, as before, writes one log record and is tried again at the
  next start; nothing asks the person. The copy's own version is not stored as a window version: the
  image carries no list beside the bundle, a list made on the person's Mac would be bound to nothing
  the feed signed, and the launcher's fallback restores the bundle it replaced, so no stored copy is
  needed. Rejected: Desktop Main running the install, because the bundled `orchbee` a terminal finds
  would then start Sessions differently from the window.
- Rejected: a tar.gz like the runtime archive, because the release check, the feed rule and the
  archive reader would split by platform; the code signature in place of the list, because a stored
  version must stay checkable against the signed feed without Apple's certificate chain, which an ad
  hoc signature lacks.

### Taskbar and Jump List

- Chosen: no relaunch properties. The Start menu shortcut carries the AppUserModelID
  (`orchbee.iss:79`) that the window sets for its process (`entry.ts:78-83`), and Microsoft's
  [AppUserModelID guidance](https://learn.microsoft.com/en-us/windows/win32/shell/appids) says the
  shell then takes the command line, icon, and text from that shortcut; relaunch properties serve an
  ID set on one window. A pin made while the window runs from a version folder therefore starts the
  entry. The real window run saw the Start menu shortcut start the active window version through
  the entry; a pin itself is still unverified.
- The Jump List's Save Diagnostic Bundle task names `process.execPath` today (`entry.ts:260-271`); a
  window that runs from a version folder names the entry instead, so the task outlives that version.
- Rejected: Electron's per-window relaunch properties through `setAppDetails`, a second source of
  identity for nothing the shortcut does not already give.

### Negotiated capabilities in the window

- Chosen: `orchbee.server.start` (`apps/orchbee/docs/native-cli-query.md:1873-1878`) gains an
  optional `negotiatedCapabilities` member: the capability names both sides listed in the
  `client-server` handshake (`crates/connection-handshake/src/message.rs:35`), which
  `LocalClient::server_serves` answers one at a time today
  (`apps/orchbee/crates/local-client/src/local_client.rs:389-397`).
- Desktop Main already runs `server start` before each document's relay
  (`apps/desktop/src/main/server-connection/readServerStartAnswer.ts`). It strict-parses the
  member beside `restoreNotice` (`interpretServerStartAnswer.ts` in the same folder), and the
  window's Update reads it for the join check.
- Chosen when the window self-update release was built: no window action needs a capability yet.
  `prompt-gated-input` serves only the TUI's start in the focused Panel, which the window lacks;
  `agent-input-area` serves only the CLI's Agent read; `compatibility-window` serves Main's
  join check; and `stream-outcome-unknown` changes only what the page's decoder admits, which it
  may admit unseen because the relay's Client offers it and a Server sends that close only when
  its answer lists it. So the page gets no bootstrap member yet. The first window action that needs a
  capability adds the bootstrap member of the same name
  (`packages/desktop-client-protocol/src/desktop-server-connection-contract.ts`), null when the
  answer has none, and one table from capability to window actions, as the TUI's
  `session_capabilities.rs` does. The relay's own handshake joins the same Server with the same
  Client, so it negotiates the same names, and each document reads them anew.
- Landed with About's Check for updates, the first window action that needs one (`update-check`):
  the bootstrap's `negotiatedCapabilities` is null when the start answer named none, and a
  malformed list also reads as null rather than refusing the document. The table is
  `apps/web/src/workbench-client/server-workbench/serverSessionServes.ts`. It reads an unknown
  list as serving nothing, as the TUI reads an unknown answer for Check for updates, so the request
  never reaches a Server that has not listed the capability. The
  [window's settings](../../apps/web/docs/terminal-settings.md#about-and-check-for-updates) describe
  the action.
- Rejected: a first frame on the relay. Main and its bundled Client are an exact pair
  ([what stays exact](versioning-and-upgrade.md#what-stays-exact)), so it would be allowed, but it
  changes both ends of the page's stream, whose bytes Main keeps opaque
  (`readServerRestoreState.ts:9-11`).

### What the window takes from comparable updaters

- VS Code's user setup runs its own Inno Setup silently with `/update`. It installs beside the
  running copy, into `_` or, for a versioned update, a version folder with `new_`-prefixed
  executables, waits for the window by checking its mutex every second, and then runs
  `inno_updater`, which renames `Code.exe` to `old_Code.exe` and `new_Code.exe` to `Code.exe` and,
  with `--gc`, removes old versions
  ([`code.iss`](https://github.com/microsoft/vscode/blob/main/build/win32/code.iss),
  [inno-updater](https://github.com/microsoft/inno-updater)).
- Squirrel.Windows, which GitHub Desktop uses, keeps each version in its own `app-<version>` folder.
  Since 1.5.0 the shortcuts name a stub in the root that keeps the application's icon and version
  metadata and starts the newest version, because pinning was "tricky" with `Update.exe` as a
  trampoline ([release 1.5.0](https://github.com/Squirrel/Squirrel.Windows/releases/tag/1.5.0)).
- Taken: installing beside the running copy and switching after the window ends, from both; one
  folder per version behind a stub with the application's icon at the path every shortcut names,
  from Squirrel, which keeps shortcuts and pins valid; a cleanup that skips what runs and retries,
  from both.
- Not taken: a Setup run per update, renamed executables, mutexes, a separate `Update.exe`, and
  rewritten shortcuts, because the store already owns the locks, the pointer, and the checks, and an
  unchanged entry path needs no shortcut change.
- Rejected: running the downloaded Setup silently at the click, because the window stays away for
  the whole install and Setup's rule against running files ties updating to Inno Setup, which macOS
  and Linux lack; renaming the running window's files aside, because a half-swapped Electron tree
  breaks the window.

The launcher reuses the command launcher's two messages. The new copy is proposed for the owner's
review before it ships:

| Text                             | `en-US`                                                                  | `zh-CN`                                                  |
| -------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------- |
| Setup, WindowFailed              | The %1 window could not be installed:%n%n%2%n%nRun Setup again to retry. | 无法安装 %1 窗口程序：%n%n%2%n%n请重新运行安装程序重试。 |
| `update`, window installed       | Window updated to {version}                                              | 窗口程序已更新到 {version}                               |
| `update`, window already current | Window up to date: {version}                                             | 窗口程序已是最新版本：{version}                          |
| `update`, window part failed     | Window not updated: {reason}                                             | 窗口程序未更新：{reason}                                 |

## What a click does

A click switches the Client it is in; other open Clients keep their own control.

### TUI

1. The TUI checks that the active version can join its Session: the Session runs slice 1 or later,
   and the active manifest's `client` component lists the `client-server` generation this
   connection negotiated. Otherwise [the new Client cannot join](#when-the-new-client-cannot-join).
2. It restores the console and exits with status 75. Its place is already saved: the TUI keeps the
   [Windows startup preference](../../apps/orchbee/docs/native-context-selection.md#windows-startup-preference)
   current after every successful draw.
3. The launcher that started it, `bin\orchbee.exe` or the npm launcher, reads `active.json` again
   and starts the active version with the unchanged command line in the same console. The new TUI
   joins the same Session, and a bare start opens at the saved place.

- A launcher announces the protocol through `ORCHBEE_RELAUNCH_V1`, which carries its own process
  id, and the TUI uses status 75 only when that process is its parent, so a stale inherited variable
  does nothing. Status 75 lies outside the [CLI exit codes](../../docs/cli/output.md#exit-codes). A
  launcher relaunches at most three times in ten seconds; a fourth request ends it with 1, so 75
  never reaches the launcher's caller.
- When the new TUI fails to start or to join, the launcher starts the version it replaced, with the
  same command line, as [make before break](#make-before-break) requires. Chosen: the launcher
  tells a failed start or join by status alone. Under the same parent check, a TUI that cannot join
  its Session ends with status 76 in place of its startup failure 1, which the launcher returns as 1;
  no process starting, Rust's panic status 101, and a Windows error status other than a console
  exit count too. Every CLI exit code is a normal exit, so a command's ordinary failure never counts
  against a version. The [launcher](../../apps/orchbee-launcher/README.md#relaunch-protocol) gives
  the rule.
- A TUI started without such a launcher exits normally and prints one line: "Run orchbee to open
  version {version}."
- macOS and Linux replace the process in place when they ship, so they need no launcher protocol.
  A macOS TUI that no launcher started records its own failures through the same helper: a
  replacement that cannot start, a failed join that a launcher would read as status 76, and a
  panic. A crash or signal that ends the replaced process records nothing, as the npm launcher
  records nothing for a Client a signal ended.

### Window

1. The window checks the join with the new version itself. It runs the active window version's
   bundled Client, `desktop\versions\<active>\resources\cohort\<stem>.exe`, as
   `server start --format json` with its own Session arguments and environment, as it does before
   each document (`apps/desktop/src/main/server-connection/readServerRestoreState.ts:14-28`). The
   check passes when that start succeeds and its `negotiatedCapabilities` lists
   `compatibility-window` (`crates/connection-handshake/src/registry.rs:17-21`): the handshake
   proves a shared `client-server` generation, and the name proves a Session of slice 1 or later.
   Otherwise [the new Client cannot join](#when-the-new-client-cannot-join), and the diagnostic log
   records why.
2. It starts the window entry detached, so the entry outlives it, with `desktop\` as working folder
   and the arguments `--relaunch-after-v1 <its process id>` and its place, then quits the way Quit
   Orchbee, Keep Terminals Running does (`apps/desktop/docs/window-lifecycle.md:102-103`): the
   Session and every terminal keep running.
3. The launcher waits for it to end and starts the active window version, which joins the same
   Session, restores its bounds as every start does (`apps/desktop/docs/data-root.md:82-96`), and
   opens at the same place.

- The place: `--select-terminal-v1=<TerminalPresentationId>` names the displayed terminal, the one
  renderer reload recovery keeps; with none, `--select-workspace-v1=<WorkspaceId>` names the
  selected Workspace; with neither, nothing is passed. Desktop Main strict-parses them with the
  identity schemas (`packages/workbench-client-protocol/src/terminal-presentation-contract.ts:3-10`
  and `src/workbench-protocol/workbench-semantic-contract.ts:27-35` in the same package) and gives
  them to its first document only, as the bootstrap member `relaunchPlace`. The page projects a
  terminal the way reload recovery projects its record, selecting its Workspace, exact Tab, and
  focused Panel (`apps/web/docs/navigation-routing.md:81-89`), and opens a Workspace by its route; a
  target that no longer resolves changes nothing. Nothing is stored, so nothing is left to delete.
  The exclusion of recovery across a restart (`navigation-routing.md:94-95`,
  `apps/web/docs/publication-and-client-view.md:138-143`) narrows by this one case.
- Rejected: a hand-off record such as `internal/state/desktop-relaunch.json`, which would add a data
  root file, a writer, a reader, and a removal rule for a value that fits on a command line.
- An announced window: Desktop Main reads the [launcher protocol](#tui) variable and removes it from
  its environment at start, so nothing it starts inherits it. When the value is its parent's process
  id and its first `server start` answer within 90 seconds refuses with `session-too-old`
  (`apps/desktop/src/main/server-connection/interpretServerStartAnswer.ts:67-77`), it exits with
  status 1 instead of showing the refusal, and the launcher starts the previous window version. The
  window never uses status 75; its relaunch is step 2.
- A returning window: started with `--fallback-from-v1=<version>`, it offers no Update for that
  version while it runs, and a newer version as usual. Nothing is stored, so the next start through
  the entry tries the active version again.
- Ready, for the window: it runs from `desktop\versions\<its own version>\`, the window pointer's
  active version is higher by SemVer precedence, and that version is not the one
  `--fallback-from-v1` named. A window that runs from anywhere else, such as a flat layout or a test
  build's output folder, never shows the control.

### When the new Client cannot join

A newer Client joins an older Session inside the [window](#compatibility-counts-time): their
`client-server` generations overlap, and the newer Client checks each negotiated capability before
it uses one, so a missing one disables only its action. The TUI keeps one table from a capability
to the actions that need it, and such an action shows as unavailable with its reason, as the
[Start Agent picker](../../apps/orchbee/docs/start-agent-picker.md) does for this Panel. When the active version cannot join the
running Session, because that Session runs a release before slice 1 or one outside the window:

- Before P1b the click does not switch; the person keeps the Client they have, and a notice says
  "This update applies when no terminal is running." The Session then
  [moves by itself](#how-the-session-moves), and the next start opens the new version.
- From P1b the click first restarts the Session's Server and Worker by
  [make before break](#make-before-break), keeping every terminal, then switches the Client. A
  restart whose preconditions fail, or that fails before it commits, changes nothing and shows the
  same notice.
- The `initiator-too-new` message keeps naming a Session stop only until P1b replaces it with
  Restart Session; a release-build `server stop` already asks the person to type the Session's
  name.

## How the Session moves

Decided: the background Session moves to the new version by itself once no terminal runs.

- Slice 1: a Server older than the store's active version ends itself through its ordinary stop
  once all of these have held for 60 seconds: no terminal runs in the Session, no Client is
  connected, no update helper runs, and no operation is in flight. Nothing is left to end, so the
  stop asks nothing. The next Client to start finds no Session and starts the active version, which
  restores the layout from records that [read forward](#records-follow-the-window).
- The no-Client condition exists because nothing reconnects on its own yet: ending a Server under a
  connected window or TUI would leave it on a Disconnected page. A window hidden in the
  notification area quits when its last terminal ends, so the condition usually follows at once.
- The idle move is the only automatic Session change before P1, and it cannot stop a program,
  because it waits until none runs. Operations in flight include the plugin jobs and configured
  commands the Worker runs, which the Server [asks the Worker about](#decision-record) at the end
  of the minute; a query that fails counts as one running.
- P1b changes three things. Clients reconnect on their own, so the no-Client condition goes. A
  click restarts Server and Worker by [make before break](#make-before-break) when the Session's
  [tier](versioning-and-upgrade.md#restart-decision) is `recommended`, or `required` for Server and
  Worker, keeping every terminal. Only a Host that must be replaced asks for a confirmation.
  Automatic restarts while terminals run stay
  [out of scope](versioning-and-upgrade.md#out-of-scope).
- P1b restarts only when its preconditions hold, among them that the active version reads every
  Session record. From slice 1 that holds unless a record is damaged, because
  [records follow the window](#records-follow-the-window); otherwise the Session waits for the idle
  move.

## The Update control

The TUI comes first, and the window follows it under the
[common feature contract](../../docs/INTERFACE-DESIGN.md#common-feature-contract).

| State     | When                                                                                                               | TUI                                                                | Window                                            |
| --------- | ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------ | ------------------------------------------------- |
| Absent    | No version newer than this Client's own is active, or a development build (a held version is never the active one) | Nothing                                                            | Nothing                                           |
| Ready     | A newer version is active; for the window, a newer window version                                                  | `[ Update ]` ending the Settings row as a primary button           | A primary Update button ending the sidebar footer |
| Switching | After a click                                                                                                      | The TUI leaves at once and its new version opens; no pending state | The same                                          |
| Waiting   | The click cannot switch yet: the Session must move first, or a restart could not complete                          | One notice; the control stays                                      | The same, in the window's notice surface          |

- Place: at the right end of the Settings row, two blank cells before Help in the TUI, and at the
  right end of the window's sidebar footer, in line with the rows' end tools, with Settings at the
  start of the row in both. It takes the primary button style as the sidebar's one
  [ready-state chrome action](../../docs/INTERFACE-DESIGN.md#visual-roles-and-themes).
  Development builds, whose
  [environment marker](../../apps/orchbee/docs/tui-shell-layout.md#environment-marker) uses that
  row, never show the control. When the row is short, Help drops first and Update takes the row's
  end, then Update drops once it cannot keep one blank cell after Settings, and stays reachable in
  the action menu.
- Keyboard: the control invokes one Client Action, Update, which the action menu (`Ctrl+b ?`) lists
  while it is available. It has no default key.
- A failed background check, install, or fallback changes nothing on screen; the next hour retries,
  and `orchbee update` or the diagnostic bundle shows what went wrong.
- No operating-system notification and no badge elsewhere: the person sees the control where they
  already look for Settings.

| Text                   | `en-US`                                                                                       | `zh-CN`                                                |
| ---------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Control                | Update                                                                                        | 更新                                                   |
| Tooltip                | Switch to Orchbee {version}. Terminals keep running.                                          | 切换到 Orchbee {version}，终端继续运行。               |
| Waiting                | This update applies when no terminal is running.                                              | 此更新将在没有终端运行时生效。                         |
| TUI without a launcher | Run orchbee to open version {version}.                                                        | 运行 orchbee 打开 {version} 版本。                     |
| Setting                | Automatic updates                                                                             | 自动更新                                               |
| Setting description    | Download and install new versions in the background. Nothing restarts until you click Update. | 在后台下载并安装新版本。点击“更新”前不会重启任何内容。 |
| TUI description        | Installs in the background; nothing restarts until you choose Update.                         | 在后台下载并安装新版本。点击“更新”前不会重启任何内容。 |

The copy is proposed for the owner's review before it ships. The TUI description is shorter in
English because a Settings description fits 69 cells at 100 columns. The TUI tooltip needs one amendment
to [pointer tooltips](../../docs/tui/overlays-and-status.md#pointer-tooltips), whose action
content is a label and its key: an effect line for a control whose label cannot say what keeps
running. The action menu entry carries the same sentence, so it stays reachable without a pointer.

## Manual check

Settings' About category shows the Client's version, a Check for updates action, and the Update
control while it is Ready. The action adds no second update state, and a Client never starts
`orchbee update`:

- A press sends one Client-to-Server request, `workbench.update-check` with `start`, under the
  `update-check` capability. The Server wakes its scheduled job, which runs the same
  `--automatic-update-v1` helper with the same locks and records; a press while a run is queued or
  running joins that run. A requested run ignores `update.automatic`, since the person asked for
  it, and leaves the hourly grid unchanged.
- The Server answers at once; About reads the run with `read` while it shows. The answer is
  `checking`, `finished` with `up-to-date`, `installed` or `failed`, `idle`, `unavailable` where
  the Server runs no job (Linux, development), or a refusal. An installed version still reaches the
  Client through the store's files, so the Update control appears as it does after a scheduled run.
- Only the Client that asked reports the run's end. A failed scheduled run still changes nothing on
  screen.
- A Client whose Session lacks the capability shows the action unavailable, with the reason. Each
  Client also shows it unavailable in a development build, where no update writes its store, and
  on a saved machine's Session, whose store is not this computer's.
- The update model is unchanged: like a scheduled run, a requested one is the `Offer` path of the
  [release-store model](../../crates/local-runtime/README.md#fallback-after-a-failed-move); only
  `orchbee update` is the explicit install.

| Text                          | `en-US`                                                        | `zh-CN`                                      |
| ----------------------------- | -------------------------------------------------------------- | -------------------------------------------- |
| Category                      | About                                                          | 关于                                         |
| Version row                   | Version                                                        | 版本                                         |
| Build kind                    | Release build / Test build / Development build                 | 正式版 / 测试版 / 开发版                     |
| Updates row                   | Updates                                                        | 更新                                         |
| Action                        | Check for updates                                              | 检查更新                                     |
| Checking                      | Checking…                                                      | 正在检查…                                    |
| Idle                          | Orchbee also checks every hour while automatic updates are on. | 自动更新开启时，Orchbee 也会每小时检查一次。 |
| Up to date                    | Orchbee is up to date.                                         | Orchbee 已是最新版本。                       |
| Failed                        | Couldn't check for updates. Try again later.                   | 无法检查更新，请稍后再试。                   |
| Unavailable, development      | Development builds don't update.                               | 开发版不会更新。                             |
| Unavailable, older Session    | Available once the Session updates.                            | 会话更新后可用。                             |
| Unavailable, no release store | Not available for this installation.                           | 此安装不支持检查更新。                       |

## Settings and the command

- `update.automatic`: boolean, default `true`, Server scope, under General in both Settings pages,
  applied at once like every setting. The Server reads it before each run. Off stops the schedule
  and its downloads; a version installed by hand still shows the control. Development builds
  ignore it.
- `update.channel` stays file-only. Automatic runs follow the selected channel and never change it.
- `orchbee update` stays and, from the window self-update release on, also installs the window part
  when `desktop\` exists, so a manual update leaves no older window behind. The `orchbee.update`
  document reports the window part through added optional members.

## Cadence, cost, and the network

- One check per running Session per hour fetches two files of about 0.7 KB together from
  orchbee.com, which Cloudflare serves as static assets free and without a request limit;
  `run_worker_first` stays off for `/update/*`, because a Worker invocation counts against the free
  plan.
- Archives come from GitHub Releases only when a new version appears: 24.9 MB for the 0.1.0-beta.2
  runtime and, for window users from the window self-update release on, the window archive, about
  175 MB as a test package zips. The owner's release-often rule accepts that size; a release whose
  window did not change may later name the previous window archive. Nothing costs the project money.
- Offline or failing: nothing on screen, and the next hour retries. A cut download starts over,
  since neither resume nor retry is frozen.
- Proxy: the helper passes `HTTPS_PROXY`, `ALL_PROXY`, and `NO_PROXY` from the Server's environment
  and otherwise the per-user system proxy, as
  [release distribution](release-distribution.md#commands) already decided; slice 1 builds the
  system proxy read. Automatic configuration scripts are not read.
- The feed never installs a lower version. A channel pointed back after a bad release reads as up
  to date, and the next fix moves people forward.

## Trust and disk

Rule: each step reuses a store rule.

- Only signed archives install. The feed signature covers both archives' size and SHA-256, the
  runtime's `release.json` is verified again after unpacking, and its smoke check runs before
  completion.
- No downgrade from the feed, no channel change, no elevation, and no service. The one automatic
  move to a lower version is the bounded [fallback](#falling-back-after-a-failed-move), which
  installs nothing.
- One update at a time: `download.lock`, then `install.lock`. A Setup that meets the lock reports it
  as it does today.
- Bounded disk: after each install the helper deletes, under `install.lock`, every runtime and
  window version that is neither active nor previous, runs no process, and is referenced by no
  Session, Host, or Capsule. A store therefore holds the active and previous versions plus whatever
  still runs. Unpacked, a window version takes about 449 MB, as a test package does, and a runtime
  version about 60 MB (unmeasured).
- A switch never touches a running file, and a failed install leaves both pointers where they were.

## Phases

| Phase                                       | Content                                                                                                                                                                                                                                                                                                                                               | Size                                                             | When                                            |
| ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- | ----------------------------------------------- |
| Slice 1, the first automatic-update release | Everything above except the window and what names P1, including the release gate                                                                                                                                                                                                                                                                      | About 22–25 agent-days in five parallel streams, about two weeks | The first release it is ready for               |
| Window self-update release                  | [How the window updates](#how-the-window-updates), the window's control and relaunch, copy                                                                                                                                                                                                                                                            | About 9 agent-days, built beside slice 1, about one week         | The first release after slice 1 it is ready for |
| P1a                                         | [Host adoption](versioning-and-upgrade.md#host-adoption) with Worker-loss recovery                                                                                                                                                                                                                                                                    | 1.5–3 weeks after its decision run                               | Starts now, beside slice 1                      |
| P1b                                         | Server and Worker restart by [make before break](#make-before-break): preconditions, the standby Server, each Host's hand-over and its way back, the durable binding record, old Clients switching themselves, the fallback after a restart, and the gate's restart checks; the click moves the Session, and the idle move drops its Client condition | 2.5–3.5 weeks after its decision run                             | Right after P1a and slice 1                     |
| P1c                                         | Host rollover: one Worker brokers several Hosts, and new terminals start on the newest                                                                                                                                                                                                                                                                | About 1 week                                                     | After P1b                                       |
| Host handover                               | [Host handover](host-handover.md#phases), H0–H5 and B1: prototypes, the Keeper, recovery after a Host crash or hang, moves across versions, moves in updates, macOS, and the Worker's background programs in the Host                                                                                                                                 | About 9 weeks to Windows' H4                                     | H0 now; H3 after P1c; B1 right after P1b        |

The [release gate](#the-release-gate) is P2 of
[versioning and upgrade](versioning-and-upgrade.md#delivery-phases): slice 1 builds it, and P1b adds
its restart checks.

| Slice 1 stream | Content                                                                                                                                                                                                                                                                      | Size                 |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| Runtime        | Server schedule and helper mode, outcome record, cleanup of old versions, system proxy, the idle move, `update.automatic` in both Settings pages                                                                                                                             | About 7 agent-days   |
| TUI            | The control and its Waiting notice, its Action and tooltip amendment, the relaunch status and the fallback in both launchers, the join check, copy                                                                                                                           | About 4.5 agent-days |
| Compatibility  | Supersession dates in the generation registry and the frozen-fixture check; a missing capability makes only its actions unavailable; record steps and stored samples for every Session and Client record; the fallback with its bound, held versions, and the record restore | About 6 agent-days   |
| Release gate   | The feed-signing job and the gate feed, the probe, the path jobs, and `release-publish` waiting on them                                                                                                                                                                      | About 4.5 agent-days |
| Evidence       | One scripted Windows run between two test builds of slice 1, through an automatic install, a click, an idle move, and a forced fallback, which also rehearses the gate                                                                                                       | About 1.5 agent-days |

- Slice 1 lands dark: each of its behaviors that a release build would show or run consults the
  local runtime's one switch,
  [`automatic_updates_live`](../../crates/local-runtime/README.md#automatic-update-switch), which
  every test build answers true and a release build false until the change that completes slice 1
  turns it on.
- Under the owner's release-often rule, slice 1 carries the runtime and the TUI, and the window
  follows in its own release; neither waits for the other beyond the install order, runtime first.
- P1 does not ship with slice 1, because slice 1 already meets the non-stop rule: no automatic path
  touches a running terminal. Bundling would hold every automatic update back by four to six weeks.
  The cost of shipping slice 1 alone is that a Server, Worker, or Host fix reaches a person only
  once no terminal runs, while a Client fix reaches them at the next click.
- Make before break adds about 1.5 weeks to P1b. Its decision run settles how each resource with a
  single owner changes hands and comes back: a Host's one Worker session, the Session endpoint with
  its lifetime lock, and the Session store's writer lock.
- Host handover's prototypes start now instead of after P1: every later Host must speak the Keeper
  protocol and read the records of H1's release, and a Host released before H1 can never hand over
  its terminals. Until H4, a
  required Host replacement remains a confirmation that lists the terminals that restart, and
  declining keeps the old Host and its visible tier. From H2 a crash in output parsing, which today
  ends every program in that Host, costs only screens.
- macOS and Linux join when their release platforms do; macOS Host handover comes with H5.
