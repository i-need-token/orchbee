# Host Handover

| Field             | Value                                                                                                                                                                                                                                            |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Status            | Proposed on 2026-11-15 from the product owner's request of that day. Decided restates a product-owner decision. Chosen marks this page's own pick, which the owner may veto, and Rule marks a safety rule that follows from an accepted outcome. |
| Owner             | Maintainers of `apps/terminal-host`, `apps/orchbee-worker`, `apps/orchbee-server`, `crates/local-runtime`, and the release workflow                                                                                                              |
| Current authority | The Terminal Host [runtime model](../../apps/terminal-host/docs/runtime-model.md), [versioning and upgrade](versioning-and-upgrade.md), and [automatic updates](automatic-updates.md)                                                            |
| Exit condition    | Delete this proposal once H4 and H5 ship and each rule lives in the Terminal Host runtime model, the Worker reference, or the owning standard.                                                                                                   |

Host handover moves a running terminal from an older Terminal Host to a newer one: the program keeps
running as the same process, and its screen, history, and pending input arrive whole. The old Host
exits after its last terminal has moved. Host handover replaces the opt-in live handoff that P3 and
P4 of [versioning and upgrade](versioning-and-upgrade.md#delivery-phases) planned. A Host is still
never restarted, as [Host lifetime](versioning-and-upgrade.md#host-lifetime) requires; it is
replaced only by moving its terminals away.

## Outcomes

Decided by the product owner on 2026-11-15:

1. A Host change reaches running terminals by moving them to the new Host, as a rolling update does,
   never by waiting for their programs to end: an Agent may run, or restart itself, indefinitely.
2. A move stops and restarts nothing and loses no input or output. Output may pause for a moment, and
   a program at most waits on a full pipe.
3. Moving is the normal path: automatic and on by default, with no click and no confirmation.
4. A failed move leaves the terminal on its old Host, serving as before.
5. The rule covers every program Orchbee runs, including the background programs the Worker runs
   today. Chosen: this page extends the owner's rule to them.
6. The plan reaches a point where every update applies with every program running, including an
   update that changes the process holding the programs.

The owner's reason: there may never be a moment when everything on the old Host has finished, and
long-running services such as Nginx update without going down. The idle move of
[automatic updates](automatic-updates.md#how-the-session-moves) covers only the case in which
nothing runs.

## What binds a program to its Host

| Binding                     | Windows                                                                                                                                                                                                                                                                                                     | macOS and Linux                                                                                                                                                                                                                                                  | Can another process take it over                                                                                                                          |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The program tree's lifetime | The per-terminal Job ends every member when its last handle closes                                                                                                                                                                                                                                          | When the last descriptor of the PTY master closes, the session receives `SIGHUP`                                                                                                                                                                                 | Yes: a duplicated handle, or a master passed by `SCM_RIGHTS`, keeps it alive                                                                              |
| The terminal device         | OpenConsole, reached through the input and output pipes, the reference handle, which keeps the console open, and the signal pipe; when the last write handle of the signal pipe closes, OpenConsole sends every program the close event. An HPCON is the signal, reference, and OpenConsole process handles | The kernel PTY, whose modes and size travel with the master                                                                                                                                                                                                      | Yes: the pinned conpty.dll exports `ConptyPackPseudoConsole`, which builds an HPCON from those three handles in a process that did not create the console |
| The exit status             | Any holder of the process handle reads it                                                                                                                                                                                                                                                                   | Only the parent can wait for it; Linux `pidfd` and macOS `kqueue` see another process end, but not its status                                                                                                                                                    | Windows yes; macOS and Linux no, because parenthood cannot move                                                                                           |
| Safe signaling              | Through the Job                                                                                                                                                                                                                                                                                             | [THCR-006](../../apps/terminal-host/docs/runtime-model.md#stable-posix-process-identity-thcr-006) relies on the Host delaying its wait for the session leader it parents, so the leader's process id cannot be reused; FreeBSD makes the Host the subtree reaper | Windows yes; macOS and Linux only through the parent                                                                                                      |

- The old Host's exit alone should end nothing once another process holds those handles or
  descriptors. Windows ties no program to its creator, and prototype 1 measured that OpenConsole does
  not either: its only tie to its creator is the signal pipe, which a duplicate keeps open. On macOS and Linux the program's process group is orphaned from its start, because its
  session leader's parent, the Host, is outside the session, so the Host's exit sends no `SIGHUP`.
- The old Host drops a moved terminal by closing its own handles, never through its ordinary
  teardown: ending the Job ends every program in it at once, whoever else holds the Job, and
  `ClosePseudoConsole` waits for OpenConsole to exit in Windows releases before 11 24H2 and in older
  conpty releases. The pinned conpty's close only closes the caller's three handles, so while a
  duplicate is held elsewhere it ends nothing; prototype 1 measured both.
- Windows Terminal already drives consoles it did not create. When it is the default terminal, the
  console host that Windows starts passes the console's server handle and the program's pending
  connect message to OpenConsole, which hands the signal, reference, and process handles to Windows
  Terminal; Windows Terminal packs them with `ConptyPackPseudoConsole` and resizes and closes the
  console through the result. That handoff happens while the program's first connection is pending,
  though: OpenConsole cannot export a console's state, so a terminal keeps its OpenConsole for life.
- Orchbee ships conpty.dll and OpenConsole.exe from microsoft/terminal. Windows Terminal ships the
  same OpenConsole, and WezTerm and node-pty, through the `useConptyDll` option that VS Code exposes,
  ship the same pair. The pair speaks a private signal protocol, and OpenConsole sends every program
  the close event when it reads a signal it does not know. On 2025-06-25 microsoft/terminal#18976
  changed the clear signal from a bare 2-byte id to the id and a 2-byte `keepCursorRow`; resize,
  show-hide, and reparent did not change. The current pin and the previous pin, 1.24.260710001,
  both use the new format, so each drives the other's OpenConsole. Rule: a Host sends each
  OpenConsole only signals in that OpenConsole's own format, which the record names, and refuses to
  claim a terminal whose format it does not know.
- The three handles inside an HPCON follow the private layout of the pinned conpty.dll
  1.25.260930003, the same as in 1.24.260710001, so a new pin repeats prototype 1 before it is
  admitted.

Windows can therefore move everything by duplicating handles. macOS and Linux can move the master
but not parenthood: without a parent that stays, a moved terminal loses its exit code and
THCR-006's safe signaling. That decides the [Keeper](#the-keeper).

## What a crash or hang costs

The Host parses all program output with Ghostty, so a defect there reaches every terminal in the
Host. One is open: crafted output with 32 or more link URIs or styles whose hashes collide makes
Ghostty panic in `cloneRowGrowCapacity` in every build, and a bypass in the Host would have to copy
Ghostty's hash and probe layout, as the
[open record](../../apps/terminal-host/docs/runtime-model.md#pinned-ghostty-checks-that-program-output-reaches)
says. A fuzz run also hung once, using no processor time, near a ConPTY resize.

| Event                     | Without a split                                                                                                                                            | With the Keeper and recovery, from H2                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The Host crashes          | Every program in it ends: shells, Agents, builds. The next start restores only the layout                                                                  | Programs keep running and their output waits. Within seconds the Worker starts a Host that takes the terminals from the Keeper. OpenConsole writes nothing after a size change, as prototype 1 measured, so on every platform a full-screen program redraws after a size change, and a shell shows its next output. On Windows a console client attached to the program's console can read the visible rows, and H2's decision run decides whether recovery seeds the screen from them. History is lost |
| The Host hangs            | Every terminal in it freezes, and each program blocks once its output buffer fills. A stop cannot end the Host without force, and force ends every program | Once the Host has made no progress for 30 seconds, and only while the Keeper holds all of its terminals, the Worker ends it, and recovery follows as after a crash                                                                                                                                                                                                                                                                                                                                      |
| The new Host breaks again | —                                                                                                                                                          | The second failure recovers each terminal in a Host of its own, so the cause fails alone. Past 3 recoveries in 10 minutes it stays running in the Keeper, shown as Can't reconnect, until the person closes it                                                                                                                                                                                                                                                                                          |

- During recovery the affected terminals show P1a's Reconnecting state, and typed keys are refused,
  because the person cannot see what they would act on. Copy for one terminal past the cap goes to
  the owner before it ships.
- The split contains these defects; it does not fix them. The Host bypass and the upstream fix stay
  wanted, but they no longer decide whether a person keeps their programs.

## The Keeper

Chosen: each Session gets one small process, the Keeper, that holds what keeps programs alive and,
on macOS and Linux, is their parent. It never reads, writes, or parses terminal data; each Host still
reads and writes its terminals directly.

- **Windows.** When a Host creates a terminal, it duplicates into the Keeper the Job, the program and
  OpenConsole process handles, the signal and reference handles, and both pipe ends.
- **macOS and Linux.** The Keeper starts each program: the Host prepares the PTY and the launch; the
  Keeper forks, runs the child steps the Host runs today, and keeps a duplicate of the master. It
  reports the leader's exit status without reaping it, and reaps when the Host asks after
  containment. FreeBSD moves the reaper role, with `PROC_REAP_KILL`, into the Keeper.
- **Ownership.** The Keeper records the one Host that owns each terminal and arbitrates every change
  of owner, as [how a terminal moves](#how-a-terminal-moves) describes.
- **Release first.** Rule: a Host that ends a terminal has each Keeper holding it close its
  duplicates first, because OpenConsole and the PTY session see their end only when the last holder
  closes. Prototype 1 measured it: the program gets the close event when the last signal write
  handle closes, and OpenConsole exits when the last reference handle closes.
- **Life.** The Worker starts the Keeper from its own installation, outside its Job as it starts
  Hosts, records it in the Session record beside the Hosts, and adopts it as it adopts them. The
  Keeper admits only that Worker and the Hosts it registers, each verified by process id and
  creation time, and exits once it holds nothing and no Host is registered. A confirmed stop sends it
  `exit` after the Hosts.
- **Form.** It is a mode of the Terminal Host executable, which shares the platform layer and adds no
  executable name.
- **Additive.** Rule: its protocol, about a dozen messages, changes only by addition, as
  `worker-host` does, so every Host and Keeper inside the
  [compatibility window](versioning-and-upgrade.md#compatibility-window) work together.

| Ends                           | Effect                                                                                                                                                                                                                                                                                                                 |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The Keeper, while Hosts live   | Nothing stops: each Host still holds its own handles or master. The Worker starts a new Keeper and the Hosts deposit again; on macOS and Linux the new Keeper cannot parent running programs, whose exit codes become unknown, and a Host signals such a program only after checking that its start time still matches |
| A Host, while the Keeper lives | [Recovery](#what-a-crash-or-hang-costs)                                                                                                                                                                                                                                                                                |
| Both                           | Programs end, as a Host crash ends them today                                                                                                                                                                                                                                                                          |

A Keeper that stops answering is ended by the Worker and replaced, at the same cost.

| Variant              | Change                                                                                                                                     | Cost                                                                                              | Risk                                                                                                                                      | A crash or hang of the parsing process                                                                         |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| A. No split          | The old Host passes handles or masters straight to the new one                                                                             | Lowest                                                                                            | On macOS and Linux every moved terminal loses its exit code and safe signaling                                                            | Ends every program in that Host, as today                                                                      |
| B. Forwarding keeper | A resident process owns the PTYs and copies every byte to a restartable parsing process, as tmux's server and VS Code's ptyHost own theirs | Highest: a second data path with flow control, attachment, and back-pressure, and a copy per byte | Crafted output passes through the process that holds every program, and a framing layer for every byte is a large protocol to keep stable | Programs survive; a screen rebuilt by replaying recent raw output would replay the bytes that caused the crash |
| C. Holding keeper    | The Keeper above                                                                                                                           | One small process with an additive protocol                                                       | Its protocol must be right early, and on macOS and Linux it is [updated](#updating-the-keeper) by re-executing in place                   | Programs survive; screens come back as [recovery](#what-a-crash-or-hang-costs) describes                       |

No split removes the need to move the screen: whichever process parses output holds it, so replacing
that process means moving it. The split decides only who holds the programs, and variant C keeps
that holder out of the data path and small enough to change rarely, as systemd's file-descriptor
store keeps descriptors for services that restart.

### Updating the Keeper

The Keeper holds the programs, so it must change without ending them too. Docker keeps containers
running across a daemon restart because a containerd shim, their subreaper, holds each one and the
daemon reconnects to it at boot. But a running shim is never replaced, and Docker supports live
restore only across patch releases; a daemon that cannot reconnect leaves the containers to be
stopped by hand. A terminal that never ends would keep its first Keeper forever, so Chosen: from H4,
and H5 on macOS, the Worker replaces the Keeper at every update that changes the Terminal Host
executable, before it moves any terminal. Until then a running Keeper stays, and newer Hosts use it.

- **Windows.** The Worker starts the new Keeper; each Host deposits its terminals into it, as it does
  when it creates one; the new Keeper confirms that it holds them all; and the old Keeper closes its
  duplicates and exits. A failure at any step leaves the old Keeper in charge, and nothing is lost,
  because every holder keeps the same objects alive.
- **macOS and Linux.** Parenthood cannot move, so the Keeper re-executes itself in place, as
  systemd's `daemon-reexec` does: `execve` keeps the process id, the children, and every descriptor
  not marked close-on-exec. The Keeper writes its state, each terminal's owner, leader process id,
  and master descriptor, to an unlinked file; clears close-on-exec on that file, the masters, and its
  listening endpoint; and executes the new version's binary in Keeper mode. A child that ends
  meanwhile stays unreaped, so the new image finds its exit status. The Hosts and the Worker
  reconnect at once, as the sockets systemd listens on stay reachable while it re-executes.
- **A way back on macOS and Linux.** systemd discards its manager before it re-executes, so a failed
  start leaves PID 1 to a fallback shell or a freeze. The Keeper keeps its way back:
  - before anything else it runs the new binary as a child in a check mode that reads a copy of the
    state, and any failure keeps the old image;
  - an `execve` that returns an error leaves the old image running with its state intact;
  - a new image that cannot read the state executes the old version's binary, which stays installed
    beside it, with the same state;
  - only a crash of the new image right after `execve`, or a kernel failure past `execve`'s point of
    no return, ends the Keeper. Programs then keep running, because each Host holds its masters, and
    lose their exit codes, as after a Keeper crash.
- Moves and recoveries wait for a Keeper update to end; the programs stay held meanwhile. A Keeper
  that has not answered 10 seconds into its update counts as one that stopped answering.

## How a terminal moves

Preconditions: one Worker brokers both Hosts, as [Host rollover](versioning-and-upgrade.md#host-lifetime)
(P1c) allows; both are registered with the Session's Keeper; the old Host exports a record format the
new Host imports; and no Server and Worker restart or Keeper update is under way.

1. **Hold.** The Worker stops sending the terminal's input, resizes, and viewer changes, and holds
   them in order. More than 64 KiB of input, such as a large paste, aborts the move.
2. **Quiesce.** The old Host stops reading output, on Windows by cancelling its blocked read as
   `abortRead` does today. It applies every byte it has read, writes the input it has accepted, marks
   the terminal in the Keeper as quiesced for the new Host, and exports its record. Output now waits
   in the pipe or PTY buffer, and the program waits only if that buffer fills.
3. **Restore.** The Worker relays the record, which it never parses, to the new Host, which validates
   it and restores a terminal that reads nothing yet.
4. **Claim, the commit.** The new Host claims the terminal in the Keeper and receives its handles or
   master. It checks them against the record by program identity, Job membership, OpenConsole image
   and signal format, and pipe types, or by the master's session; rebuilds the pseudoconsole with
   `ConptyPackPseudoConsole`; and starts reading.
5. **Release.** The Worker sends the held input, resizes, and viewer changes to the new Host, gives
   each viewer a full frame, and reports the new Host terminal identity to the Server's binding
   record. The old Host then drops the terminal: it closes its own handles without
   `ClosePseudoConsole` and without ending the Job, and it exits after its last terminal, as a
   draining Host does.

- Rule: a claim succeeds only while the old Host holds the terminal quiesced for the claiming Host,
  or after the old Host has ended, and the old Host's resume succeeds only while no claim has. The
  Keeper applies one request at a time, so exactly one Host reads a terminal at any moment, whatever
  crashes.
- Abort: a failure before the claim, or 5 seconds after the quiesce, resumes the terminal on the old
  Host through the Keeper, at the Worker's request or, once the deadline passes, on the old Host's
  own. The Worker sends the held input there, and the new Host discards its import. Without the
  Keeper, the old Host resumes only once the Worker confirms that the new Host has not claimed.
- Held input is delivered: the Worker that accepted each key stays, so every key reaches exactly one
  Host once. A Server and Worker restart instead refuses input during its gap, because its Worker
  changes.
- Nothing resizes a terminal that no Host reads. While output is unread, OpenConsole's signal
  thread waits for the console, and each resize queues 6 bytes in the 4 KiB signal pipe, so the
  684th resize blocks its caller until output is read, with OpenConsole using no processor time, as
  prototype 1 measured. Today's Host resizes under the lock its reader takes around every read, so
  a blocked resize would stop the reader for good; this likely explains the fuzz hang near a resize,
  which H1 checks against the harness. Rule: a Host never resizes, clears, or signals under a lock
  its reader needs.
- Neither the program nor OpenConsole notices a move: the console and the kernel PTY stay where they
  are, and the screen moves exactly, so nothing has to redraw.
- When: once the Session runs the new pair, the Worker moves every terminal of an older Host, one at a
  time, each after 2 seconds without input or after 10 minutes at the latest. The first failure stops
  moves for that version, so a defect costs one paused and resumed terminal; the Worker tries again
  when a newer version becomes active.
- Pause: the target is well under a second from quiesce to claim; H0 measures it.

| Ends during a move | Before the claim                                                                               | After the claim                                                                        |
| ------------------ | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| The Worker         | The old Host resumes at its deadline, and P1a replaces the Worker                              | The new Host serves; P1a's replacement finds the terminal in the Host the Keeper names |
| The old Host       | The new Host may still claim: nothing was read after the export, so the record is exact        | Nothing changes                                                                        |
| The new Host       | The old Host resumes                                                                           | [Recovery](#what-a-crash-or-hang-costs)                                                |
| The Keeper         | No claim can succeed: the Worker has the new Host discard its import, and the old Host resumes | The new Host holds its own handles; the Worker starts a new Keeper                     |

## What a person sees

- **During a move**, nothing marks it. The terminal keeps its place, screen, scrollback, and program.
  Output pauses for the length of the move, well under a second by target, and at most 5 seconds when
  a move aborts.
- **Typed keys** are never lost. Keys typed during a move arrive once and in order when it ends; more
  than 64 KiB, such as a large paste, aborts the move, and the old Host receives them.
- **A failed move** leaves the terminal on its old Host, which keeps running and serving it; the next
  newer version tries again.
- **A Host crash or hang**, from H2, leaves the program running: the terminal shows Reconnecting for a
  few seconds, keys typed then are refused, and its scrollback is lost.
- **For contrast**, upgrading tmux across a protocol change means ending its server and every program
  in it; zellij reaches only sessions of its own client-server contract version and brings a session
  back by running its commands again once the person presses Enter.

## The terminal record

- An Orchbee envelope — magic, the Host handover format version, the exporting Host's product
  version, and checksummed sections — carries the pinned Ghostty binary snapshot, version 1. The
  snapshot holds the screens, history, styles, hyperlinks, and modes, and keeps the parser's
  unfinished bytes as a continuation record, so a sequence that the quiesce splits resumes exactly.
- The other sections carry every per-terminal field of the Host's
  [core data model](../../apps/terminal-host/docs/runtime-model.md#core-data-model) that is not a
  broker-session attachment, such as the title, the reported directory, sizes, the content revision,
  the ConPTY clear coordination, and a settled terminal's exit; H1's decision run lists them. On
  Windows a section names the terminal's OpenConsole build and its signal format.
  Attachments and viewer leases do not move; the Worker makes them again.
- Kitty images are not in Ghostty's snapshot. An image section, bounded by the Host's image storage
  limit, carries the images and placements of the
  [graphics projection](../../apps/terminal-host/docs/runtime-model.md#kitty-graphics-projection-model);
  until it exists, a terminal holding an image stays on its Host.
- Rule: a new Host imports every format that a Host inside the
  [compatibility window](versioning-and-upgrade.md#compatibility-window) exports. A format change adds
  a reader, no reader is removed while a Host that writes its format can run, and each released
  format keeps a stored sample that every later Host must import.
- Rule: Ghostty's version 1 reader accepts only version 1 and rejects unknown record tags, so a
  Ghostty pin that changes the snapshot version is admitted only with a reader for each version that
  a Host inside the window writes.
- Rule: a record that fails validation is refused and its terminal stays; an unknown optional section
  is skipped; nothing in a record can end a terminal. Inside the snapshot, Ghostty's own decoder
  degrades an invalid value instead of failing.
- A record restores the screen exactly, so a page near the open panic's condition moves as it is. If
  it breaks the new Host, recovery costs that Host's screens, not its programs.

## Background programs the Worker runs

Configured `shell` commands, plugin jobs, and the jobs plugin hooks start run under the Worker today,
only on Windows, each in a Job without kill-on-close
([plugin jobs](../../apps/orchbee-worker/src/plugin_jobs.rs),
[custom commands](../../apps/orchbee/docs/windows-custom-commands.md#invocation-ownership-model)).
The Worker's ordinary stop cancels them, a Worker crash leaves them running untracked, and the
Server cannot see whether a configured command still runs.

| Event                          | Until B1                                                                                                                                                                                                                                                                                                                           | From B1                                                 |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| Idle move, slice 1             | Waits while the Worker reports one running, through the read-only same-version query of [decision 22](automatic-updates.md#decision-record)                                                                                                                                                                                        | Unchanged                                               |
| Worker loss, P1a               | They keep running, and the replacement Worker does not know them                                                                                                                                                                                                                                                                   | The replacement lists and reaches them through the Host |
| Server and Worker restart, P1b | The check asks the old Server, whose own Worker answers the same query, and the restart waits while any runs; the drain closes admission and asks again, and a nonzero count aborts. The update stalls and nothing is cancelled. Bounded helpers, such as worktree and probe jobs, finish inside the drain as in-flight operations | The restart no longer waits for them                    |
| Host rollover, P1c             | Unaffected                                                                                                                                                                                                                                                                                                                         | New background programs start on the newest Host        |
| Host handover                  | Unaffected                                                                                                                                                                                                                                                                                                                         | They move with their Host, as terminals do              |

- B1 runs each background program as a Host-owned process without a terminal, through an additive
  `worker-host` capability. The Host starts it from the Worker's prepared command with null or
  captured output, 64 KiB per stream as today, holds its kill-on-close Job, deposits it in the
  Keeper once one runs, reports its exit status, and lists it in its inventory. The Worker keeps admission, the
  catalog, start confirmation, logs, and cancellation, as requests to the Host.
- Rejected: leaving the old Worker draining after a restart. Server and Worker are an exact pair, so
  the old Worker could report to no Server, and its programs might never finish. Also rejected: a
  keeper of their own, because the Host and its Keeper already give programs a lifetime independent
  of the Worker.
- Plugin jobs that come to macOS and Linux start in the Host from the first day.

## Which updates keep programs running

| From    | Client                                      | Server and Worker                          | Terminal Host and Keeper                                                                                 | The Worker's background programs                      |
| ------- | ------------------------------------------- | ------------------------------------------ | -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| Slice 1 | Applies at the click, with programs running | Waits for the idle move, when nothing runs | Waits for the idle move                                                                                  | Hold the idle move while one runs                     |
| P1b     | Unchanged                                   | Applies with every terminal running        | Unchanged                                                                                                | Hold the restart while one runs; nothing is cancelled |
| B1      | Unchanged                                   | Applies with every program running         | Unchanged                                                                                                | Run in the Host and hold no update                    |
| P1c     | Unchanged                                   | Unchanged                                  | Reaches new terminals; running ones stay on their old Host, which keeps serving them                     | Start on the newest Host                              |
| H4      | Unchanged                                   | Unchanged                                  | Applies with every program running: the Keeper is replaced, every terminal moves, and the old Host exits | Move with their Host                                  |

The rows are for Windows; macOS reaches the H4 row with H5. From then on, every update applies with
every program running, including one that changes the Terminal Host and the Keeper. What remains
ends no program:

- A terminal started by a Host released before H1, or before H5 on macOS, keeps that Host until its
  program ends, and a terminal whose record the new Host refuses stays until a newer version takes
  it. At a security floor the confirmation lists them.
- An update across more than the
  [compatibility window](versioning-and-upgrade.md#compatibility-window), possible only when
  automatic updates were off or failing that long, waits for the idle move, as it does today.
  Docker's live restore stops at patch releases; Orchbee's window spans at least 90 days.
- A terminal keeps the console it started with: on Windows its OpenConsole, and on macOS and Linux
  the kernel PTY. A conpty fix therefore reaches a running terminal only when it restarts, as in
  every product that ships OpenConsole; if a security floor ever names OpenConsole, the confirmation
  lists the terminals still using the old one.

## Comparable products

| Product                | What a new version takes over                                                                                                                                                                                                       | What waits or ends                                                                                     | Orchbee takes                                                                                                                             |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Nginx                  | The old master starts the new executable, whose workers accept on the same listening sockets; the old master keeps its sockets and can start workers again if the new version misbehaves                                            | Open connections finish on the old workers                                                             | Old and new side by side, with the old kept as the way back until the new has taken over                                                  |
| HAProxy                | The new process fetches listening sockets from the old one over a Unix socket by `SCM_RIGHTS`                                                                                                                                       | Old connections finish, bounded by `hard-stop-after`                                                   | Descriptor passing from the old process                                                                                                   |
| Envoy                  | Hot restart passes listening sockets and shares statistics, and refuses when the hot-restart version differs                                                                                                                        | Old connections drain for a set time                                                                   | A versioned transfer format that refuses rather than guesses                                                                              |
| systemd                | `daemon-reexec` serializes the manager, re-executes it in place, and deserializes it; a failed start leaves PID 1 to a fallback shell or a freeze. A service's file-descriptor store keeps descriptors across the service's restart | Nothing, if the new image starts                                                                       | Re-execution in place for the Keeper on macOS and Linux, with a check run and a way back; the descriptor store as the Keeper's model      |
| Docker with containerd | Live restore keeps containers running across a daemon restart: a shim holds each one, and the daemon reconnects to it at boot                                                                                                       | Live restore is supported only across patch releases, and a running shim is never replaced             | A small holder apart from the processes that manage it; Orchbee also replaces the holder, and keeps compatibility across the whole window |
| CRIU                   | Checkpoints and restores process trees, terminals included                                                                                                                                                                          | Needs elevated rights and Linux                                                                        | Nothing; [out of scope](versioning-and-upgrade.md#out-of-scope)                                                                           |
| IIS                    | Overlapped recycling: a new worker process takes new requests while the old one finishes its own                                                                                                                                    | The old one's requests past a time limit                                                               | Overlap of old and new                                                                                                                    |
| Windows Terminal       | As the default terminal, it receives consoles that other processes created, packs their handles into an HPCON, and resizes and closes them                                                                                          | A console keeps its OpenConsole once its program attaches                                              | Driving a console through handles from another process                                                                                    |
| VS Code                | ptyHost owns terminals apart from windows; a reload reattaches and restores serialized text                                                                                                                                         | Terminal processes, at an update                                                                       | A process that owns terminals apart from windows, and a serialized screen                                                                 |
| tmux, screen           | Each server owns its terminals and is never replaced while it runs; tmux rejects a client of another protocol version, and its changelog advised restarting the server after an upgrade that changed it                             | Every program, at that restart                                                                         | Nothing beyond the server model Orchbee has                                                                                               |
| zellij                 | A client reaches only sessions of its own client-server contract version, which names the socket directory; resurrection runs the saved commands again once the person presses Enter                                                | Running programs, when the contract changes or the session ends                                        | Nothing                                                                                                                                   |
| herdr                  | Opt-in, Unix-only live handoff: the old server starts an import server, passes PTY masters 64 per message with a versioned manifest, carries history as ANSI text, and resizes so programs redraw                                   | Nothing ends, but the new server is not the programs' parent, so their exit status is out of its reach | Descriptor batches under the platform limit, a versioned manifest, and rollback that ends the import process                              |
| Ghostty                | No server; closing the application ends its terminals                                                                                                                                                                               | Every program                                                                                          | Its binary terminal snapshot, which the record carries                                                                                    |
| Warp                   | Restarts to update and restores the layout (not checked against source)                                                                                                                                                             | Running programs                                                                                       | Nothing                                                                                                                                   |

None of them moves a live program and its exact screen between two versions of the process that owns
it, without elevated rights, on Windows and macOS, and by default. The servers above move listening
sockets and let connections drain; Orchbee's programs never drain, so Orchbee moves the connection
itself: the handles, the screen with its parser state, and, through the Keeper, the parent. Like
Docker it keeps a small holder apart, and unlike Docker it replaces that holder too.

## Riskiest unknowns and prototypes

| #   | Unknown                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Smallest prototype                                                                                                                                                                                      | Size           |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| 1   | Whether a program and its OpenConsole keep running after the Host that created the pseudoconsole exits without `ClosePseudoConsole` while another process holds duplicates; whether OpenConsole watches its creator; whether the holder can resize, clear, and close through a packed HPCON, and a resize repaints the visible screen; whether ending a terminal waits on a remaining duplicate or a resize waits while output is unread; whether a Host on the current pin drives an OpenConsole of the previous pin with that pin's signal format | Two small programs on the pinned bundle and a numbered-line probe                                                                                                                                       | 2 agent-days   |
| 2   | Whether the Ghostty snapshot restores exactly across two pins, and its size and time for the longest history the Host keeps                                                                                                                                                                                                                                                                                                                                                                                                                         | Encode a busy terminal, with wide glyphs, hyperlinks, the alternate screen, and a sequence split midway, with the current pin; decode it with that pin and the previous one; compare rows and time both | 1.5 agent-days |
| 3   | Whether a quiesce loses or repeats bytes, including a read that completes as it is cancelled                                                                                                                                                                                                                                                                                                                                                                                                                                                        | A probe printing a numbered line every millisecond through 1,000 quiesce-and-resume cycles                                                                                                              | 1 agent-day    |
| 4   | Whether a program the Keeper forks on macOS keeps its controlling terminal and gets no `SIGHUP` while its master passes between two Host processes; whether the Keeper reads its exit status without reaping; whether a Keeper re-executed in place keeps its children, its masters, and the status of a child that ended during `execve`; and whether killing it right after `execve` leaves the shell running                                                                                                                                     | A keeper, two stand-in Hosts, and a shell on a PTY                                                                                                                                                      | 2 agent-days   |

- Prototype 1 was the riskiest, because it rests on behavior ConPTY does not document. It has run:
  OpenConsole does not end with its creator, so on Windows the Keeper holds duplicates and does not
  create the pseudoconsole or the program.
- If prototype 2 finds a difference between pins, moves run only between Hosts with the same Ghostty
  pin until the difference has a reader.

### Prototype 1 result

Run on 2026-11-15 on Windows 11 build 26300 with the pinned conpty 1.25.260930003 and the previous
pin 1.24.260710001, whose package matched the hash in the pin record. A creator process created a
pseudoconsole and a program in a kill-on-close Job, duplicated the Job, the program and OpenConsole
process handles, the signal and reference handles, and both pipe ends into a holder, and then
exited or was terminated without `ClosePseudoConsole`. Every result is measured unless it says
inferred.

| Question                  | Result                                                                                                                                                                                                                                                                                                                                                                              |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Takeover                  | On both pins the program and OpenConsole kept running, and output flowed through the duplicated pipes, after the creator exited and after it was terminated. OpenConsole does not watch its creator; its source ties it to the creator only through the signal pipe                                                                                                                 |
| Numbered lines            | 8,000 lines at one per millisecond across the creator's end, with 1 second of output left unread: none lost or repeated in four runs. The program stalled while output was unread and resumed when the holder read                                                                                                                                                                  |
| Packed HPCON              | `ConptyPackPseudoConsole` in the holder returned a working HPCON: resize and clear applied, and closing through it gave the program the close event within 4 ms, ended it with `STATUS_CONTROL_C_EXIT`, and ended the output                                                                                                                                                        |
| Repaint                   | Neither OpenConsole wrote anything after a resize to a larger, smaller, or equal size. A helper attached to the program's console read the visible rows with `ReadConsoleOutputCharacterW`; on the current pin the attach also made OpenConsole write `ESC [ 6 n` and wait about 0.5 seconds for a reply. Colors through `ReadConsoleOutputW` are inferred                          |
| Close with a duplicate    | The creator's `ClosePseudoConsole` returned in 6 µs and ended nothing, and output kept flowing. The program got the close event only when the holder closed its signal duplicate, and OpenConsole exited only when the last reference handle closed                                                                                                                                 |
| Job termination           | `TerminateJobObject` ended the program at once with the given exit code, whoever else held the Job, and the program saw no event. OpenConsole stayed until the last reference handle closed                                                                                                                                                                                         |
| Resize with unread output | Each resize returned within 60 µs until 683 of them filled the signal pipe; the 684th blocked, with OpenConsole using no processor time, until the holder read output, and then all completed                                                                                                                                                                                       |
| Cross-pin signals         | A holder on the current pin resized and cleared an OpenConsole of the previous pin. Between the release tags v1.24.11911.0 and v1.25.2733.0 the signal code is identical; conpty only adds the creation flag `--ambiguousIsWide`                                                                                                                                                    |
| Foreign format            | Sent the pre-#18976 clear, each pinned OpenConsole waited, read the next signal's id as `keepCursorRow`, and closed every program at that signal's next field, as it closes them at once on an unknown id. An OpenConsole older than #18976 would read the new clear's `keepCursorRow` as the next id: 0 closes every program, and 1 misreads the stream (inferred from its source) |

- The Keeper fallback is not needed: Windows keeps the deposit design.
- Recovery on Windows gets no repaint from OpenConsole, as the
  [crash table](#what-a-crash-or-hang-costs) now says.
- The unread-output resize block becomes the rule in [how a terminal moves](#how-a-terminal-moves)
  and an H1 check of the fuzz hang.

## Phases

| Phase | Content                                                                                                                                                                                                                                                                                                                                                                 | Size                                                 | When                                                                                                                                        |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| H0    | The four prototypes                                                                                                                                                                                                                                                                                                                                                     | About 1 week, about 6.5 agent-days in parallel       | Now, beside slice 1 and P1a                                                                                                                 |
| H1    | The Keeper on Windows; the record, quiesce, export, import, and claim in the Host; the dispatch thread's progress in `query`; a model of the move under [modeling and correctness](../../docs/MODELING-AND-CORRECTNESS.md), whose invariants are one reader per terminal and a reader for every running terminal; a same-build move between two Host processes in tests | About 3 weeks after its decision run, in two streams | After H0; ships dark in the first release it is ready for: its Hosts can deposit, export, import, and claim, but no Keeper starts before H2 |
| H2    | Recovery: the Worker replaces a Host that crashed or hung, and the new Host takes its terminals from the Keeper; a Keeper that crashed or stopped answering is replaced, and the Hosts deposit again                                                                                                                                                                    | 1.5–2 weeks                                          | After H1; on as soon as it passes                                                                                                           |
| H3    | Moves across versions by the Worker: timing, the hold, rebinding, the Server's binding record, P1a's import for a terminal seen in two Hosts, the image section, and diagnostics                                                                                                                                                                                        | 2.5–3 weeks after its decision run                   | After H1 and P1c                                                                                                                            |
| H4    | Moves in updates: the [Keeper update](#updating-the-keeper) on Windows, the release gate's checks of both, the confirmation reduced to terminals that cannot move, status, and the switch turned on                                                                                                                                                                     | About 1.5 weeks                                      | After H2, H3, and P2                                                                                                                        |
| H5    | macOS: the Keeper as parent and its re-execution in place, claims by `SCM_RIGHTS` 64 descriptors per message, the quiesce, recovery, and the gate on macOS                                                                                                                                                                                                              | 3–4 weeks                                            | After the first macOS package, before macOS automatic updates                                                                               |
| B1    | [Background programs in the Host](#background-programs-the-worker-runs)                                                                                                                                                                                                                                                                                                 | 1.5–2 weeks                                          | Right after P1b                                                                                                                             |

- H1 ships as early as possible, because a Host released before it can never hand over its
  terminals: they keep the P1c behavior and wait for their programs to end, and a security floor
  still asks the confirmation. Every later Host must speak the Keeper protocol of that release, so
  its decision run reviews the protocol first. A Keeper without recovery would leave a crashed
  Host's programs running out of reach, so none starts before H2; on Windows a capable Host then
  deposits the terminals it already runs.
- H2 comes before moves because it does not need P1c and it removes the worst open incident: one
  crafted output stream ending every program in its Host.
- Windows' critical path is H0, H1, H3, and H4, about 9 weeks with their decision runs, later if
  P1c is. [Which updates keep programs running](#which-updates-keep-programs-running) gives what
  each phase delivers.
- Windows turns moves on at H4 without waiting for macOS, under the release-often rule. H5 is not on
  the first macOS package's critical path; it lands before macOS updates itself, so that every
  terminal those updates meet can move with its exit code. A macOS Host released before H5 behaves as
  a Windows Host released before H1. Linux joins with its release platform.
- Existing phases: slice 1 and its idle move do not change. P1a does not change; H2 and H3 reuse its
  adopt mode, kernel import, quarantine, and restart cap. P1b adds the
  [background-program check](#background-programs-the-worker-runs) until B1, and no move runs during
  a restart. P1c does not change and comes before H3. P3 becomes H0 and starts now instead of after
  P1. P4 becomes H1 to H5, neither optional nor opt-in.

## Acceptance

- **H1**, on real Windows in a test-owned Session: a same-build move of a terminal running a
  numbered-line probe, and of one running a full-screen program, keeps each process id, loses and
  repeats no number, and leaves the screen and history unchanged. A failure forced at every step
  leaves the terminal on its old Host, and ending the old Host after the quiesce, or the Keeper at
  any step, leaves the program running. The model catches a mutant that drops the claim's guard.
- **H2**: ending a Host by process id, or stopping its dispatch thread, brings every terminal back
  within seconds with its program still running; a fourth failure inside 10 minutes leaves the
  terminal running and shown as Can't reconnect.
- **H3 and H4**: with Claude Code or a long-running script in a terminal, an update that changes the
  Host replaces the Keeper, moves the terminal to the new Host, and the old Host and old Keeper exit;
  keys typed during the move arrive once and in order; ending the new Host right after a claim brings
  the program back on screen.
- **H5**: the same on macOS, and a moved program's exit code still reaches the terminal after the
  Keeper re-executes; ending the Keeper right after its re-execution leaves every program running.
- **B1**: a configured `shell` command running through a Server and Worker restart and a Host move
  keeps running and reports its completion.

## Decision record

| #   | Decision                                                                                                                                                                                                                                                                                                              | Reason                                                                                                                                                      | Status             |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ |
| 1   | Host handover is the normal way a Host change reaches running terminals: automatic, on by default, with no click or confirmation, and never waiting for programs to end; the plan reaches a point where every update, including one that changes the process holding the programs, applies with every program running | Agents may never end; updates must roll                                                                                                                     | Decided 2026-11-15 |
| 2   | A failed move leaves the terminal on its old Host, which keeps serving, and the Worker tries again when a newer version becomes active                                                                                                                                                                                | Non-stop outranks every other goal                                                                                                                          | Rule               |
| 3   | A per-Session Keeper holds what keeps programs alive and parents them on macOS and Linux; it never reads or writes terminal data                                                                                                                                                                                      | Parenthood cannot move, and a parsing defect must not end programs                                                                                          | Chosen             |
| 4   | The Keeper is a mode of the Terminal Host executable, started, recorded, and adopted by the Worker                                                                                                                                                                                                                    | It reuses the platform layer and the Host's start and adoption                                                                                              | Chosen             |
| 5   | The Keeper's protocol changes only by addition, so every Host and Keeper inside the window work together                                                                                                                                                                                                              | Hosts of several versions share one Keeper                                                                                                                  | Rule               |
| 6   | The commit is the new Host's claim in the Keeper, and a claim and a resume exclude each other                                                                                                                                                                                                                         | Exactly one Host reads a terminal, whatever crashes                                                                                                         | Rule               |
| 7   | The Worker holds a moving terminal's input, resizes, and viewer changes and delivers them once and in order; more than 64 KiB of input aborts the move and goes to the old Host                                                                                                                                       | No input is lost or replayed                                                                                                                                | Chosen             |
| 8   | The record carries Ghostty's snapshot version 1 in an Orchbee envelope; new Hosts read every format exported inside the window, and a Ghostty pin that changes the snapshot version needs those readers                                                                                                               | A move must never depend on two Hosts sharing one build                                                                                                     | Rule               |
| 9   | Terminals move one at a time after 2 seconds without input or after 10 minutes; a move aborts 5 seconds after its quiesce; the first failure stops moves for that version                                                                                                                                             | A defect costs one paused terminal, and typing is rarely interrupted                                                                                        | Chosen             |
| 10  | A Host that crashes, or makes no progress for 30 seconds while the Keeper holds its terminals, is replaced; its terminals come back under a cap of 3 recoveries in 10 minutes, a terminal that breaks its Host again is recovered alone, and keys typed during recovery are refused                                   | Contain a parsing defect to the terminal that caused it                                                                                                     | Chosen             |
| 11  | Recovery turns on as soon as it passes, before moves                                                                                                                                                                                                                                                                  | It removes the worst open incident and needs no P1c                                                                                                         | Chosen             |
| 12  | Windows turns moves on at H4 without waiting for macOS                                                                                                                                                                                                                                                                | Release often                                                                                                                                               | Chosen             |
| 13  | Until B1, a Server and Worker restart waits while the Worker runs a background program; B1 runs them in the Host                                                                                                                                                                                                      | Non-stop covers every program                                                                                                                               | Rule; B1 Chosen    |
| 14  | A terminal that cannot move keeps the P1c behavior and, at a security floor, the confirmation                                                                                                                                                                                                                         | The last resort stays explicit                                                                                                                              | Rule               |
| 15  | From H4, and H5 on macOS, the Worker replaces the Keeper at every update that changes the Terminal Host executable, before any move: on Windows by deposit into a new Keeper, on macOS and Linux by re-execution in place after a check run, with the old binary as the way back                                      | Every update must reach a terminal that never ends; Docker's shims never change and its live restore spans only patch releases                              | Chosen             |
| 16  | A Host sends each OpenConsole only signals in that OpenConsole's format, named in the record, and refuses to claim a terminal whose format it does not know                                                                                                                                                           | OpenConsole closes every program on a signal it does not know, and the clear signal changed in 2025; prototype 1 measured both pins, which share one format | Rule               |
| 17  | A terminal keeps the console it started with; if a security floor names OpenConsole, the confirmation lists the terminals still using the old one                                                                                                                                                                     | No product can move a console after its program attaches                                                                                                    | Rule               |

Nothing waits on the owner; each Chosen row stays open to a veto.
