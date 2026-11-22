# Host Handover

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Proposed on 2026-11-15 from the product owner's request of that day; H1's decision run decided [its design](#h1-design) the same day, its design challenge, R1, and probe P1 amended it the same day, and R1's second run amended it on 2026-11-16. Decided restates a product-owner decision. Chosen marks this page's own pick, which the owner may veto, and Rule marks a safety rule that follows from an accepted outcome. |
| Owner             | Maintainers of `apps/terminal-host`, `apps/orchbee-worker`, `apps/orchbee-server`, `crates/local-runtime`, and the release workflow                                                                                                                                                                                                                                                                                             |
| Current authority | The Terminal Host [runtime model](../../apps/terminal-host/docs/runtime-model.md), [versioning and upgrade](versioning-and-upgrade.md), and [automatic updates](automatic-updates.md)                                                                                                                                                                                                                                           |
| Exit condition    | Delete this proposal once H4 and H5 ship and each rule lives in the Terminal Host runtime model, the Worker reference, or the owning standard.                                                                                                                                                                                                                                                                                  |

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
says. A fuzz run also hung once, using no processor time, near a ConPTY resize; a resize blocked
under the reader's lock explains it, and the Host no longer signals under that lock.

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

- **Windows.** When a Host creates a terminal, the Keeper duplicates from it the Job, the program
  and OpenConsole process handles, the signal and reference handles, and both pipe ends, as
  [the Keeper protocol](#the-keeper-protocol) describes.
- **macOS and Linux.** The Keeper starts each program: the Host prepares the PTY and the launch; the
  Keeper forks, runs the child steps the Host runs today, and keeps a duplicate of the master. It
  reports the leader's exit status without reaping it, read with `waitid` and `WNOWAIT` as
  [prototype 4](#prototype-4-result) measured, and reaps when the Host asks after containment.
  FreeBSD moves the reaper role, with `PROC_REAP_KILL`, into the Keeper.
- **Ownership.** The Keeper records the one Host that owns each terminal and arbitrates every change
  of owner, as [how a terminal moves](#how-a-terminal-moves) describes.
- **Release first.** Rule: a Host that ends a terminal has each Keeper holding it close its
  duplicates first, because OpenConsole and the PTY session see their end only when the last holder
  closes. Prototype 1 measured it: the program gets the close event when the last signal write
  handle closes, and OpenConsole exits when the last reference handle closes.
- **Life.** The Worker starts the Keeper from its own installation, in the Session's Job placement
  as it starts Hosts, records it in the Session record beside the Hosts, and adopts it as it adopts
  them. The Keeper admits one Worker connection at a time, as a Host admits one broker session, and
  only the Hosts the Worker registers, each verified by process id and creation time. It exits once
  it has held nothing, with no Host registered and no Worker connected, for 30 seconds. A confirmed
  stop sends it `exit` after the Hosts. [The Keeper's life](#the-keepers-life) gives the details.
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
  meanwhile stays unreaped, so the new image finds its exit status, as prototype 4 measured. The
  Hosts and the Worker reconnect at once, as the sockets systemd listens on stay reachable while it
  re-executes; prototype 4 connected during every `execve` and got an answer within 8 ms.
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
- Moves and recoveries wait for a Keeper update to end; the programs stay held meanwhile. Rule: on
  macOS and Linux a Keeper update also waits until a live Host holds the master of every terminal,
  so it waits for a pending recovery, because a program whose master only the Keeper holds gets
  `SIGHUP` and ends if the new image crashes, as prototype 4 measured. A Keeper that has not
  answered 10 seconds into its update counts as one that stopped answering.

## How a terminal moves

Preconditions: one Worker brokers both Hosts, as [Host rollover](versioning-and-upgrade.md#host-lifetime)
(P1c) allows; both are registered with the Session's Keeper; the old Host exports a record format the
new Host imports; and no Server and Worker restart or Keeper update is under way.

1. **Hold.** The Worker stops sending the terminal's input, resizes, and viewer changes, and holds
   them in order. More than 64 KiB of input, such as a large paste, aborts the move.
2. **Quiesce.** The old Host finishes writing the input it has accepted, then stops reading output,
   on Windows by cancelling its blocked read as `abortRead` does today. The quiesce ends when the
   reader thread has stopped with its last batch applied, not when `abortRead` returns, because a
   read can complete with data despite the cancel, as [prototype 3](#prototype-3-result) measured;
   and the stop must be undoable, because an aborted move resumes reading in the same Host, while
   today's abort is final and ends the reader thread. The reader stops only after a batch that
   leaves the parser and the Host's scanners at ground, so no sequence is split; if none does within
   a second, the quiesce refuses as passing and the terminal stays on its old Host. The old Host then exports its record
   and marks the terminal in the Keeper as quiesced for the new Host. Output now waits in the pipe
   or PTY buffer, and the program waits only if that buffer fills.
3. **Restore.** The Worker relays the record, which it never parses, to the new Host, which validates
   it and restores a terminal that reads nothing yet.
4. **Claim, the commit.** The new Host takes the terminal's handles or master from the Keeper,
   checks them against the record by program identity, Job membership, OpenConsole build and signal
   format, and pipe types, or by the master's session, and rebuilds the pseudoconsole with
   `ConptyPackPseudoConsole`. Only then does it claim the terminal in the Keeper, and it starts
   reading once the Keeper confirms the claim. It must not read before the old reader has stopped:
   a duplicated read handle shares the pipe's file object, so a second read waits behind a blocked
   one, and the blocked read takes the next output.
5. **Release.** The Worker sends the held input, resizes, and viewer changes to the new Host, gives
   each viewer a full frame, and reports the new Host terminal identity to the Server's binding
   record. The old Host then drops the terminal: it closes its own handles without
   `ClosePseudoConsole` and without ending the Job, and it exits after its last terminal, as a
   draining Host does.

- Rule: a claim succeeds only while the terminal is quiesced for the claiming Host under the mark
  its record names, which stays true if the old Host ends after its mark, and the old Host's resume
  succeeds only while no claim has.
  The Keeper applies one request at a time, so exactly one Host reads a terminal at any moment,
  whatever crashes.
- Abort: a failure before the claim, or 5 seconds after the quiesce, resumes the terminal on the old
  Host through the Keeper, at the Worker's request or, once the deadline passes, on the old Host's
  own. The Worker sends the held input there, and the new Host discards its import. Without the
  Keeper's answer, or after a claim that the new Host never saw confirmed, the old Host resumes only
  once the Worker has ended the Keeper and confirms that the new Host discarded its import without
  seeing its claim confirmed.
- Held input is delivered: the Worker that accepted each key stays, so every key reaches exactly one
  Host once. A Server and Worker restart instead refuses input during its gap, because its Worker
  changes.
- Nothing resizes a terminal that no Host reads. While output is unread, OpenConsole's signal
  thread waits for the console, and each resize queues 6 bytes in the 4 KiB signal pipe, so the
  684th resize blocks its caller until output is read, with OpenConsole using no processor time, as
  prototype 1 measured. The Host used to resize under the lock its reader takes around every read,
  so a blocked resize stopped the reader for good, the likely cause of the fuzz hang near a resize.
  The rule that a Host never resizes, clears, or signals under a lock its reader takes is now in
  the runtime model's [ConPTY signals](../../apps/terminal-host/docs/runtime-model.md#conpty-signals).
- Neither the program nor OpenConsole notices a move: the console and the kernel PTY stay where they
  are, and the screen moves exactly, so nothing has to redraw.
- When: once the Session runs the new pair, the Worker moves every terminal of an older Host, one at a
  time, each after 2 seconds without input or after 10 minutes at the latest. The first failure stops
  moves for that version, so a defect costs one paused and resumed terminal; the Worker tries again
  when a newer version becomes active.
- Pause: the target is well under a second from quiesce to claim. At the default scrollback, encoding
  and decoding the snapshot take at most 10 and 20 milliseconds, as
  [prototype 2](#prototype-2-result) measured, and stopping the reader takes tens of microseconds
  and at most 6 milliseconds, as [prototype 3](#prototype-3-result) measured. S9 measured whole
  moves between two processes of the release build on Windows, from the quiesce request to the
  claim's answer: 46 to 63 milliseconds at the default scrollback with a full history, 8,996 rows
  at 120×40 in a 3.9 MiB record; 34 to 41 milliseconds during a flood at 80×24, which includes
  applying the last batch; and 646 to 684 milliseconds at 100 MB of history, 96,107 rows in a
  41 MiB record, of which the quiesce with its export takes about 170, reading the record from the
  old Host and writing it to the new one about 160, and the restore about 330.

| Ends during a move | Before the claim                                                                                               | After the claim                                                                                                                                                         |
| ------------------ | -------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The Worker         | The old Host resumes at its deadline, the new Host discards its import at its own, and P1a replaces the Worker | The new Host serves; P1a's replacement finds the terminal in the Host the Keeper names and drops the old Host's moved copy instead of quarantining it                   |
| The old Host       | The new Host may still claim: nothing was read after the export, so the record is exact                        | Nothing changes                                                                                                                                                         |
| The new Host       | The old Host resumes                                                                                           | [Recovery](#what-a-crash-or-hang-costs)                                                                                                                                 |
| The Keeper         | No claim can succeed: the Worker has the new Host discard its import, and the old Host resumes                 | The new Host holds its own handles; the Worker starts a new Keeper. A new Host that never saw its claim confirmed discards its import instead, and the old Host resumes |

## What a person sees

- **During a move**, nothing marks it. The terminal keeps its place, screen, scrollback, and program.
  Output pauses for the length of the move, well under a second by target: about 50 milliseconds at
  the default scrollback and 0.7 seconds at 100 MB of history, as S9 measured, and at most 5
  seconds when a move aborts.
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
  version, its Ghostty pin's package hash, and checksummed sections — carries the pinned Ghostty
  binary snapshot, version 1. The snapshot holds the screens, history, styles, hyperlinks, modes,
  cursors, title, and working directory, and has a continuation record for the parser's unfinished
  bytes, so a sequence that a quiesce splits could resume exactly. H1 splits none: its reader stops
  only at ground, as [the Host's side](#the-hosts-side) decides, so an H1 Host writes the
  continuation empty and its stream runs without Ghostty's continuation tracking; a later phase can
  add tracking without a new frame format.
- The other sections carry every per-terminal field of the Host's
  [core data model](../../apps/terminal-host/docs/runtime-model.md#core-data-model) that is not a
  broker-session attachment, such as the title, the reported directory, sizes, the content revision,
  the ConPTY clear coordination, and a settled terminal's exit;
  [the record's sections](#the-records-sections) list them. On Windows a section names the
  terminal's OpenConsole build and its signal format.
  Attachments and viewer leases do not move; the Worker makes them again.
- Kitty images are not in Ghostty's snapshot. An image section, bounded by the Host's image storage
  limit, carries the images and placements of the
  [graphics projection](../../apps/terminal-host/docs/runtime-model.md#kitty-graphics-projection-model);
  until it exists, a terminal holding an image stays on its Host.
- Rule: a new Host imports every format that a Host inside the
  [compatibility window](versioning-and-upgrade.md#compatibility-window) exports. A format change adds
  a reader, no reader is removed while a Host that writes its format can run, and each released
  format keeps a stored sample that every later Host must import. An import is checked by the
  restored state, not by bytes, because a re-encoded snapshot may number its styles differently.
- Rule: a Host imports a snapshot only from its own Ghostty pin, or from a pin that the
  [pin check](#prototype-2-result) admitted. Ghostty calls version 1 a work-in-progress format that
  it may still break without changing the version number, so the version alone does not identify a
  format. Its version 1 reader accepts only version 1 and rejects unknown record tags. A pin change
  that fails the check needs a reader for each pin a Host inside the window runs; until then,
  terminals on those Hosts keep the P1c behavior.
- Rule: a record that fails validation is refused and its terminal stays; an unknown optional section
  is skipped; nothing in a record can end a terminal. Inside the snapshot, Ghostty's own decoder
  degrades an invalid value instead of failing.
- A record restores the screen exactly, so a page near the open panic's condition moves as it is. If
  it breaks the new Host, recovery costs that Host's screens, not its programs.

## Background programs the Worker runs

Configured `shell` commands, plugin jobs, and the jobs plugin hooks start run under the Worker today:
on Windows each in a Job without kill-on-close, and on macOS each configured `shell` command in its
own process group
([plugin jobs](../../apps/orchbee-worker/src/plugin_jobs.rs),
[custom commands](../../apps/orchbee/docs/custom-commands.md#invocation-ownership-model)).
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
- Prototype 2 has run: the snapshot restores exactly within one pin, the previous pin has no
  snapshot at all, and Ghostty may still change version 1 without a new version number. Moves
  therefore run only between Hosts with the same Ghostty pin, or with pins that the pin check
  admitted.
- Prototype 3 has run on Windows: a quiesce loses and repeats no bytes, and a cancelled read never
  consumes bytes it does not return. The quiesce waits for the reader to stop, not for
  `abortRead`, and the claiming Host reads only after that stop.
- Prototype 4 has run on macOS: a program the Keeper forks keeps its controlling terminal and gets
  no `SIGHUP` while its master moves between Hosts; the Keeper reads exit statuses without reaping
  and keeps its children, masters, and their statuses across a re-execution in place; a Keeper
  killed right after `execve` leaves every program whose master a Host holds running; and the POSIX
  quiesce loses and repeats no bytes.
- Probe P1, which R1 asked for, has run on Windows: `DuplicateHandle` and the checks do not wait
  behind another process's blocked read, so a Host holds without parking its reader.

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
  and the Host's [ConPTY signals](../../apps/terminal-host/docs/runtime-model.md#conpty-signals)
  rule.

### Prototype 2 result

Run on 2026-11-15 on Windows 11 build 26300 in a ReleaseSafe probe on the pinned Ghostty
`9c3ec931`, and on the source of the previous pin `91f66da2`, whose package matched the hash in the
older pin record. The probe fed a 120×40 terminal through Ghostty's standard stream with
continuation tracking. On the primary screen it wrote history lines with styles, palette and RGB
colors, CJK, emoji, a grapheme cluster, explicit and implicit OSC 8 links, and soft wraps. It then
set a title, a working directory, cursor keys, bracketed paste, button-event mouse with SGR reports,
focus reports, modify-other-keys, a blinking bar cursor, a saved cursor, and a scroll region. On
the alternate screen it drew a full screen with its own scroll region, Kitty keyboard flags, cursor
style, saved cursor, and an active link. The fed bytes ended in an OSC 8 link cut midway through its
URI. Every result is measured unless it says inferred.

| Question         | Result                                                                                                                                                                                                                                                                                                                                                                        |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Same pin         | After decode, both screens matched in every row, cell, grapheme, width, resolved style, link URI and ID, and row flag, and in the modes, cursors, saved cursors, charsets, Kitty keyboard stack, palette, title, and directory, for all five terminals below. Only the dirty flags differed, which a restore resets by design                                                 |
| Bytes            | Re-encoding the restored terminal gave the same bytes in three of the five terminals. In the other two, one page numbered the same styles in a different order                                                                                                                                                                                                                |
| Resume parsing   | The continuation held the 33 unfinished bytes. Feeding the rest after the restore completed the link as on the source, and both terminals stayed identical through later output and the return to the primary screen. Cuts inside CSI, UTF-8, OSC, and DCS, after a lone ESC, and in a CSI holding a BEL resumed the same way; the BEL is left out, so it does not ring twice |
| Previous pin     | It has no snapshot code: no snapshot module, no continuation tracking, and no `ghostty_snapshot_*` exports, and it needs Zig 0.15. A Host on it can neither export nor import, so a terminal on that pin cannot move                                                                                                                                                          |
| Format stability | Ghostty's source calls version 1 a work-in-progress format that it intends to keep breaking until it promises binary compatibility, and counts mode bit positions and enum indices as part of the format. A later pin can therefore change version 1 without a new version number (from the source; no newer pin was tested)                                                  |
| Compression      | `compress(.full)` reports unsupported on Windows, so a Windows Host never compresses history. On macOS the encoder clones compressed pages, at a cost not measured                                                                                                                                                                                                            |

| Terminal                                 | History rows | Snapshot | Encode       | Decode       | Ready      |
| ---------------------------------------- | -----------: | -------: | ------------ | ------------ | ---------- |
| 120×40, screens only                     |            0 |   41 KiB | 0.06–0.07 ms | 1.8–1.9 ms   | 1.8–1.9 ms |
| 120×40, default 10 MB, styled lines      |        4,678 |  3.0 MiB | 4.5–5.4 ms   | 9.6–9.9 ms   | 1.8–2.0 ms |
| 120×40, default 10 MB, plain build lines |        8,892 | 0.75 MiB | 1.7–2.2 ms   | 8.5–10.6 ms  | 2.8–3.5 ms |
| 240×70, default 10 MB, styled lines      |        3,089 |  2.5 MiB | 6.3–9.2 ms   | 14.8–19.6 ms | 3.6–4.4 ms |
| 120×40, 100 MB, styled lines             |       50,609 |   32 MiB | 135 ms       | 207 ms       | 4.5 ms     |

Times are medians of seven repetitions, three at 100 MB, and the ranges span three probe runs. Decode
includes moving the result into an owned terminal. Ready is the time to Ghostty's READY marker, after
which the terminal can render while its history arrives.

- The pin check: a Ghostty pin change decodes the stored samples of every pin inside the window
  with the new pin and must restore identical state, compared as this probe compares it. A pin
  that passes is admitted for moves. A reader for a pin that fails would need that pin's version 1
  decoder built against the new pin's terminal and the old pin's Unicode width policy, since
  restored cells keep their old widths while new output takes the new ones (inferred).
- H1 parks the reader at ground instead of tracking continuations, as
  [the Host's side](#the-hosts-side) decides, so its records carry an empty continuation, and the
  resume-parsing result above serves the later phase that adds tracking. The Host wraps Ghostty's
  standard handler, for which Ghostty guarantees that replaying a continuation does no work; that
  phase proves that the wrapper keeps that guarantee, which stays inferred until then.
- At the default scrollback the record stays near 3 MiB and fits one 44 MiB transport record. The
  Host allows up to 1 GB. Above about 130 MB a snapshot would outgrow one transport record, and near
  1 GB it would take seconds (inferred from the 100 MB row). Chosen in H1: the whole record crosses
  before the claim in chunks of up to 4 MiB, up to 128 MiB; a larger one keeps its terminal on its
  Host, and claiming at READY with history afterwards is left to H3's decision run.

### Prototype 3 result

Run on 2026-11-15 on Windows 11 build 26300 with the pinned conpty 1.25.260930003 and with the
system ConHost. A driver held the output pipe, as the Keeper will, and gave a duplicated read handle
to each of two reader processes. The readers took turns. Each read as the Host's reader does, with
a 256 KiB `ReadFile` marked active under the reader's lock, and after a random delay stopped as
`abortRead` does: it set the abort, called `CancelSynchronousIo`, retried on `ERROR_NOT_FOUND`, and
waited for the read to leave. The other reader started only after the first had applied every byte
it read. A third of the delays were zero, a third under 200 µs, and a third under 3 ms. Each run had
20,000 turns, so each reader resumed 9,999 times after its own cancel. Every result is measured
unless it says inferred.

| Producer                                                                    |  Turns | Before the read started | Cancelled while blocked | Completed before the cancel landed | Completed with data despite the cancel | Lost or repeated      |
| --------------------------------------------------------------------------- | -----: | ----------------------: | ----------------------: | ---------------------------------: | -------------------------------------: | --------------------- |
| A child through the pinned conpty, a line every 1 ms or 100 µs, or flooding | 60,000 |                  20,613 |                  29,322 |                              8,833 |                                  1,232 | 0 of 648,793 lines    |
| The same child through the system ConHost                                   | 60,000 |                  20,247 |                  30,659 |                              7,928 |                                  1,166 | 0 of 573,961 lines    |
| A raw pipe, a counter stream in chunks every 20 µs or flooding              | 40,000 |                  19,450 |                   4,327 |                             14,985 |                                  1,238 | 0 of 4.15 GB, by byte |

| Question               | Result                                                                                                                                                                                                                                                                                                                                                                |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Loss or repeat         | Through both consoles the turns, joined in order, held every numbered line once and in order with no stray text, and the raw stream matched byte for byte                                                                                                                                                                                                             |
| Hidden consumption     | Every read that ended in `ERROR_OPERATION_ABORTED` returned 0 bytes, and the next reader, in the other process, got the very next byte. No read ended aborted without a cancel, and no cancel reached a later read. A cancelled read never consumes bytes it does not return: the pipe completes a read either with its data or cancelled with none                   |
| Completion past cancel | 3,636 of the 160,000 reads returned data after `CancelSynchronousIo` had succeeded, so bytes still arrive after the abort is requested and before the reader stops. 1,193 cancels came before `ReadFile` began, got `ERROR_NOT_FOUND`, and succeeded on a retry                                                                                                       |
| Stop time              | From the abort to the reader stopped with its last batch applied, the run medians were 6 to 22 µs, the 99th percentiles 46 to 82 µs, and the maximum 5.9 ms; `abortRead` alone returned at a median of at most 20 µs. The probe applied a batch by checking or logging it, not by parsing it with Ghostty                                                             |
| Two readers at once    | While one read was blocked, `PeekNamedPipe` and `ReadFile` through a duplicated handle waited behind it, because a duplicate shares the synchronous file object; the blocked read took the next byte and the second read the byte after. Tested in one process; [probe P1](#probe-p1-result) measured that a duplicate in another process shares the same file object |

- The quiesce waits for the reader thread to stop, as [step 2](#how-a-terminal-moves) now says. In
  the probe a reset flag and a parked reader thread resumed reading 159,984 times, and no cancel
  ever reached a later read; H1 parks the Host's reader thread, as
  [the Host's side](#the-hosts-side) says.
- The Host already maps an aborted read to 0 bytes, which the counts show is exact; H1 asserts that
  an aborted read reports no bytes.
- Recovery on Windows: a hung Host with a blocked read keeps taking output until it ends, so the
  Worker ends it before the new Host reads, and bytes it read but never applied are lost, as
  recovery already accepts (inferred).
- POSIX, from the code, and [prototype 4](#prototype-4-result) measured it on macOS.
  `PosixPty.read` polls the master and an abort pipe, returns before reading once the abort is set,
  and reads the master only after poll reports data, so nothing cancels a read in progress. A stop
  takes effect between reads, and a poll that reports both the abort and data leaves the data in
  the PTY buffer for the next reader, so a read that completes as it is cancelled cannot occur. The
  abort marker is never drained and `read_aborted` stays set, so the stop must become undoable there
  too. Two readers of one master would split its output, so the one-reader rule holds on POSIX as
  well (inferred).

### Prototype 4 result

Run on 2026-11-16 on macOS 14.8.7 on an M1 MacBook Air, beside another agent's test runs, with
probes in C built by `zig cc`. A Keeper process forked each program onto a PTY with the child steps
of `PosixPty.childSetup` (`setsid`, `TIOCSCTTY`, the standard streams, default signal dispositions)
and kept the master, and stand-in Host processes took masters from it by `SCM_RIGHTS`. The programs
were a witness and an interactive `zsh -f`, which ends on `SIGHUP`. The witness logs every signal
it receives and reports, every 2 ms or on request, its parent, whether `/dev/tty` opens, whether the
PTY is its session's controlling terminal, and whether its group is in the foreground. Every result
is measured unless it says inferred.

| Question                   | Result                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Handover                   | 1,000 handovers of the witness and 200 of zsh, a quarter each with the old Host exiting before the claim, exiting right after it, killed as the new Host claimed, or with no Keeper copy, so that for 30 ms after the old Host exited the master existed only in an unreceived message. No program got `SIGHUP` or lost its controlling terminal or foreground group, the witness's numbered lines ran on from each Host to the next with none lost or repeated, and zsh answered every Host. Closing the last copy at the end delivered `SIGHUP` and ended both, so the probe sees a hangup when one happens |
| Status without reaping     | 3,000 children ended by the exit codes 0, 1, 7, 42, and 255 and by `SIGTERM`, `SIGKILL`, `SIGHUP`, `SIGINT`, and `SIGUSR1`, half of them session leaders on a PTY. `waitid(P_PID, pid, WEXITED \| WNOWAIT)` returned the right status every time and again 1 ms later, the zombie stayed, and the later `waitpid` reaped the same status. A kqueue `EVFILT_PROC` filter registered before the exit returned the same status with `NOTE_EXITSTATUS` in all 2,000 cases; registered after the exit, it failed with `ESRCH` in all 1,000                                                                         |
| Wait hazards               | `waitid(P_ALL, …, WNOWAIT)` returned the same zombie on every call, so it cannot walk several ended children. With `SIGCHLD` ignored the kernel reaped at once, and `waitid` answered `ECHILD`                                                                                                                                                                                                                                                                                                                                                                                                                |
| Re-execution               | A Keeper re-executed itself 1,000 times in a chain, holding two witnesses and zsh. In every generation the new image found all three as its running children with the same masters by `ptsname`, the witnesses named it as their parent and kept their controlling terminal with no `SIGHUP`, zsh answered through its master, the open descriptors stayed at 9, and the last image reaped all three. From `execve` to the new image's `main` took a median 0.94 ms, a 99th percentile of 1.9 ms, and at most 6.4 ms                                                                                          |
| A child ending in `execve` | Each generation released a child to end just before `execve`. 593 ended between the call and the new `main`, 574 of them already zombies when it started; 157 ended after it; and 250 were killed with `SIGKILL` just before the call. The new image found and reaped the right status in all 1,000                                                                                                                                                                                                                                                                                                           |
| Reconnect                  | A Host connecting to the Keeper's listening socket as soon as `execve` began connected all 1,000 times, and the new image answered it in a median 1.2 ms and at most 8 ms                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Failed `execve`            | A missing binary returned `ENOENT` and a non-executable file `EACCES`, and the old image kept running with its children and masters                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Keeper killed              | In 300 runs a Host held the masters while the new image killed itself as its first instruction, or was killed with `SIGKILL` as `execve` began or once its `main` ran. All 900 programs kept running with parent 1, their controlling terminal, their foreground group, and no `SIGHUP`, and zsh answered through the Host's master. With no Host holding the masters, every witness in 10 runs got `SIGHUP` and ended                                                                                                                                                                                        |
| Quiesce                    | Two Host processes took turns reading one master as `PosixPty.read` does, stopped by the abort pipe, drained it to resume, and claimed the master again by `SCM_RIGHTS` every turn. In 22,000 turns of numbered lines every 1 ms or 100 µs or flooding through the line discipline, and of a raw counter stream in chunks every 20 µs or flooding, the joined turns held 16.4 million lines and 59.5 million counters, 673 MB, with no byte lost, repeated, or changed. In 4,559 stops poll reported the abort and data together, and the next Host got that data intact                                      |
| Stop time                  | From the abort to the reader stopped with its last batch applied, the run medians were 2 to 13 µs, the 99th percentiles 6 to 25 µs, and the maximum 59 µs. The probe applied a batch by checking it, not by parsing it with Ghostty                                                                                                                                                                                                                                                                                                                                                                           |
| Descriptors per message    | One message carried 64 distinct masters, received in order, and up to 254 descriptors; 255 or more failed `sendmsg` with `EINVAL`. A receiver short of free descriptor slots got `EMFILE`, and a retry with free slots received the message's bytes without any descriptors: they were lost                                                                                                                                                                                                                                                                                                                   |

- The Keeper design holds on macOS as written: the Keeper forks and parents the programs.
- Rule: the Keeper reads each leader's status with `waitid(P_PID, …, WEXITED | WNOWAIT)`, never with
  `P_ALL` or `waitpid(-1)`. A kqueue filter only wakes it. A registration that fails with `ESRCH`
  means the leader has ended, but its status may not be waitable yet, and a registered filter can
  miss `NOTE_EXIT`, as the Host's macOS wait worker in the
  [runtime model](../../apps/terminal-host/docs/runtime-model.md#terminal-lifecycle) records; so the
  Keeper runs that worker's bounded non-reaping `waitid` poll after `ESRCH` and at a bounded
  interval while it waits. It sets `SIGCHLD` to its default action when it starts, because an
  ignored `SIGCHLD` reaps at once and loses every status.
- A Keeper killed after `execve` leaves running only the programs whose master a Host holds, so a
  Keeper update waits for pending recoveries, as [updating the Keeper](#updating-the-keeper) now
  says. Their exit codes go to `launchd`, which reaps them (inferred).
- Rule: a claim counts the descriptors it receives against the record, and a short count fails the
  claim and leaves the terminal with its old Host. A Host claims only with free slots for every
  descriptor of the message. H5 keeps 64 descriptors per message, under the macOS limit of 254.
- The POSIX quiesce needs only the undoable stop that prototype 3 named: drain the abort pipe and
  clear `read_aborted`. A stop took at most 59 µs, against 5.9 ms on Windows.
- Not covered, and left to H5: Linux and FreeBSD, the product's Host and Keeper code, the check
  mode, bash, and a Keeper killed while a job in the shell is stopped. The leader's process group is
  orphaned from the start, because the Keeper is in another session, so the Keeper's death orphans
  no new group and sends no `SIGHUP` or `SIGCONT` (inferred).

### Probe P1 result

Run on 2026-11-15 on Windows 11 build 26300 with the pinned conpty 1.25.260930003, the system
ConHost, a bare anonymous pipe, and a named pipe whose reading end was opened for synchronous I/O. A
driver process, standing in for the Keeper and for a claiming Host, created each pipe and gave
duplicates of its read end, the Job, and the program to two processes: A read as the Host's reader
does, with a 256 KiB synchronous `ReadFile`, and K held them without reading, as the Keeper does at
a claim. Each call ran on a worker thread under a 100 ms watchdog, and a call that outlived it
counted as a wait. Each pipe kind ran 20,000 repetitions before A's first read and 20,000 while A's
read was blocked; no read completed during the blocked repetitions. Every result is measured unless
it says inferred.

| Question          | Result                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hold and claim    | While A's read was blocked, `DuplicateHandle` of the pipe, the Job, and the program out of A and of the pipe out of K, `NtQueryObject` with `ObjectTypeInformation` and `ObjectBasicInformation`, `IsProcessInJob`, `QueryInformationJobObject` for the extended limits, `GetProcessId`, `GetProcessTimes`, and `CloseHandle` of each duplicate never waited: 0 waits in 300,000 calls per pipe kind. Medians were 0.2 to 0.5 µs, and 2.6 to 2.8 µs for duplicating the program; the 99th percentiles were at most 5.4 µs and the maximum 71.5 µs, against 124.5 µs with A idle |
| Variants          | Duplicating with no access or only `SYNCHRONIZE`, the type and basic queries on the duplicate with no access, `ObjectHandleFlagInformation`, and `GetHandleInformation` never waited in 5,000 repetitions per pipe kind                                                                                                                                                                                                                                                                                                                                                         |
| Forbidden calls   | `PeekNamedPipe`, `GetNamedPipeInfo`, and `NtQueryObject` with `ObjectNameInformation`, also on a duplicate with no access, waited in all 3 trials on every pipe kind until A's read completed, so a duplicate in another process shares the synchronous file object. `GetFileType` never waited                                                                                                                                                                                                                                                                                 |
| Basic information | `ObjectBasicInformation` fails with `STATUS_INFO_LENGTH_MISMATCH` unless its length is exactly 56 bytes                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Type and access   | Every pipe reports the type `File`. A read end from `CreatePipe`, for a console or bare, grants `0x120189`; a named pipe client opened with `GENERIC_READ` grants `0x120089`                                                                                                                                                                                                                                                                                                                                                                                                    |

- The pull and the checks do not wait behind a blocked read, so a Host holds without parking its
  reader, and H2's recovery may pull and check a hung Host's handles before that Host has ended.
  The new Host still reads only after it has ended, because a blocked read keeps taking output, as
  [prototype 3](#prototype-3-result) found.
- The rule against pipe I/O before the commit stands, now measured across processes. `GetFileType`
  does not wait but stays out, since no check needs it.
- Not covered: a Host suspended or hung outside a read, a loaded machine, and other Windows builds.
  The calls that did not wait act on kernel objects and run no code in the source process, so a
  suspended Host would not change them (inferred).

## Phases

| Phase | Content                                                                                                                                                                                                                                                                                                                                                                                                                         | Size                                                 | When                                                                                                                                        |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| H0    | The four prototypes                                                                                                                                                                                                                                                                                                                                                                                                             | About 1 week, about 6.5 agent-days in parallel       | Now, beside slice 1 and P1a                                                                                                                 |
| H1    | The Keeper on Windows; the record, quiesce, export, import, and claim in the Host; the dispatch thread's progress in `query`; a model of the move under [modeling and correctness](../../docs/MODELING-AND-CORRECTNESS.md), whose invariants are one reader per terminal and a reader for every running terminal; a same-build move between two Host processes in tests; [H1 design](#h1-design) gives the decisions and slices | About 3 weeks after its decision run, in two streams | After H0; ships dark in the first release it is ready for: its Hosts can deposit, export, import, and claim, but no Keeper starts before H2 |
| H2    | Recovery: the Worker replaces a Host that crashed or hung, and the new Host takes its terminals from the Keeper; a Keeper that crashed or stopped answering is replaced, and the Hosts deposit again                                                                                                                                                                                                                            | 1.5–2 weeks                                          | After H1; on as soon as it passes                                                                                                           |
| H3    | Moves across versions by the Worker: timing, the hold, rebinding, the Server's binding record, P1a's import for a terminal seen in two Hosts, the image section, and diagnostics                                                                                                                                                                                                                                                | 2.5–3 weeks after its decision run                   | After H1 and P1c                                                                                                                            |
| H4    | Moves in updates: the [Keeper update](#updating-the-keeper) on Windows, the release gate's checks of both, the confirmation reduced to terminals that cannot move, status, and the switch turned on                                                                                                                                                                                                                             | About 1.5 weeks                                      | After H2, H3, and P2                                                                                                                        |
| H5    | macOS: the Keeper as parent and its re-execution in place, claims by `SCM_RIGHTS` 64 descriptors per message with a short count failing the claim, the quiesce, recovery, and the gate on macOS                                                                                                                                                                                                                                 | 3–4 weeks                                            | After the first macOS package, before macOS automatic updates                                                                               |
| B1    | [Background programs in the Host](#background-programs-the-worker-runs)                                                                                                                                                                                                                                                                                                                                                         | 1.5–2 weeks                                          | Right after P1b                                                                                                                             |

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

## H1 design

H1's decision run decided this design on 2026-11-15. It covers the Windows Keeper and the Host's
side of a move, which H1 builds and ships dark, and what H2, H3, and H4 inherit from them. It adds
no user-facing behavior or setting. Each pick is marked Chosen, which the owner may veto, or Rule,
and rows 18 to 33 of the [decision record](#decision-record) list them with their reasons. R1's
challenge amended rows 21, 22, 24, 29, and 33 and added rows 34 to 45, and probe P1 settled
row 43. R1's second run, on 2026-11-16, amended rows 26, 27, 31, 33, and 42 and added rows 46 to
55; S2 had landed before it, so row 55 reopens S2. A slice that finds a decision here wrong
reports it and pauses only the work that depends on it, as
[development](../../docs/DEVELOPMENT.md) asks.

### The Keeper protocol

**Endpoint and handshake**

- The Keeper runs as `orchbee-terminal-host keep`, with the `--runtime-directory` and
  `--endpoint-id` arguments of `serve`, naming a runtime directory and endpoint of its own that the
  Worker derives as it derives a Host's. It serves them through the Host's Local IPC with every
  protection it has, the same-user check at the first byte included.
- Rule: a peer that fails a check loses only its connection, and nothing a peer sends or does ends
  the Keeper or a holding, because the Keeper may be the only holder of a program.
- Every connection opens with OBHS. Chosen: one new negotiated connection kind, `keeper`, at
  generation 1 with no capabilities. Like `worker-host`, it drops no generation within 1.x, because
  a Host released with H1 may hold a terminal for as long as its program runs.
- The Keeper answers the control requests. `query` reports the new role `keeper`, which older
  readers read as `unknown`, with its holdings as `terminals`. After `drain` it admits no
  registration or hold and exits once it holds nothing. After `exit` it closes every duplicate and
  ends. It terminates no Job and no process, so `exit` ends a program only where the Keeper held the
  last handle, as at a confirmed stop.

**Frames**

- Chosen: after the answer, each message is one frame: the ASCII bytes `OBKP`, a big-endian `u32`
  length from 1 to 1,048,576, and one JSON object under the connection contract's
  [payload rules](../../schemas/connection/README.md#payload). JSON suits a few small messages per
  terminal and reuses the strict decoders that both bindings already have.
- A request carries `type` and an `id`, an integer from 1 to 4,294,967,295 that its connection has
  not used. Its reply carries the same `id` and is `<type>-result`, `refused` with a `reason`, or
  `unsupported`. One request is outstanding per connection. A reply is due within 2 seconds, or the
  requester closes the connection and counts the Keeper as unresponsive. H1's Hosts apply this
  bound to every later Keeper, so S6 reports the Keeper's reply time at its capacity under full
  processor load.
- Process ids are integers. Creation times and handle values are 16 lowercase hexadecimal digits,
  because a `FILETIME` exceeds the exact range of a JSON number; a holding id and a mark id are 32.
- Additions only: a new request, member, or event comes under a capability name; an unknown request
  gets `unsupported`, and an unknown event is ignored.
- A new `schemas/keeper` is the language-neutral authority: the messages, a JSON Schema, valid,
  boundary, and invalid fixtures, and a sequence fixture for each row of the state table below. Its
  Zig binding serves the Keeper and the Host; the Rust binding comes with its first consumer, the
  Worker, in H2. The fingerprint inputs of `keeper` are the connection contract, `schemas/keeper`,
  and each binding.

**Roles and admission**

- After the answer, the initiator's first frame is `hello` with `role`, `worker` or `host`. The
  Keeper takes the
  peer's process id from the pipe with `GetNamedPipeClientProcessId` before it decodes any frame,
  and admits:
  - `worker` from a process that is not a registered Host, while no other Worker connection is
    open, as a Host admits one broker session (Chosen). P1a's replacement and P1b's new Worker
    connect once the old Worker's connection has closed.
  - `host` only from a process the Worker registered. Any other peer gets `refused` with
    `not-registered`, and its connection closes.
- The Keeper opens each registered Host with `SYNCHRONIZE`, `PROCESS_QUERY_LIMITED_INFORMATION`,
  and `PROCESS_DUP_HANDLE`, checks its creation time, and keeps the handle while the Host is
  registered, so the process id stays that Host's. That handle tells it when the Host ends, through
  thread-pool waits, since one wait covers at most 64 handles. A Host that ends orphans the holdings
  it owns, and the Keeper unregisters it once no holding names it.
- A Host checks the Keeper the same way, by the pipe's server process id and the creation time that
  `AttachKeeper` names.
- Rule: the Keeper and every Host of a Session run at one integrity level, since the Worker starts
  them all with its own token. A lower process cannot open a higher one with `PROCESS_DUP_HANDLE`,
  so a mismatch fails `hold`, or the claim's pull, as `handle-failed` and ends nothing.
- No process of another user or of a lower integrity level can send the Keeper a frame: the pipe
  admits only the user's SID, a lower process cannot write to it, and an elevated Keeper labels it
  High, as an elevated Host does. Nor can such a process open the Keeper or a Host to take a handle
  or end a program. A process of the same user and level can do both without the protocol, as it
  can to a Host today, and can take the free Worker slot as it can a Host's broker session, so the
  protocol adds nothing to the Host's
  [trust boundary](../../apps/terminal-host/docs/runtime-model.md#boundary-and-non-goals).
- Chosen: the Keeper serves at most 256 Host connections, one Worker connection, and 8 connections
  not yet admitted or carrying a control request, each with 2 seconds for its first frame as on a
  Host, and closes any connection beyond them at once. The Host's Local IPC creates at most five
  pipe instances, one broker session and four control lanes, so the Keeper's listener sets its own
  limit.
- Rule: a Host counts the Keeper unresponsive only when a reply on an open link is late. A connect
  that fails, or takes more than 2 seconds, fails `AttachKeeper` and changes nothing else.

**Messages**

After `hello`, a connection sends only the requests its role allows; another gets `refused` with
`wrong-role`.

| Request           | From        | Members                                  | Effect                                                                                                                                                       |
| ----------------- | ----------- | ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `register-host`   | Worker      | `process`                                | Registers a running Host                                                                                                                                     |
| `unregister-host` | Worker      | `process`                                | Removes a Host that no holding names                                                                                                                         |
| `status`          | Worker      | `after`, optional                        | Up to 256 holdings in holding id order, each with its owner, state, mark target, and facts; the registered Hosts on the first page; `next` while more remain |
| `hold`            | Host        | `holding`, `handles`, `facts`            | The Keeper duplicates the seven handles from the Host, checks them against `facts`, and records the Host as owner, all or nothing                            |
| `mark`            | Owner       | `holding`, `for`, `mark`                 | Marks the holding quiesced for another registered Host under that mark                                                                                       |
| `resume`          | Owner       | `holding`, `mark`                        | Clears that mark: `resumed`, or `claimed` once the target has claimed under it                                                                               |
| `inspect`         | Mark target | `holding`, `mark`                        | Returns the Keeper's handle values and the facts                                                                                                             |
| `claim`           | Mark target | `holding`, `mark`, `program`, `terminal` | The commit: the target becomes owner                                                                                                                         |
| `decline`         | Mark target | `holding`, `mark`                        | Clears that mark for the requester: `declined`, or `claimed` when the requester's claim consumed it                                                          |
| `release`         | Owner       | `holding`                                | Closes the Keeper's duplicates and forgets the holding                                                                                                       |

- `process` and `for` are a process id and creation time. `mark` is a random 128-bit mark id that
  the old Host mints for one quiesce and writes into that quiesce's record. `handles` gives the
  values, in the requesting Host, of the Job, the program and OpenConsole process handles, the
  signal and reference handles, the input write end, and the output read end. `facts` gives the
  owner's terminal identity, the program's and OpenConsole's process ids and creation times, the
  OpenConsole build by SHA-256, its signal format, and the launch kind.
- At `hold` the Keeper checks its own duplicates: each object's type, both process identities, and
  the program's membership in the Job. At `claim`, `program` must equal the holding's program, and
  `terminal`, the claimant's terminal identity, replaces the owner's in `facts`.
- Rule: `inspect` and `claim` name the holding's current mark, and any other gets `stale-mark`. A
  record from an earlier quiesce misses what the old Host read after it resumed, so it never
  claims, and a late `decline` never clears a newer mark.
- A repeated request whose effect already holds gets the same answer: a `hold` of the same holding
  by its owner, a `mark` of the same mark for the same target, a `resume` by the owner while no mark
  is current, a `claim` of the mark it consumed by the current owner, a `decline` of a mark that is
  neither current nor consumed, and a `release` of an absent holding.
- Reasons are names that grow, read as `unknown` when not known: `not-registered`, `wrong-role`,
  `not-owner`, `not-target`, `unknown-holding`, `holding-exists`, `marked`, `stale-mark`,
  `identity-mismatch`, `capacity`, `draining`, and `handle-failed`.
- Chosen: a Keeper holds at most 1,024 holdings and registers at most 256 Hosts; a request beyond
  either gets `capacity`.

**Handle transfer**

- Chosen: handles move by pull. The process that will hold a duplicate makes it with
  `DuplicateHandle` from the other process, by the value a message names: the Keeper from the Host
  at `hold`, and the claiming Host from the Keeper after `inspect`. A lost message therefore never
  leaves a handle in a process that does not know it holds one, and no process writes handles into
  another.
- Rule: before the commit, neither the Keeper nor a claiming Host does I/O on a pipe handle or asks
  for its name: no `ReadFile`, `WriteFile`, `PeekNamedPipe`, `GetNamedPipeInfo`, or `NtQueryObject`
  with `ObjectNameInformation`, each of which can wait behind a blocked synchronous read of the
  shared file object, as [prototype 3](#prototype-3-result) measured for `PeekNamedPipe` and
  `ReadFile` and [probe P1](#probe-p1-result) across processes for `PeekNamedPipe`,
  `GetNamedPipeInfo`, and `ObjectNameInformation`; and no `GetFileType`, which did not wait but no
  check needs. Checks use `NtQueryObject` with `ObjectTypeInformation` and `ObjectBasicInformation`,
  which takes exactly 56 bytes, process and Job queries, and `IsProcessInJob`; probe P1 measured
  that none of them waits behind a blocked read.
- Rule: a handle value stays valid while a peer may pull it. The Keeper closes a holding's
  duplicates only at `release`, which a marked holding refuses, and when it ends, so a claim
  accepted under the mark that `inspect` named proves that the claimant pulled that holding's
  objects, which no check can show for a pipe before the commit. A Host closes no handle that an
  unanswered `hold` named until that Keeper has ended, and pulls only through the Keeper process
  handle it opened at attach, with `PROCESS_DUP_HANDLE` and `SYNCHRONIZE`, and checked there.
- Rule: every duplicate keeps its source's access, through `DUPLICATE_SAME_ACCESS`, and is not
  inheritable, so no child process becomes a holder. The claim's access check requires each handle
  to grant what the Host does with it after the commit, as the Host's reference lists per handle,
  so no step after the claim fails for want of access. Type and access cannot tell the signal end
  from the input end, both pipe write ends, and keys written to the signal pipe would make
  OpenConsole close every program, so S7 proves the hold's mapping object by object.
- Chosen: a Host mints a random 128-bit holding id at a terminal's first `hold`. The id stays the
  same across moves and Keeper replacements, and the record carries it.

**States**, per holding:

| Event                                                                                   | From                                          | To                                            |
| --------------------------------------------------------------------------------------- | --------------------------------------------- | --------------------------------------------- |
| `hold` by registered Host H                                                             | Absent                                        | Held by H                                     |
| `mark` M by H for registered Host N                                                     | Held by H                                     | Quiesced, H for N under M                     |
| `resume` of M by H, or `decline` of M by N                                              | Quiesced, H for N under M                     | Held by H                                     |
| `inspect` of M by N                                                                     | Quiesced or orphaned, H for N under M         | Unchanged                                     |
| `claim` of M by N naming H's program                                                    | Quiesced or orphaned, H for N under M         | Held by N, claimed from H under M: the commit |
| `resume` of M by H                                                                      | Held by N or orphaned, claimed from H under M | Unchanged, answered `claimed`                 |
| `claim` or `decline` of M by N                                                          | Held by N, claimed from H under M             | Unchanged, answered `claimed`                 |
| `inspect` or `claim` of a mark that is not current, or `resume` of one while another is | Any                                           | Unchanged, refused `stale-mark`               |
| H ends                                                                                  | Held by H                                     | Orphaned                                      |
| H ends                                                                                  | Quiesced, H for N under M                     | Orphaned, H for N under M                     |
| `decline` of M by N                                                                     | Orphaned, H for N under M                     | Orphaned                                      |
| `release` by H                                                                          | Held by H                                     | Absent                                        |
| `exit`                                                                                  | Any                                           | The Keeper closes every duplicate and ends    |

- The Keeper applies one request at a time across all its connections, so a claim and a resume
  never both succeed.
- Rule: only that application is serialized, under a lock held across no I/O and no wait. Each
  connection reads and writes on its own and keeps at most one reply, and a peer that has not taken
  its reply within the reply deadline loses its connection, so a peer that stops reading stalls no
  other and cannot make the Keeper look unresponsive to the Worker.
- N's end changes nothing; the old Host's resume then ends the mark. `release` of a quiesced
  holding gets `marked`. A holding remembers the Host it was claimed from, and the mark that claim
  consumed, until that Host ends or is unregistered.
- Only a mark's target claims an orphaned holding. H2's `recovery` lets the Worker give an unmarked
  orphan a mark and a target with `assign`, or end it with `discard`, as
  [H2 design](#h2-design) says; without it such a holding keeps its program running until `exit`.

**Commit and failures**

- Rule: the claim is the only commit. Before it the new Host takes every fallible step: receiving
  and validating the record, restoring the terminal, `inspect`, pulling the duplicates, the claim
  checks, packing the pseudoconsole, and creating every thread the terminal runs behind one gate.
  After `claimed` it only opens the gate, which cannot fail.
- Abort: the Worker's `ResumeTerminal`, or the old Host's own deadline after its mark, which
  `QuiesceTerminal` names, makes the old Host send `resume`. `resumed` lets it read again, and
  `claimed` makes it report the terminal as moved and wait for `DropTerminal`; any other answer, or
  one it cannot decode, counts as unanswered. The new Host discards its import, sending `decline`
  first, at the Worker's request, when its claim is refused, or by itself once the deadline that
  `ImportTerminal` names passes without a claim, so a lost Worker never leaves its duplicates
  keeping the old Host's console from ending.
- Rule: whichever answer first shows the new Host `claimed`, to `claim` or to `decline`, completes
  its claim: it opens the gate, and `DiscardImport` answers `claimed` instead of discarding.
- Rule: a Host reads a terminal only while the Keeper names it owner and after it has seen that
  answer, and a request left unanswered counts as possibly applied: the old Host stays stopped and
  the new Host does not read. Without the Keeper's `resumed`, the old Host reads again only on a
  `ResumeTerminal` made without the Keeper, which the Worker sends only after the Keeper process has
  ended, ending it if it stopped answering, and after the new Host has discarded its import without
  having seen `claimed`. This holds even after the old Host has seen `claimed`: a claim whose
  answer the new Host lost stops both Hosts, and ending the Keeper undoes it, because that Host
  never read. A moved terminal therefore keeps its handles until `DropTerminal`, which the Worker
  sends only after the new Host has seen `claimed`. A deadline never resumes a terminal without the
  Keeper's answer.
- Chosen: the Worker aborts a move before it closes the moving terminal.
- Rule: the Worker sends `DropTerminal` as soon as the new Host has seen `claimed`. Until the drop
  the old copy holds OpenConsole's reference handle, so OpenConsole stays open and a program that
  ends on the new Host, by itself or by a close, cannot settle there. A Worker that adopts a Host
  with a moved terminal drops it once the Host that owns the holding by the Keeper's `status`
  reports the terminal running. Inventory names the holding id of every held, quiesced, moved, and
  restored terminal, so a replacement Worker can pair the two copies.

### The Keeper's life

- Chosen: from H2, the Worker starts one Keeper per Session right after its first Host is ready,
  and on adoption when none runs. It starts it from its own installation and in the Session's Job
  placement, as it starts Hosts, through a Keeper slot in the Session record with the Worker as
  registered parent. A failed start fails no launch and is logged; terminals then run unheld, as
  before H2.
- The Worker owns it. It registers each Host and then sends that Host `AttachKeeper`, and a Host
  holds each terminal it creates once its creation completes as running. On a confirmed stop the
  Worker sends `exit` to the Hosts and then to the Keeper; the Server's stop without a Worker, from
  P1a, does the same, checking the Keeper by its recorded process id and creation time.
- The Keeper outlives the Server and the Worker. P1a's replacement Worker and P1b's new Worker adopt
  it as they adopt a Host: by its recorded identity, its endpoint's server process, a `keeper`
  offer, and `status`, whose owners they reconcile with the Hosts they adopted. H2 adds the Keeper's
  identity to `AdoptHost`, whose exact OBCN contract changes in place.
- It exits after it has held nothing, with no Host registered and no Worker connected, for 30
  seconds, or at `exit`. A machine restart ends it with every program.
- It writes `keeper-fatal.json` and `keeper-diagnostics.jsonl` in its runtime directory, in the
  Host's formats with the role `keeper`; H2 adds them to what deleting a Session removes.
- Chosen, for H4 on Windows: the Worker starts the new Keeper in a second Keeper slot, registers
  every Host with it, and sends each Host `AttachKeeper` for it, and each Host holds every terminal
  there under the same holding id. The Worker compares both Keepers' `status` holding by holding,
  then sends the old Keeper `exit`, which closes only its duplicates, and each Host `DetachKeeper`
  for it. It starts only while every holding has a live owner, and a failure at any step sends the
  new Keeper `exit` instead, which ends nothing, so the old Keeper stays.
- Chosen: a Host holds at most two Keeper attachments and refuses a quiesce while it has two, so
  one Keeper arbitrates each move and H4 needs no Host change.

### The Host's side

**Undoable stop.** Chosen: the Host parks its reader thread instead of ending it, as
[prototype 3](#prototype-3-result) did 159,984 times without a stray cancel.

- A pause cancels a blocked read as `abortRead` does, but the platform read then reports a pause,
  apart from end of output and from abort. The reader applies whatever the read returned. While
  that leaves Ghostty's parser, its UTF-8 decoder included, or one of the Host's scanners inside a
  sequence, it reads on without a cancel; after a batch that leaves all of them at ground, it
  publishes that it is parked under the terminal's lock and waits for resume or abort. Resume reads
  on; abort ends the thread as today. A pause withdrawn before the reader parks lets it read on.
- The platform read asserts that a read ending in `ERROR_OPERATION_ABORTED` returned no bytes.
- POSIX gets the same pause: resume drains the abort pipe and clears `read_aborted`. Its pause test
  runs wherever the POSIX suite runs, and H5 adds the move evidence on macOS.

**Quiesce**, in the old Host, with no step blocking the dispatch thread:

1. The terminal must be on the bundled OpenConsole, without an image, and not moving, and either
   running, held in the Host's one Keeper, and without a termination intent, or settled with its
   outcome published; otherwise the request is refused as ineligible. One whose screen and history
   memory exceeds 128 MiB is refused as `too-large`. A settled terminal skips steps 2 to 5 and 7,
   as eligibility below describes.
2. From then on it refuses input, resizes, terminal control, and termination with
   `terminal-quiesced`; the Worker holds them.
3. The Host waits up to 1 second for its writer to finish the accepted input and its signal sender
   to go idle, because OpenConsole stops taking signals while its output goes unread, as prototype
   1 measured, and input may wait the same way.
4. It pauses the reader and waits up to 1 second for it to park at ground with its last batch
   applied; otherwise it withdraws the pause and refuses, as passing `mid-sequence` when the reader
   is reading on inside a sequence.
5. It waits again, up to 1 second, for the writer and the signal sender to go idle, because the
   last batches can queue terminal replies and resizes that the output drove, such as DECCOLM's;
   if they do not go idle, it resumes the reader and refuses as passing.
6. It mints a mark id and exports the record, which carries it, off the dispatch thread, and
   refuses a record over 128 MiB as `too-large`.
7. It marks the holding for the new Host under that mark id and starts the deadline that
   `QuiesceTerminal` names.
8. It answers with the record's size.

Chosen: the export comes before the mark, so a failure in steps 3 to 6, or a refused mark, discards
the export and resumes reading without the Keeper. After a mark only the Keeper's `resumed`
restarts the reader, and a mark left unanswered counts as made.

**Parking at ground.** Chosen: H1 tracks no continuation.

- The reader parks only at ground, as the undoable stop describes, so a quiesce never splits a
  sequence, and the Host's stream runs without tracking, as it does today. The check is the
  stream's `ground()`, which covers Ghostty's parser and its UTF-8 decoder and works with tracking
  off, as the pin's source shows.
- The Host's own scanners that span reads, for OSC 7501, the private APC, and shell observation,
  must be at ground too.
- The record keeps Ghostty's continuation record and the scanners' partial state, so a later phase
  can add tracking without a new frame format. An H1 Host writes both empty, checks ground again
  before it exports, since an empty continuation would drop a split sequence, and refuses to import
  a record in which either is not empty.
- The later phase brings tracking and the rule that replaying a continuation through the Host's
  wrapper repeats no effect.

**Restore and claim**, in the new Host:

- The import joins the Host's creation pipeline; H1 does not assume that a creation runs on the
  request thread. `RestoreTerminal` admits a creation whose slow work runs where a start's slow
  work runs: it validates and decodes the record, builds the engine in the state `restored`, which
  reads nothing and refuses input, resizes, terminal control, and attachment, then `inspect`s the
  holding, pulls the seven duplicates, checks them, packs the pseudoconsole, and creates every
  thread the terminal runs, its wait thread, reader, writer, animation thread, and signal sender,
  behind one gate. Its process record is built from the duplicates, with the checked OpenConsole as
  its console reference instead of one learned again from the live program. The creation ends at
  `restored`, not at `running`.
- The claim checks, on the Host's own duplicates:
  - the program's process id and creation time equal the record's and the holding's;
  - `IsProcessInJob` places the program in the Job, and the Job has kill-on-close;
  - OpenConsole's process id and creation time equal the record's;
  - Rule: the record's OpenConsole SHA-256 is in the Host's table of known conpty builds, whose
    entry gives that OpenConsole's signal format, which must equal that of the Host's own pin. The
    Host signals a moved console through its own conpty, so a pin with a new format must add a
    writer for the old one before terminals in the old format can move to it. The HPCON layout a
    Host reads is always its own conpty's, which created or packed the pseudoconsole, so the table
    gives a layout only for the Host's own build. Chosen: the Host does not read the running
    console's image again, since its build was fixed when it started;
  - each object has its expected type and access.
- Packing with `ConptyPackPseudoConsole` must send nothing to OpenConsole and take the three
  handles as they are, keeping no copy that would outlive a drop, which S7 confirms from the pinned
  source.
- `ClaimTerminal` sends `claim` and, after `claimed`, opens the gate; the terminal is then
  `running`. `DiscardImport` sends `decline` and then discards; once the Host has seen `claimed`,
  in that answer or an earlier one, it completes the claim instead and answers `claimed`.
- Rule: a Host keeps at most one import of a holding and refuses a second `RestoreTerminal` of it
  as `moving`.
- Rule: no screen-gated key crosses a move. A gated key is input, so from step 2 of the quiesce it
  fails `terminal-quiesced` like any other. The record keeps the generation and the content
  revision that a sender names a frame by, so the restore starts the terminal with a person's input
  recorded on its arrival revision, at its own clock's time. Every key decided before the move
  names a first-seen revision at or before that screen, and rule 6 of
  [screen-gated input](../../apps/terminal-host/docs/runtime-model.md#screen-gated-input) refuses it
  however late it arrives, a retried one included; a prompt first seen after the arrival is answered once
  the quiet time since the arrival has passed, so no automatic key follows a person's last input on
  the old Host by less than the quiet time. The old Host's input record does not travel: each of its
  entries came before the quiesce, at or before the arrival revision, so the arrival record refuses
  at least every key the old one did. A prompt already on screen when its terminal moved is left to
  the person. Chosen over starting a new generation, which would change the frame identity the move
  keeps for every Client and still leave the new Host not knowing whether a person typed just before
  the move.
- H1 takes the rule for requests that name a terminal still in creation from the asynchronous
  start, now the runtime model's
  [creation pipeline](../../apps/terminal-host/docs/runtime-model.md#terminal-creation-pipeline).
  It gives two of the three things H1 needed of it: a held request is decided by the state the
  creation ends in and never acts on a partly built terminal, so a `restored` terminal refuses
  input, resizes, terminal control, and attachment until its claim. It cancels no creation, since a
  requester that disconnects does not cancel its own, so Rule: a `DiscardImport`, or the import's
  deadline, that comes while the restore builds is held like a request naming the terminal, and
  discards the import when the build ends, closing only the new Host's duplicates. The pipeline's
  teardown contains a runtime its build made; Rule: for an import it discards the runtime instead.
  S8 writes both into the pipeline.

**Drop.** Rule: an old Host drops a moved terminal, and a new Host discards an import, in two
steps. First it stops and joins every thread of the terminal with no I/O on its pipes: the parked
or gated reader ends, the idle or gated writer and the signal sender stop, and the wait thread,
which waits on the program and on a stop event, ends without running its exit path. A wait thread
left blocked would wake when the program ends and close a pseudoconsole through handle values that
may by then name another terminal's handles. Then it closes only its own handles: the four it holds
directly and the three inside its pseudoconsole, read through the pinned layout. It keeps the
pseudoconsole's small block until it exits. It never calls `ClosePseudoConsole` and never
terminates the Job or changes its limits, so it does not run today's `deinit`, which clears
kill-on-close; it sends no signal and writes no input, detach repair included.

**Eligibility and lifecycle**

- Rule: only terminals on the bundled OpenConsole are held and move. System-ConHost terminals, on
  ARM64 or without the bundle, keep the P1c behavior.
- Chosen: a running terminal moves with a claim. A settled terminal moves as a record, without a
  Keeper or a claim, so a draining Host can exit while a finished terminal stays on screen: once
  its outcome is published, `QuiesceTerminal` exports it with no mark; its import ends settled with
  that outcome already published, so it never sends `TerminalExited` again, and refuses
  `ClaimTerminal`; and the Worker then disposes the old copy as it disposes any settled terminal.
  Awaiting-start, finishing, containing-failure, and terminating terminals do not move, and neither
  does a terminal holding an image until H3's image section.
- New terminal states, reported additively in inventory: `quiesced` in the old Host; `moved` once
  the Keeper has answered `claimed`, until `DropTerminal`; and `restored` in the new Host until its
  claim or discard. A dropped terminal leaves without settlement, with no outcome and no
  `TerminalExited`, and `DropTerminal` needs every attachment gone.
- Rule: while a terminal is quiesced or moved, the old Host takes no lifecycle step for it. An
  observed program exit waits for the move's result, and after a claim the new Host observes the
  exit through its own process handle.
- Rule: a Host never ends or contains a moved or restored terminal, or a quiesced one without the
  Keeper's `resumed`. Every path that ends terminals, `exit` and the
  [ordinary](../../apps/terminal-host/docs/runtime-model.md#host-lifecycle) and
  [fatal](../../apps/terminal-host/docs/runtime-model.md#fatal-teardown-retained-cleanup-thdd-001)
  teardowns included, resumes a quiesced terminal through the Keeper and ends it only after
  `resumed`. It drops a moved terminal and a quiesced one whose resume got any other answer or none,
  and it discards a restored import, because ending any of them would end a program that the other
  Host serves or may serve.

**Release first.** Rule, in every Host from H1: before a held terminal's pseudoconsole, signal
handle, reference handle, or Job handle closes, or its Job is terminated, each Keeper holding it has
released it, or 2 seconds have passed and the Host has reported `KeeperUnresponsive`. After one
missed deadline the link fails later requests at once until the Keeper is replaced, and the
terminal's end completes once the Keeper's handles close. Chosen: Keeper I/O runs on a link thread
per attachment, never on the dispatch thread or a reader thread.

**Keepers during a move.** Rule: a Host holds in a Keeper only the terminals it reads, so no holding
names a Host that may not read. `AttachKeeper` holds every running terminal that is not quiesced; a
quiesced terminal is held there once it resumes, and a restored or moved one never is. A quiesced
terminal's `resume` goes only to the Keeper that holds its mark, and `DetachKeeper` is refused as
`moving` while that Keeper holds a mark of the Host's or a restored import came from it. Once that
Keeper is lost, or its link has missed a deadline, only the Worker settles the terminal: by
`ResumeTerminal` without the Keeper, under the rule of [commit and failures](#the-keeper-protocol),
or by `DropTerminal` after the new Host has seen `claimed`, which a quiesced terminal then accepts
too. A quiesced terminal held again in a second Keeper could otherwise be resumed there while the
new Host reads.

**Requests on `worker-host`**, under two new capabilities:

| Capability | Message                                              | Content                                                                                                                                                      |
| ---------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `keeper`   | `AttachKeeper`                                       | The Keeper's process id, creation time, and endpoint id; the Host connects and holds every terminal it reads                                                 |
| `keeper`   | `DetachKeeper`                                       | Releases the Host's holdings in that Keeper and closes the link                                                                                              |
| `keeper`   | `KeeperLost`, `KeeperUnresponsive`, `TerminalUnheld` | Events                                                                                                                                                       |
| `handover` | `QuiesceTerminal`                                    | The terminal; the new Host's process id and creation time; the mark's deadline, from 1 to 30 seconds, which the Worker sets to 5; answers the record's size  |
| `handover` | `ReadHandoverRecord`                                 | An offset; answers up to 4 MiB                                                                                                                               |
| `handover` | `ResumeTerminal`                                     | Answers `resumed` or `claimed-elsewhere`; made without the Keeper once the Keeper holding the mark is lost or unresponsive, it also resumes a moved terminal |
| `handover` | `DropTerminal`                                       | Removes a moved terminal, or a quiesced one whose mark's Keeper is lost or unresponsive                                                                      |
| `handover` | `ImportTerminal`                                     | A creation id, the record's size, and the import's deadline, from 1 to 30 seconds, which the Worker sets to the mark's                                       |
| `handover` | `WriteHandoverRecord`                                | An offset and up to 4 MiB                                                                                                                                    |
| `handover` | `RestoreTerminal`                                    | Restores, inspects, pulls, checks, packs, and creates the gated threads; answers the new terminal identity                                                   |
| `handover` | `ClaimTerminal`                                      | Claims and starts reading                                                                                                                                    |
| `handover` | `DiscardImport`                                      | Discards an import or a restored terminal, or answers `claimed`                                                                                              |

- A refusal names its class, so H3 can act on it: ineligible (`not-running`, `unheld`,
  `system-console`, `unknown-console-build`, `holds-image`, `too-large`, `two-keepers`), passing
  (`input-pending`, `signal-pending`, `mid-sequence`, `moving`), or failed.
- The dispatch thread never waits on the Keeper, a parking reader, or an export: each such request
  completes when its result is published, with its reply reserved up front.
- A Host advertises `handover` only when its bundled conpty exports `ConptyPackPseudoConsole` and
  its own build is in its table.
- Chosen: `query` gains `dispatch-progress` in `extensions`, the number of completed dispatch-loop
  turns, which control lanes answer from published facts, for H2's hang check.

**Record size.** Chosen: the whole record crosses before the claim, in chunks of up to 4 MiB, up to
128 MiB: about 1.5 seconds at the rates [prototype 2](#prototype-2-result) measured, and about
400 MB of styled history. A larger terminal stays on its Host; claiming at READY with history
afterwards is left to H3's decision run, as an addition. Chosen: a Host keeps at most one export
and one import at a time, and refuses another `QuiesceTerminal` or `ImportTerminal` as passing
(`moving`), so records cost a Host at most 256 MiB; moves run one at a time anyway, as decision 9
says.

**Commit seams.** The runtime model's
[commit-seam set](../../apps/terminal-host/docs/runtime-model.md#commit-seam-fault-matrix) gains
these seams from `CS-030`, each with a named failure test:

| Seam               | Disposition                                                                           |
| ------------------ | ------------------------------------------------------------------------------------- |
| `KeeperHold`       | Commit and drop: the terminal runs unheld, counted and reported                       |
| `KeeperRelease`    | Contain and retain: the end waits for the release or its deadline                     |
| `ReaderPause`      | Rollback: the reader reads on                                                         |
| `HandoverExport`   | Rollback: the export is discarded and the reader reads on                             |
| `KeeperMark`       | Rollback when refused; contain and retain when unanswered                             |
| `HandoverRestore`  | Reject without commit: the import is discarded                                        |
| `ClaimPreparation` | Reject without commit: the duplicates close and the import is discarded               |
| `KeeperClaim`      | Reject without commit when refused; contain and retain when unanswered                |
| `KeeperResume`     | Contain and retain: the terminal stays stopped until the Keeper or the Worker answers |

**Shipping dark.** Chosen: H1 adds no switch. Hosts advertise `keeper` and `handover`, but a
capability works only when both sides list it, and no release Worker lists either before H2, so
outside a move every terminal runs as before, except that its wait thread also waits on a stop
event; the reader's pause path runs only in a quiesce. From H2 the Worker offers
`keeper` while a new local-runtime switch, `keeper_live`, holds, and from H3 it offers `handover`
while `host_moves_live` holds. Like
[`automatic_updates_live`](../../crates/local-runtime/README.md#automatic-update-switch), each
answers true in test builds and false in release builds until H2 and H4, respectively, turn it on.

### The record's sections

- Chosen: the envelope is `OBHR`, the format's generation, the exporting Host's product version,
  its Ghostty pin's package hash, and a section count. The format is registered as frame format
  `obhr` at generation 1 and, like `keeper`, drops no generation within 1.x, so the registry
  enforces the import rule of [the terminal record](#the-terminal-record). Each section has a tag, a
  required flag, a length, and a CRC-32C, which catches corruption, the only threat inside the
  same-user boundary. Deadlines travel as remaining durations.
- The format's authority is a reference of the Terminal Host package, written with its encoder, and
  the stored sample of generation 1 joins the registry's frozen fixtures.

| Section            | Content                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal`         | The holding id and the quiesce's mark id; the exporting Host's terminal identity; the lifecycle, running or settled with its outcome and final revision; the content revision and generation; the geometry and start geometry; the viewport restore target; the launch kind and interactive-shell flag; the launch fingerprint and environment-overlay digest, which the new Host binds to the creation id of `ImportTerminal`                                                                              |
| `configuration`    | The committed configuration and its revision; the complete theme with its operation id and revision                                                                                                                                                                                                                                                                                                                                                                                                         |
| `screen`           | Ghostty's snapshot version 1 with its continuation record, which an H1 Host writes empty                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `engine`           | Dynamic and default colors; the color scheme; the bell sequence; progress; the OSC 7501 record; the command-completion record; the pending clipboard intent; the application-signal FIFO; the mouse-input epoch; what the program was last told of focus, keys, and buttons, which the new Host repairs as it repairs a detached attachment's; prompt-gate and shell-observation state; the scanners' partial state, empty from an H1 Host; the remaining mode-2026 deadline; the ConPTY clear coordination |
| `console`, Windows | The conpty pin version, OpenConsole SHA-256, and signal format; the program's and OpenConsole's process ids and creation times                                                                                                                                                                                                                                                                                                                                                                              |
| `images`           | From H3; until then a terminal holding an image refuses export                                                                                                                                                                                                                                                                                                                                                                                                                                              |

- Not carried: attachments, viewer leases, delivery sequences, held-input and focus-claim ledgers,
  the creation binding, and the compression cursor. The Worker makes attachments again, and a
  quiesced or moved terminal materializes no detach repair.

### The move's model

- Chosen: a TLA+ model, `terminal-host-handover`, because four independently ordered actors share
  one small protocol. It sits beside the Terminal Host's models in
  `apps/terminal-host/formal/tla/`, is registered in `scripts/tla/models.json`, and meets the 20-second deadline with two
  workers.
- Actors: the Worker, the old and new Host, the Keeper, and a second Keeper. Each Host and each
  Keeper may crash once, the Worker may crash once and be replaced, the Worker may end a Keeper
  that missed a deadline and attach a second at any moment, a broken connection loses the message
  in flight, a reply after its request was applied included, and one answer may arrive undecodable.
  The Worker may quiesce twice, so that a record of the first quiesce can meet the second mark. The
  bounds are one terminal, a program whose output is 2 bytes forming one sequence, so the reader
  can stop before, inside, or after it, and one held key in each quiesce; a third byte adds no case
  a property tells apart and more than doubles the state space, past the deadline. The module lists
  what the bounds leave out.
- Invariants, each a Rule:
  - one reader: at most one Host reads the terminal, and it is the Keeper's owner, or the Keeper has
    ended and the Worker confirmed the resume;
  - no loss or repeat: the reading Host's applied bytes, the record's included, followed by the
    pipe's, equal the produced bytes, and no record ends inside a sequence;
  - each held key reaches at most one Host, at most once and in order, and exactly once when the
    Worker that accepted it does not crash, since a replacement Worker refuses keys during its gap;
  - an aborted move leaves the old Host's state plus what it read since;
  - only a claim commits: the new Host reads only after it has seen `claimed`, and the old Host
    never reads after the new Host has seen `claimed`;
  - a holding names only a Host that may read the terminal, in every Keeper;
  - a reader remains: while both Hosts run, one of them reads the terminal or keeps the copy
    through which it may read it later, so no drop or discard leaves a terminal neither can read.
- Progress: if neither Host crashes and a Worker eventually runs, the terminal eventually has a
  reader.
- Every path that settles a quiesced terminal in the old Host, its deadline, `exit`, and both
  teardowns, decides by the same `resume` answer, so the model's resume stands for all of them, and
  S8 runs each for real.
- Mutants, each caught by a named property in the mutation suite: a claim accepted without a mark
  for the claimant; a `resume` that succeeds after a claim; a mark sent before the reader parks; a
  new Host that reads before it has seen `claimed`; an old Host that resumes at its deadline without
  the Keeper's answer; a claim accepted under an earlier mark; a quiesced terminal held again in a
  second Keeper; a reader that parks inside a sequence; and a drop sent once the old Host has seen
  `claimed` but before the new Host has, which loses the only copy that could read again when the
  claim's answer was lost.
- Claims `TH-028` to `TH-034` join the runtime model with the slices that land the behavior they
  state: one reader across Hosts, the undoable stop, exact export and import with the reader stopped
  only at ground, release first, the claim as the only commit, a drop that ends nothing on every
  path that ends a Host or its terminals, and the Keeper's arbitration, which no peer can end or
  stall. Until then the runtime model lists the model as evidence that maps to no claim.
- `TH-033` narrows `TH-009` and THDD-001: a dropped or discarded terminal's process tree belongs to
  the Host that serves it, so leaving it uncontained orphans nothing.

| Claim                                  | Witnesses                                                                               |
| -------------------------------------- | --------------------------------------------------------------------------------------- |
| `TH-028`, one reader                   | The model's one reader and only a claim commits, with their mutants; S8; S9             |
| `TH-029`, the undoable stop            | S3; S8's forced failures; S9's flood and stalled sequence                               |
| `TH-030`, exact export and import      | S3; S5; the model's no loss or repeat and records at ground; S9                         |
| `TH-031`, release first                | S7                                                                                      |
| `TH-032`, the claim as the only commit | The model; S8's seams and lost answers                                                  |
| `TH-033`, a drop that ends nothing     | The model's reader remains, with its mutant; S8's `exit` and teardowns of each Host; S9 |
| `TH-034`, the Keeper's arbitration     | S6                                                                                      |

- The claims left without a witness are in [risks](#risks-h1-leaves-open).
- Other registered models: `terminal-host-delivery` and `terminal-host-process-identity` do not
  change in H1. `session-crash-recovery` and
  `local-session-startup` change in H2 for the Keeper slot and its start,
  `local-terminal-coordinator` and `worker-send-transaction` change in H3 for the hold and
  rebinding, and `terminal-host-process-identity` changes in H5, when the Keeper parents programs.

### H1 slices

Two streams follow the challenge and the contracts: stream A, the engine, is S3 to S5, and stream
B, the Keeper, is S6 and S7. Each slice merges on its own after the affected host gate in
[verification](../../docs/VERIFICATION.md) and maps its evidence into the runtime model's native
evidence index.

| Slice | Content                                                                                                                                                                                                                                                                                                                                                                                           | Acceptance and witnesses                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Rung       | After                                  |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | -------------------------------------- |
| R1    | Design challenge of this section: the protocol, the commit, the claims, and the evidence plan, given the scope and authorities without this run's reasoning                                                                                                                                                                                                                                       | Each finding is resolved here or recorded as an accepted risk before S1 merges                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | fable/high | —                                      |
| S1    | Contracts: `schemas/keeper` with its fixtures and Zig binding; `keeper` among the registry's negotiated connections with the 1.x rule, and `obhr` among its frame formats with the same rule; the role `keeper` and `dispatch-progress` in `schemas/connection`; the `keeper` and `handover` capabilities and their messages in `schemas/terminal` and every codec its conformance check requires | Every binding consumes every fixture; the Rust registry test accepts the new entries and refuses a dropped `keeper` or `obhr` generation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | opus/xhigh | R1                                     |
| S2    | The model, its entry in the runtime model's evidence, and its mutants; it landed before R1's second run, which reopens it for the reader-remains invariant and the drop mutant (row 55)                                                                                                                                                                                                           | The configuration passes inside its deadline with full required coverage, and `scripts/tla/model-mutations.test.mjs` catches each of the nine mutants inside the suite's budget                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       | opus/xhigh | R1                                     |
| S3    | Stream A: the undoable stop with its park at ground, and the aborted-read assertion; seam `ReaderPause`                                                                                                                                                                                                                                                                                           | Through the pinned conpty and the system ConHost, a numbered-line probe survives 1,000 pause-and-resume cycles in one Host with nothing lost or repeated, and a pause during a flood parks within 1 second; amid sequences cut across reads a pause parks only at ground, and a pause while the program stops inside a sequence never parks, and withdrawing it loses nothing; in `src/platform/__tests__/pty_test.zig` and an engine test                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | opus/xhigh | R1                                     |
| S4    | Stream A: `dispatch-progress`                                                                                                                                                                                                                                                                                                                                                                     | A control lane's `query` reports a count that grows with dispatch turns and stops while a test holds the dispatch thread, in `src/transport/__tests__/candidate_test.zig`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | opus/high  | S1                                     |
| S5    | Stream A: the record's encoder, decoder, and reference; export and import in one Host; the stored sample; seams `HandoverExport` and `HandoverRestore`                                                                                                                                                                                                                                            | For prototype 2's corpus and a busy shell, export then import restores identical state, compared as prototype 2 compared it, every section included, with the continuation record and the scanners' state written empty; the corpus cut at every byte exports where the parser and the scanners are at ground, refuses everywhere else, and each export, imported and fed the rest, ends identical to the source; a corrupt, truncated, foreign-pin, or unknown-required-section record, or one whose continuation is not empty, is refused with nothing changed; the stored sample imports                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | opus/xhigh | S1                                     |
| S6    | Stream B: the Keeper, as its protocol and life state, without H2's start and adoption                                                                                                                                                                                                                                                                                                             | With a stand-in Host holding a real program in a kill-on-close Job on the pinned conpty: every sequence fixture passes; mismatched identities are refused; `DuplicateHandle` and the type, access, and Job checks complete while another process's synchronous read blocks the pipe; a peer that stops reading its replies stalls no other connection; a `claim` and a `resume` of one mark sent at once on two connections, 1,000 times, never both succeed; every invalid fixture sent on a real connection loses only that connection while the Keeper serves the others; a connection beyond the bounds closes at once; the reply time at capacity under full processor load is reported; an unregistered peer loses only its connection, a peer under another user's token, such as the anonymous one, cannot open the pipe, and a Low-integrity child sends no frame; a Host's end orphans its holdings; `exit` ends no program while the stand-in keeps its handles; an empty Keeper exits after 30 seconds                                                                                                                                                                                                                                                                    | opus/xhigh | S1                                     |
| S7    | Stream B: `AttachKeeper` and `DetachKeeper`, holds, release first, the link thread and its events, the layout read, and the table of known conpty builds; seams `KeeperHold` and `KeeperRelease`                                                                                                                                                                                                  | With a real Host and Keeper: every running terminal is held; natural exit, close, forced termination, containment, and Host `exit` each release first and complete; a Keeper that stops answering yields one `KeeperUnresponsive`, and the ends complete once it is ended; ending the Keeper leaves every program running and yields `KeeperLost`; a `hold` left unanswered while its terminal ends keeps the handles it named until the Keeper is ended; each of the Keeper's seven duplicates is the same object as the Host's handle in its role, by `CompareObjectHandles`; the pinned conpty source shows that packing sends nothing and takes the three handles without duplicating them                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | opus/xhigh | S6                                     |
| S8    | The `handover` requests: quiesce, export, import through the creation pipeline, the claim checks, packing, claim, drop, discard, settled-terminal moves, and the lifecycle rules while quiesced; seams `KeeperMark`, `ClaimPreparation`, `KeeperClaim`, and `KeeperResume`                                                                                                                        | With two Host processes and a Keeper: a failure forced at every seam leaves the terminal reading on its old Host with nothing lost; a refused or unanswered claim never lets the new Host read, and a claim whose answer is lost ends with one reader once the Keeper is ended; a record of an earlier quiesce is refused as `stale-mark`; a quiesce held inside a sequence is refused as `mid-sequence` and the terminal reads on; the deadline resumes the terminal, and an unclaimed import discards itself at its own; a resume without the Keeper is refused while the Keeper holding the mark answers; drop and discard join every thread first and end nothing, and no `exit`, ordinary teardown, or fatal teardown of either Host ends a program the other serves; after each forced failure, discard, drop, and settled move, the handle counts of both Hosts and the Keeper return to their baselines, and ending the terminal on its owner ends the program and OpenConsole; a `DiscardImport` or import deadline during the restore's build discards the import when the build ends; a settled terminal moves once its outcome is published, and its import sends no `TerminalExited`; after `KeeperLost`, the next Keeper holds no quiesced, restored, or moved terminal | opus/xhigh | S3, S5, S7, and the asynchronous start |
| S9    | Real Windows black-box run, `src/service/__tests__/handover_black_box_test.zig`, in the move witnesses' executable: two Host processes and a Keeper in a test-owned runtime root, the test acting as the Worker                                                                                                                                                                                   | The [H1 acceptance](#acceptance), plus the pause from the quiesce request to the claim during a flood, at the default scrollback, and at 100 MB of history, reported in the release build                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | opus/high  | S8                                     |
| R2    | Review of the whole H1 candidate against this section, its claims, and its evidence                                                                                                                                                                                                                                                                                                               | Findings are fixed before the first release that carries H1                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | fable/high | S9                                     |

### What H2 needs from H1

- Holdings carry what recovery needs: the owner's terminal identity, both process identities, the
  OpenConsole build and signal format, the launch kind, and all seven handles.
- Hosts attach to a Keeper started after them, hold every terminal they read there, and after
  `KeeperLost` hold again in the next Keeper the Worker attaches.
- Every Host from H1's release releases first, so a Keeper that H2 starts never keeps a terminal
  from ending.
- `dispatch-progress` in `query`, and the Keeper's `status`.
- Holding ids in inventory, so a replacement Worker pairs the two copies of a move its predecessor
  left half done.
- The claim path in separate steps, inspect, pull, check, pack, create the gated threads, claim,
  and open the gate, so that H2's recovery reuses them with a screen it rebuilds in place of a
  record.
- No pipe I/O before the commit, so a hung Host's blocked read cannot hold up a claim: H2's recovery
  may pull and check a hung Host's handles before that Host has ended, as
  [probe P1](#probe-p1-result) measured, but its replacement reads only after it has ended.
- A model with crashes, lost replies, and a second Keeper, which H2 extends with orphan claims.
- When the new Host ends after the Keeper applied its claim, or the Keeper and the new Host both end
  after a mark, the old Host stays stopped until the Worker settles it: by recovery from the Keeper,
  or, once the Keeper has ended, by `ResumeTerminal` without it, which leaves a gap on screen if the
  new Host may have read. H2 decides which; an H1 Host needs no change for either.
- Left to H2: the Keeper slot in the Session record, the Worker's start and adoption after P1a,
  claims of unmarked orphaned holdings as a Keeper request under a new capability, `keeper_live`,
  the Keeper's files in Session deletion, and dropping, on adoption, a moved terminal whose new
  owner reports it running.

### Risks H1 leaves open

- The private HPCON layout and `ConptyPackPseudoConsole`: each conpty pin repeats prototype 1; a
  pin with a new layout needs its own entry in the table, and a pin with a new signal format also
  needs a writer for the old one.
- Ghostty calls version 1 a work-in-progress format; the pin check guards each pin change.
- A program that stops inside a sequence, or output that reaches no ground within a second,
  refuses its quiesce as passing, so its terminal stays on its Host until a later try finds it at
  ground.
- A program that reads no input can keep the writer from finishing, so its terminal refuses to
  quiesce until the program reads again.
- A record over 128 MiB keeps its terminal on its Host. Claiming at READY, if H3 adds it, reaches
  only Hosts released with it, so such a terminal on an earlier Host keeps the P1c behavior for as
  long as its program runs.
- A terminal holding an image never leaves an H1 or H2 Host, since the image section comes with
  H3, so it keeps the P1c behavior for as long as its program runs.
- One creation builds at a time, so a restore holds the other creations in its Host while it
  builds, which S9 measured at up to 26 milliseconds at the default scrollback and 345 at 100 MB
  of history.
- A Keeper that stops answering delays the ends of the terminals it holds until the Worker ends it.
- Settling a claim whose answer was lost ends the Keeper, which would end a program that only the
  Keeper holds, so from H2 the Worker settles it after pending recoveries.
- Between a claim and its drop the old copy keeps OpenConsole open, so a program that ends on the
  new Host cannot settle until the drop. With the Worker the drop follows at once; after a lost
  Worker it waits for the replacement's adoption.
- A Session whose activator's Job refuses breakaway runs inside that Job, whose end ends every
  program, Keeper or not, as it does today.
- In a Session that is not elevated, a lower-integrity process of the same user can open the
  Keeper's pipe for reading and fill its unadmitted connections 2 seconds at a time. That can delay
  an attach and leave terminals unheld, but it sends no frame and ends no program.
- No test runs a peer as another user beyond the anonymous token; the Keeper shares the pipe's DACL,
  label, and same-user check with the Host's endpoint. The elevated Session is CI's: GitHub's
  Windows runner runs elevated, where the Keeper's Low-integrity probe finds even a read-only open
  denied by the High label, and the Keeper, link, and move witnesses pass there. Probe P1 and
  prototype 1 ran on one Windows build, so S6 and S8 repeat their checks under a blocked read
  wherever their Windows suites run.
- H1's release freezes what an H1 Host does with a Keeper and as the old Host, because every later
  Keeper and Worker must serve H1's Hosts: its `keeper` offer, frames, and 2-second reply bound;
  `hello`, `hold` with its facts and handle encoding, `release`, `mark`, and `resume` with the
  answers it reads, so a later Keeper answers an H1 Host's `resume` of its current mark only with
  `resumed` or `claimed`; the Keeper check of `AttachKeeper`, `DetachKeeper`, the events, holding
  again after `KeeperLost`, release first with its deadline, and the limit of two attachments;
  `QuiesceTerminal`, settled exports included, `ReadHandoverRecord`, `ResumeTerminal`, and
  `DropTerminal`; the inventory states and holding ids; `dispatch-progress`; the records it writes;
  and the rules above. No release Session moves a terminal into an H1 Host, since recovery and
  moves claim into a Host from the Worker's own installation, so the claimant's requests first meet
  another version later and freeze then: `inspect`, `claim`, and `decline` with H2's Keeper, and
  the import requests with H3's Worker. The Worker's Keeper requests freeze with H2's release.
- H1's evidence is same-build; moves across versions are H3's.
- ARM64 and system-ConHost terminals never move.

## H2 design

The H2 builder settled this design on 2026-11-21 from H1's design, P1a's replacement, and the
coordinator's brief. Each pick is marked Chosen, which the owner may veto, or Rule, and rows 56 to
65 of the [decision record](#decision-record) list them with their reasons.

### Recovery

- Chosen: recovery runs through P1a's replacement. A Host that ends fails its Worker's connection,
  and the Worker exits as it does today. Where the Server's Supervision finds the Host gone, it
  stopped the Session before H2; from H2, when the Session record names a running Keeper, it
  replaces the Worker with a recovery target instead: the Keeper's recorded identity and the lost
  Host's epoch, which every binding names. The replacement adopts the Keeper as decision 31 says,
  starts a recovery Host, recovers each terminal into it, and imports the recovery Host's
  inventory into its kernel as an adoption does. P1a's quarantine, missing bindings, restart cap,
  `GivenUp`, and the Clients' Reconnecting and Can't reconnect states apply unchanged, so a fourth
  failure inside 10 minutes leaves the programs running in the Keeper and the Session shown as
  Can't reconnect.
- Chosen: the recovery Host takes the lost Host's epoch, which `serve --host-epoch` passes, and
  each recovered terminal keeps its creation id and terminal id, so the Server's bindings and
  fences stand and nothing is rebound. A Host refuses a recovery whose holding names another
  epoch. Rebinding comes with H3.
- Recovering one terminal: the Worker sends the Keeper `assign` of the orphaned holding that the
  binding's Host terminal identity names, for the recovery Host, under a mark it mints, and then
  sends that Host `RecoverTerminal` with the holding, the mark, the binding's identity, a size, and
  the launch profile the Worker resolves now. The Host builds a fresh engine and takes H1's claim
  steps with it: `inspect` through its one Keeper, the pull, the claim checks, the pack, the gated
  threads, `claim`, and the gate. Every step before the claim is undoable; a failure sends
  `decline` and leaves the holding orphaned with its program running.
- Chosen: recovery seeds no screen. The recovered terminal starts empty at the size the Worker
  names, and the Host resizes the console once after the claim, so a full-screen program redraws
  and a shell shows its next output; the rows on screen and the history before the loss are lost.
- Rule: the Worker recovers from one Keeper only. Each Host it starts attaches that Keeper alone,
  and a Host lost during H4's Keeper change fails that change first, so one Keeper arbitrates each
  orphan. The model's single-Keeper guard on `Assign` is this rule.
- Chosen: a terminal whose recovery fails keeps its program running in the Keeper and fails the
  replacement, so the Server tries again under P1a's cap, with the recovery Host adopted and the
  remaining orphans recovered into it; a terminal already recovered stays recovered. An orphan of
  the lost Host that no binding names is discarded after the import, which ends its program.
- Chosen: the hang check. The Worker reads its Host's `dispatch-progress` with `query` on a control
  lane every 2 seconds. A count that has not grown for 10 seconds, or a Host that has not answered
  for as long, counts as hung; when the Keeper holds every running terminal of that Host, the
  Worker ends the Host, and recovery follows as for a crash. A hang check never ends a Host that
  serves a terminal the Keeper does not hold.

### The Keeper's life from H2

- The Worker starts the Keeper, registers each Host, sends `AttachKeeper`, and adopts a running
  Keeper as [the Keeper's life](#the-keepers-life) says. The Session record gains a Keeper slot
  with the Worker as registered parent, and the recovery Host takes the Host slot of the Host that
  ended, once the Keeper reports it ended.
- Chosen: a Keeper that ends, or does not answer the Worker within its 2-second reply bound, is
  ended by the Worker and replaced; each Host holds again in the new Keeper after `KeeperLost`.
  The Server's stop without a Worker sends the Keeper `exit`, checked by its recorded process id
  and creation time, so stopping the Session still ends every program.
- `keeper-fatal.json`, `keeper-fatal.previous.json`, and `keeper-diagnostics.jsonl` join what
  deleting a Session removes.
- What a person sees needs no new state or copy: Reconnecting while the replacement recovers,
  Can't reconnect once P1a's cap gives up, in both Clients and both locales.

### H2 slices

| Slice | Content                                                                                                                                                                      | Status |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| S1    | The Keeper's `recovery`: `assign`, `discard`, `not-orphaned`, their fixtures and real-process witnesses, and the model's `Assign` and `Recover` with the single-Keeper guard | Built  |
| S2    | The Host: `serve --host-epoch` and `RecoverTerminal`, witnessed with two Host processes and a Keeper                                                                         | Open   |
| S3    | The Session record: the Keeper slot, the Host slot for the recovery Host, the Keeper's files in deletion, and `keeper_live`                                                  | Open   |
| S4    | The Worker's `keeper` client: start, registration, `AttachKeeper`, adoption, replacement, `exit` at a stop, and the `keeper` offer to Hosts                                  | Open   |
| S5    | The recovery target in `AdoptHost`, the Server's decision for a gone Host with a running Keeper, recovery in adopt mode, the hang check, and the stop without a Worker       | Open   |
| S6    | The [H2 acceptance](#acceptance) on real Windows, `keeper_live` on in release, and the freezes of row 42                                                                     | Open   |

## Acceptance

- **H1**, on real Windows in a test-owned Session: a same-build move of a terminal running a
  numbered-line probe, and of one running a full-screen program, keeps each process id, loses and
  repeats no number, and leaves the screen and history unchanged. After the move, typed input and a
  resize reach the program through the new Host, and closing the terminal there ends the program
  and its OpenConsole. A move during output whose sequences cross reads parks only at ground, and
  one while a program stops inside a sequence is refused as passing and leaves the terminal on its
  old Host. A failure forced at every step leaves the terminal on its old Host, with the handle
  counts of the Hosts and the Keeper back at their baselines, and ending the old Host after the
  quiesce, or the Keeper at any step, leaves the program running. The model catches each mutant
  that [the move's model](#the-moves-model) names, including one that drops the claim's guard, and
  every slice in [H1 slices](#h1-slices) passes its own check.
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

| #   | Decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Reason                                                                                                                                                                                                                                          | Status                                                             |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| 1   | Host handover is the normal way a Host change reaches running terminals: automatic, on by default, with no click or confirmation, and never waiting for programs to end; the plan reaches a point where every update, including one that changes the process holding the programs, applies with every program running                                                                                                                                                        | Agents may never end; updates must roll                                                                                                                                                                                                         | Decided 2026-11-15                                                 |
| 2   | A failed move leaves the terminal on its old Host, which keeps serving, and the Worker tries again when a newer version becomes active                                                                                                                                                                                                                                                                                                                                       | Non-stop outranks every other goal                                                                                                                                                                                                              | Rule                                                               |
| 3   | A per-Session Keeper holds what keeps programs alive and parents them on macOS and Linux; it never reads or writes terminal data                                                                                                                                                                                                                                                                                                                                             | Parenthood cannot move, and a parsing defect must not end programs                                                                                                                                                                              | Chosen                                                             |
| 4   | The Keeper is a mode of the Terminal Host executable, started, recorded, and adopted by the Worker                                                                                                                                                                                                                                                                                                                                                                           | It reuses the platform layer and the Host's start and adoption                                                                                                                                                                                  | Chosen                                                             |
| 5   | The Keeper's protocol changes only by addition, so every Host and Keeper inside the window work together                                                                                                                                                                                                                                                                                                                                                                     | Hosts of several versions share one Keeper                                                                                                                                                                                                      | Rule                                                               |
| 6   | The commit is the new Host's claim in the Keeper, and a claim and a resume exclude each other                                                                                                                                                                                                                                                                                                                                                                                | Exactly one Host reads a terminal, whatever crashes                                                                                                                                                                                             | Rule                                                               |
| 7   | The Worker holds a moving terminal's input, resizes, and viewer changes and delivers them once and in order; more than 64 KiB of input aborts the move and goes to the old Host                                                                                                                                                                                                                                                                                              | No input is lost or replayed                                                                                                                                                                                                                    | Chosen                                                             |
| 8   | The record carries Ghostty's snapshot version 1 in an Orchbee envelope that names the Ghostty pin; a Host imports a snapshot from its own pin or from a pin the pin check admitted, and a pin change that fails the check needs a reader for each pin inside the window                                                                                                                                                                                                      | A move must never depend on two Hosts sharing one build; prototype 2 found that Ghostty may change version 1 without a new version number                                                                                                       | Rule                                                               |
| 9   | Terminals move one at a time after 2 seconds without input or after 10 minutes; a move aborts 5 seconds after its quiesce; the first failure stops moves for that version                                                                                                                                                                                                                                                                                                    | A defect costs one paused terminal, and typing is rarely interrupted                                                                                                                                                                            | Chosen                                                             |
| 10  | A Host that crashes, or makes no progress for 30 seconds while the Keeper holds its terminals, is replaced; its terminals come back under a cap of 3 recoveries in 10 minutes, a terminal that breaks its Host again is recovered alone, and keys typed during recovery are refused                                                                                                                                                                                          | Contain a parsing defect to the terminal that caused it                                                                                                                                                                                         | Chosen                                                             |
| 11  | Recovery turns on as soon as it passes, before moves                                                                                                                                                                                                                                                                                                                                                                                                                         | It removes the worst open incident and needs no P1c                                                                                                                                                                                             | Chosen                                                             |
| 12  | Windows turns moves on at H4 without waiting for macOS                                                                                                                                                                                                                                                                                                                                                                                                                       | Release often                                                                                                                                                                                                                                   | Chosen                                                             |
| 13  | Until B1, a Server and Worker restart waits while the Worker runs a background program; B1 runs them in the Host                                                                                                                                                                                                                                                                                                                                                             | Non-stop covers every program                                                                                                                                                                                                                   | Rule; B1 Chosen                                                    |
| 14  | A terminal that cannot move keeps the P1c behavior and, at a security floor, the confirmation                                                                                                                                                                                                                                                                                                                                                                                | The last resort stays explicit                                                                                                                                                                                                                  | Rule                                                               |
| 15  | From H4, and H5 on macOS, the Worker replaces the Keeper at every update that changes the Terminal Host executable, before any move: on Windows by deposit into a new Keeper, on macOS and Linux by re-execution in place after a check run, with the old binary as the way back                                                                                                                                                                                             | Every update must reach a terminal that never ends; Docker's shims never change and its live restore spans only patch releases                                                                                                                  | Chosen                                                             |
| 16  | A Host sends each OpenConsole only signals in that OpenConsole's format, named in the record, and refuses to claim a terminal whose format it does not know                                                                                                                                                                                                                                                                                                                  | OpenConsole closes every program on a signal it does not know, and the clear signal changed in 2025; prototype 1 measured both pins, which share one format                                                                                     | Rule                                                               |
| 17  | A terminal keeps the console it started with; if a security floor names OpenConsole, the confirmation lists the terminals still using the old one                                                                                                                                                                                                                                                                                                                            | No product can move a console after its program attaches                                                                                                                                                                                        | Rule                                                               |
| 18  | The Keeper speaks one negotiated connection kind, `keeper`, at generation 1: OBHS, then `OBKP` frames of strict JSON, a `hello` naming the role, one request at a time per connection answered within 2 seconds, and additions only under capabilities; it drops no generation within 1.x                                                                                                                                                                                    | It reuses the handshake, decoders, and evolution rules every connection has; a Host released with H1 may hold a terminal for as long as its program runs                                                                                        | Chosen                                                             |
| 19  | The Keeper admits one Worker connection at a time and only Hosts the Worker registered, each checked by process id and creation time through the pipe; a failed check closes only that connection, and nothing a peer does ends the Keeper or a holding                                                                                                                                                                                                                      | The Host's same-user boundary and single broker session; the Keeper may be a program's only holder                                                                                                                                              | Rule; one Worker connection Chosen                                 |
| 20  | Handles move by pull, made by the process that will hold the duplicate; before the commit no process does I/O on a pipe handle or asks for its name                                                                                                                                                                                                                                                                                                                          | A lost message never strands a handle; a second synchronous operation waits behind a blocked read, as prototype 3 measured                                                                                                                      | Chosen; no I/O Rule                                                |
| 21  | The claim is the only commit: every fallible step comes before it, and a Host reads only as the Keeper's owner after seeing that answer; without the Keeper's `resumed`, the old Host reads again only after the Worker has ended the Keeper and the new Host has discarded its import without seeing `claimed`, even when the old Host saw `claimed`, so a moved terminal keeps its handles until `DropTerminal`, which follows only the new Host's `claimed`               | Exactly one Host reads, whatever crashes or whichever reply is lost; R1 found that a claim whose answer the new Host lost left no Host able to read                                                                                             | Rule; widened by R1                                                |
| 22  | The old Host exports before it marks; a mark expires at the deadline `QuiesceTerminal` names, from 1 to 30 seconds and 5 by default, when the old Host asks the Keeper to resume; the new Host declines the mark before it discards, and discards by itself once the deadline `ImportTerminal` names passes                                                                                                                                                                  | A local failure never depends on the Keeper, and either Host can end a mark; R1: a Host constant would bind every later Worker, and a lost Worker must not leave an import keeping the old Host's console from ending                           | Chosen; deadlines as members R1                                    |
| 23  | The Host parks its reader thread for a quiesce; the platform read reports a pause apart from end of output, and an aborted read reports no bytes                                                                                                                                                                                                                                                                                                                             | Prototype 3 resumed a parked reader 159,984 times without a stray cancel                                                                                                                                                                        | Chosen                                                             |
| 24  | H1 tracks no continuation: the reader parks only after a batch that leaves Ghostty's parser and the Host's scanners at ground, within 1 second, or the quiesce refuses as passing and the terminal stays on its old Host; the record keeps its continuation record and scanner state, written empty by an H1 Host                                                                                                                                                            | A quiesce then never splits a sequence, a dark H1 adds no parser path to every release terminal, and a later phase can add tracking without a new frame format                                                                                  | Chosen; revised by row 45                                          |
| 25  | The record is an `OBHR` envelope of checksummed sections, registered as frame format `obhr`, which drops no generation within 1.x; it crosses whole before the claim, in chunks of up to 4 MiB and up to 128 MiB; claiming at READY waits for H3's decision run                                                                                                                                                                                                              | The registry then enforces the import rule; 128 MiB keeps a move near 1.5 seconds and covers about 400 MB of styled history                                                                                                                     | Chosen                                                             |
| 26  | Only terminals on the bundled OpenConsole are held and move; a running terminal moves with a claim and a settled one as a record; awaiting-start, finishing, containing-failure, terminating, and image-holding terminals stay                                                                                                                                                                                                                                               | Only the bundled conpty packs a pseudoconsole; a draining Host must exit while a finished terminal stays on screen                                                                                                                              | Rule for the bundle; the rest Chosen; row 51 details settled moves |
| 27  | A Host signals a moved console through its own conpty, so it claims only a console whose build, named by the record's SHA-256, is in its table with its own pin's signal format; it does not read the running image again; the HPCON layout it reads is always its own conpty's                                                                                                                                                                                              | Decision 16; a console's build is fixed when it starts                                                                                                                                                                                          | Rule; no second read Chosen; layout by R1's second run             |
| 28  | Every Host from H1 has each Keeper release a held terminal before it closes the terminal's pseudoconsole, signal, reference, or Job handle or terminates its Job, waiting at most 2 seconds; Keeper I/O runs on a link thread                                                                                                                                                                                                                                                | A remaining duplicate keeps OpenConsole and the close event from ending, as prototype 1 measured                                                                                                                                                | Rule; 2 seconds and the link thread Chosen                         |
| 29  | An old Host drops a moved terminal, and a new Host discards an import, by first stopping and joining every thread of the terminal, its wait thread waiting on a stop event as well as the program, and then closing only its own handles, the pseudoconsole's three through the pinned layout, without `ClosePseudoConsole`, a signal, input, or ending the Job or changing its limits; a quiesced or moved terminal takes no lifecycle step in the old Host                 | Ending the Job or closing the console would end the program that the other Host serves; R1: a wait thread left blocked would later close a pseudoconsole through handle values that may name another terminal's                                 | Rule; thread stop R1                                               |
| 30  | H1 adds no switch: no release Worker lists `keeper` or `handover` before H2; `keeper_live` turns the Keeper on in H2 and `host_moves_live` turns moves on in H4, each true in test builds and false in release builds until then                                                                                                                                                                                                                                             | A capability works only when both sides list it; `automatic_updates_live` set the precedent                                                                                                                                                     | Chosen                                                             |
| 31  | From H2 the Worker starts one Keeper per Session after its first Host is ready, in the Session's Job placement, in a Keeper slot of the Session record, and P1a and P1b adopt it as they adopt a Host; it exits after 30 seconds without holdings, registered Hosts, or a Worker connection; it holds at most 1,024 holdings and 256 Hosts                                                                                                                                   | Decision 4; the Keeper must outlive the Worker                                                                                                                                                                                                  | Chosen; placement corrected by R1's second run                     |
| 32  | On Windows, H4 replaces the Keeper by holding every terminal in a second Keeper before the first exits; a Host holds at most two Keeper attachments and refuses a quiesce while it has two                                                                                                                                                                                                                                                                                   | Every holder keeps the same objects alive, so either Keeper suffices, and one Keeper arbitrates each move                                                                                                                                       | Chosen                                                             |
| 33  | The move's model is a TLA+ model of the Worker, both Hosts, and the Keeper, with crashes, lost replies, a second Keeper, and two quiesces; its invariants are one reader, no loss or repeat, each key at most once and exactly once while its Worker lives, an abort that restores the old Host, only a claim committing, holdings that name only Hosts that may read, records that end only at ground, and a reader that remains while both Hosts run; nine mutants test it | Four independently ordered actors share one small protocol; R1: a crashed Worker loses the keys it held, and the Host rules for a second Keeper freeze with H1; R1's second run: no property caught a drop that lost the last copy able to read | Chosen; invariants Rule; amended by R1 and its second run          |
| 34  | Each quiesce has a random 128-bit mark id, minted by the old Host and carried in its record; `mark`, `resume`, `inspect`, `claim`, and `decline` name it, `inspect` and `claim` of another mark get `stale-mark`, and a Host keeps one import per holding                                                                                                                                                                                                                    | A record of an earlier quiesce misses what the old Host read after it resumed, and a late `decline` must not clear a newer mark                                                                                                                 | Rule (R1)                                                          |
| 35  | After the reader parks, a quiesce waits again for the writer and the signal sender to go idle, and otherwise resumes the reader and refuses as passing                                                                                                                                                                                                                                                                                                                       | The last batches can queue terminal replies and resizes, which would be lost or interleave with the new Host's writes                                                                                                                           | Rule (R1)                                                          |
| 36  | A restore creates every thread of the terminal behind one gate and builds its process record from the duplicates, with the checked OpenConsole as its console reference; the claim only opens the gate                                                                                                                                                                                                                                                                       | A thread that failed to start after the commit would leave a claimed terminal without its reader or its exit observation                                                                                                                        | Rule (R1)                                                          |
| 37  | Whichever answer first shows the new Host `claimed`, to `claim` or to `decline`, completes its claim; the old Host reads any answer but `resumed` or `claimed`, or one it cannot decode, as none; the Keeper answers the old Host's `resume` with `claimed` after a claim, also once the claimant has ended                                                                                                                                                                  | Each Host acts only on answers that settle the holding, and an answer that a later Keeper extends never resumes a terminal                                                                                                                      | Rule (R1)                                                          |
| 38  | No path that ends terminals, `exit` and both teardowns included, ends or contains a terminal its Host does not read: a quiesced terminal ends only after `resumed`, a moved one or a quiesced one without it is dropped, and a restored import is discarded                                                                                                                                                                                                                  | Ending one would end a program that the other Host serves or may serve                                                                                                                                                                          | Rule (R1)                                                          |
| 39  | A Host holds in a Keeper only the terminals it reads; a quiesced terminal resumes only through the Keeper holding its mark, or by the Worker once that Keeper is lost or unresponsive; `DetachKeeper` is refused while that Keeper holds a mark or an import's source; `DropTerminal` also takes a quiesced terminal whose mark's Keeper is lost or unresponsive                                                                                                             | A quiesced terminal held again in a second Keeper could be resumed there while the new Host reads                                                                                                                                               | Rule (R1)                                                          |
| 40  | The Keeper serializes only the application of requests; each connection does its own I/O and keeps one reply, and a peer that does not take its reply in time loses its connection                                                                                                                                                                                                                                                                                           | A peer that stops reading must not stall every other connection and get the Keeper ended                                                                                                                                                        | Rule (R1)                                                          |
| 41  | The macOS Keeper also observes each leader with the Host wait worker's bounded non-reaping `waitid` poll; neither `ESRCH` nor a silent filter counts as an exit with a status                                                                                                                                                                                                                                                                                                | XNU can drop the process reference before the child is waitable and can lose `NOTE_EXIT`, as the runtime model records                                                                                                                          | Rule (R1)                                                          |
| 42  | H1's release freezes the old Host's side; `inspect`, `claim`, and `decline` freeze with H2's release, the import requests with H3's, and the Worker's Keeper requests with H2's                                                                                                                                                                                                                                                                                              | No release Session moves a terminal into an H1 Host, and no release Keeper starts before H2                                                                                                                                                     | Chosen (R1); widened by row 54                                     |
| 43  | `DuplicateHandle` and the type, basic, process, and Job queries do not wait behind another process's blocked synchronous read, so a Host holds without parking its reader; in H2's recovery a replacement may pull and check a hung Host's handles before that Host has ended, but reads only after it has ended                                                                                                                                                             | Probe P1: 0 waits in 1.2 million calls while another process's read was blocked, through the pinned conpty, the system ConHost, an anonymous pipe, and a named pipe; the forbidden calls waited every time                                      | Rule (probe P1)                                                    |
| 44  | The Keeper's 2-second reply deadline stays a constant, and S6 reports the Keeper's reply time at capacity under full processor load                                                                                                                                                                                                                                                                                                                                          | H1's Hosts apply it to every later Keeper; a measurement before H1's release settles it more cheaply than a parameter                                                                                                                           | Chosen (R1)                                                        |
| 45  | Row 24 changes from continuation tracking in every build, with a 64 KiB bound, to parking the reader at ground, with the record's continuation written empty                                                                                                                                                                                                                                                                                                                 | R1 recommended it and the coordinator picked it: the Worker moves after 2 seconds without input, when an unfinished sequence is rare, and a dark H1 then adds no parser path to every release terminal                                          | Chosen (R1)                                                        |
| 46  | After the new Host's `claimed` the Worker drops the old copy at once, and a Worker that adopts a Host with a moved terminal drops it once the holding's new owner reports it running; inventory names the holding id of every held, quiesced, moved, and restored terminal                                                                                                                                                                                                   | The old copy holds OpenConsole's reference handle, so until the drop a program that ends on the new Host cannot settle                                                                                                                          | Rule; holding ids Chosen (R1 second run)                           |
| 47  | A handle value stays valid while a peer may pull it: the Keeper closes a holding's duplicates only at `release`, refused while marked, and when it ends; a Host keeps the handles an unanswered `hold` named until that Keeper has ended, and pulls only through the Keeper process handle it checked at attach                                                                                                                                                              | A claim under the inspected mark then proves that the pulled handles were the holding's, which no check can show for a pipe before the commit                                                                                                   | Rule (R1 second run)                                               |
| 48  | Every duplicate keeps its source's access and is not inheritable; the claim checks that each handle grants what the Host does with it after the commit; S7 proves the hold's mapping object by object                                                                                                                                                                                                                                                                        | No child becomes a holder, and no step after the commit fails for want of access; type and access cannot tell the signal end from the input end, and keys sent as signals would close every program                                             | Rule (R1 second run)                                               |
| 49  | The Keeper and the Hosts of a Session run at one integrity level, and a mismatch fails `hold` or the pull and ends nothing; no other user and no lower level can send the Keeper a frame or open it; within one user and level the protocol adds nothing to the Host's trust boundary                                                                                                                                                                                        | A lower process cannot open a higher one for duplication, and the pipe's DACL, its label, and no-write-up keep the others out                                                                                                                   | Rule (R1 second run)                                               |
| 50  | The Keeper serves at most 256 Host connections, one Worker connection, and 8 more not yet admitted or carrying a control request, under its own pipe-instance limit; a Host counts the Keeper unresponsive only for a late reply on an open link                                                                                                                                                                                                                             | The Host's Local IPC has five pipe instances, and a burst of connections must neither end the Keeper nor get it ended                                                                                                                           | Chosen; unresponsive Rule (R1 second run)                          |
| 51  | A settled terminal moves once its outcome is published: exported with no mark, imported settled with that outcome published, never sending `TerminalExited` again, and its old copy disposed as any settled terminal is                                                                                                                                                                                                                                                      | Row 26 named settled moves without their requests, and an outcome is published exactly once                                                                                                                                                     | Chosen (R1 second run)                                             |
| 52  | A Host keeps at most one export and one import at a time                                                                                                                                                                                                                                                                                                                                                                                                                     | A record may reach 128 MiB, and moves run one at a time                                                                                                                                                                                         | Chosen (R1 second run)                                             |
| 53  | `ImportTerminal`'s deadline runs from 1 to 30 seconds, and the Worker sets it to the mark's; a discard, or that deadline, during the restore's build discards the import when the build ends; teardown discards a restored import instead of containing it                                                                                                                                                                                                                   | The creation pipeline cancels no creation and contains what its build made, and a restore must end nothing                                                                                                                                      | Rule; range Chosen (R1 second run)                                 |
| 54  | H1's release also freezes the Host's `keeper` offer, frames, and reply bound, `hold`'s facts, the Keeper check, holding again after `KeeperLost`, release first with its deadline, the two-attachment limit, settled exports, the inventory states and holding ids, and `dispatch-progress`; a later Keeper answers an H1 Host's `resume` of its current mark only with `resumed` or `claimed`                                                                               | Every later Keeper and Worker must serve H1's Hosts, and an H1 Host reads any other answer as none                                                                                                                                              | Chosen; widens row 42 (R1 second run)                              |
| 55  | The model adds the invariant that a reader remains while both Hosts run, and the mutant of a drop before the new Host has seen `claimed`; its resume stands for every path that settles a quiesced terminal in the old Host                                                                                                                                                                                                                                                  | The drop's order keeps the only copy that can read again, and no property caught its loss; S2 had landed, so this reopens it                                                                                                                    | Invariant Rule; reopens S2 (R1 second run)                         |
| 56  | Recovery runs through P1a's replacement: when the Server finds the Host gone and the Session record names a running Keeper, it replaces the Worker with a recovery target instead of stopping the Session                                                                                                                                                                                                                                                                    | It reuses P1a's adoption, import, quarantine, cap, `GivenUp`, and Client states, and the Server already owns the decision and the cap                                                                                                           | Chosen (H2)                                                        |
| 57  | The recovery Host takes the lost Host's epoch, and each terminal keeps its creation id and terminal id, so no binding changes; rebinding stays with H3                                                                                                                                                                                                                                                                                                                       | The bindings and fences stand, and H2 depends on neither P1c nor H3                                                                                                                                                                             | Chosen (H2)                                                        |
| 58  | The Worker gives an orphaned holding to the recovery Host with `assign` under a mark it mints, behind the Keeper capability `recovery`, and the Host claims it with H1's claim steps and a fresh engine                                                                                                                                                                                                                                                                      | Only a mark's target claims a holding, so recovery adds a request and no second claim path                                                                                                                                                      | Rule (H2)                                                          |
| 59  | Recovery seeds no screen: the terminal starts empty, and the Host resizes the console once after the claim                                                                                                                                                                                                                                                                                                                                                                   | Seeding needs a console client on the program's console and its own decision run; a full-screen program redraws after a size change                                                                                                             | Chosen (H2)                                                        |
| 60  | The Worker recovers from one Keeper only, and a Host lost during H4's Keeper change fails that change first                                                                                                                                                                                                                                                                                                                                                                  | TLC found a second live Keeper still naming the ended Host as owner after a recovery through the first                                                                                                                                          | Rule (H2)                                                          |
| 61  | A terminal whose recovery fails keeps its program running in the Keeper and fails the replacement, which the Server retries under P1a's cap; a terminal already recovered stays recovered                                                                                                                                                                                                                                                                                    | One failed step must never end a program, and P1a's cap already bounds the retries                                                                                                                                                              | Chosen (H2)                                                        |
| 62  | An orphan of the lost Host that no binding names is discarded after the import, which ends its program                                                                                                                                                                                                                                                                                                                                                                       | No person can reach it, and the Keeper would otherwise keep it running until the Session stops                                                                                                                                                  | Chosen (H2)                                                        |
| 63  | The Worker reads its Host's `dispatch-progress` every 2 seconds, and a count that has not grown for 10 seconds, while the Keeper holds every running terminal of that Host, makes the Worker end the Host and recover                                                                                                                                                                                                                                                        | 10 seconds is the Worker's exchange bound, after which a silent Host already counts as lost, and ending a Host whose terminals are all held ends no program                                                                                     | Chosen (H2)                                                        |
| 64  | A Keeper that ends, or does not answer the Worker within its 2-second reply bound, is ended and replaced by the Worker, and the Hosts hold again after `KeeperLost`                                                                                                                                                                                                                                                                                                          | H1's Hosts hold again in the next Keeper they attach                                                                                                                                                                                            | Chosen (H2)                                                        |
| 65  | `keeper_live` turns on in release once the H2 acceptance passes on real Windows                                                                                                                                                                                                                                                                                                                                                                                              | The Phases row: H2 is on as soon as it passes                                                                                                                                                                                                   | Rule (H2)                                                          |

Nothing waits on the owner; each Chosen row stays open to a veto.
