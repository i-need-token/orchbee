# Orchbee Files

`orchbee-files` serves the File Bar for one Worker generation: it lists, reads, watches and searches
files and runs Git for every Workspace of its Session on its machine. The Worker starts it when the
Server asks, and it then talks only to that Server, over the private OBFL link that
[`orchbee-files-protocol`](../../crates/files-protocol/README.md) defines. It accepts no Client
connection, reads no Orchbee setting and runs no command a Client names. Scope and containment are
checked here, against the Workspace roots only the Server sends; the Server relays File Bar records
without reading their paths.

## Executable interface

The Worker starts the service and the Git it runs starts the relay helper, so this interface is a
contract between programs rather than a command.

| Fact            | Contract                                                                                                                                             |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Invocation      | `--local-endpoint <endpoint>`: the absolute address of the one-shot endpoint the Server bound for this start                                         |
| Standard input  | The start's token, 64 lowercase hexadecimal digits and a line feed. It never appears among the arguments, which other processes of the user can read |
| Standard output | `ready` and a line feed once the Server admitted the hello; then, on macOS and Linux, each Git or rg group as it starts and once it settled          |
| Exit            | 0 after the Server's drain; `128 + signal` after `SIGTERM` or `SIGINT` on macOS and Linux; 1 for every other end, which the Server counts as a crash |

The service names this executable as `GIT_ASKPASS`, `SSH_ASKPASS` and, with `--git-editor <file>`,
`GIT_EDITOR`, together with `ORCHBEE_FILES_RELAY` and `ORCHBEE_FILES_RELAY_TOKEN`, only on the
commands of an operation that may prompt. Started with both variables and without
`--local-endpoint`, it is the relay helper: it asks that service's relay, prints the answer or waits
until the message file is done, and exits 0, or 1 when the prompt was cancelled. It reads no data
root and writes no log.

Every end cancels the open calls first and gives their operations three seconds to end their
children, so a Git or ripgrep process tree does not outlive the call that started it. A killed
process cannot, so the Worker ends the groups it reported and left running, as a
[group report](../../crates/local-runtime/README.md#retained-commands) does. A panic on
any thread ends the whole process instead of unwinding that thread, because a half-serving process
would leave Client streams open; the Server restarts it on the next demand.

## Scope and grants

These rules are the contract a Client builds on; no single declaration holds them.

- A path is in scope when it lies lexically inside its Workspace's root, spelled as the Workbench
  reported it or as the real location `fs.subscribe` reports; inside the working tree of a
  repository consented to in that Workspace, spelled as `scm.subscribe` names it, from the consent's
  answer until `repository.close` answers or the summary stops listing it; when it is the file an
  editor prompt of the Workspace names, from the prompt's arrival until it closes, where only a
  file whose real location lies in its repository's Git directory or common directory opens a
  prompt; or when it names a file the requesting connection opened. A Client builds its paths under the root `fs.subscribe`
  reported.
- `fs.grant` from a tree or a diff needs the path in scope already; a Save As destination, a
  terminal link, a restored tab or the operating system's open may name any path, since the person
  chose it explicitly. The entry is read before the grant is recorded, so a grant that fails grants
  nothing, and a directory is refused; a Save As destination where nothing is yet is granted
  without an entry, and the save creates it.
- An open picker session lists any directory and grants nothing else; `picker.close`, ten minutes
  without a request or the connection's end closes it. The paths of a connection's last
  `fs.clipboard`, each in scope when it arrived, stay readable and copyable, and movable when cut,
  after the root moves, until the next `fs.clipboard` or the connection's end.
- Each successful grant of a file adds one grant, and each path of `fs.ungrant` releases one, so
  two tabs of one file release it independently; a set would let closing either tab cut off the
  other. Grants outlive their Workspace's close and end with their connection.
- Grants live in this process. After it restarts, which ends every subscription, a Client grants
  its open files again.
- Search reaches past the root only through what the person typed or picked, and only under
  `files-search`: a text search reads a folder outside the root that the person typed as an
  include, for that search alone; `fs.stat` with source `quick-open` states the one path typed in
  Quick Open wherever it lies and admits nothing more; and `fs.grant` from a Quick Open pick may
  name any path, as a terminal link may.

## Local History

Local History keeps copies of the files the File Bar saves, as VS Code does, in the data root's
`internal/local-history/`, which every Session sharing the data root reads and changes under one
lock; a default uninstall keeps it, and without a data root nothing is recorded.

- It records a save whose `fs.write` names a `history` reason and each `history.restore`. A rename
  or a move takes the history of each file it moves along and records the file at its new path. A
  file larger than 256 KiB records nothing.
- A file keeps VS Code's default 50 entries. A save of the same reason within ten seconds of the
  newest entry replaces it, unless `history.rename` named that entry. All files together keep at
  most 1 GiB, and the oldest entries of any file go first.
- An entry is recorded after the operation that caused it answers, so a list asked at once may not
  show it yet. Recording never fails that operation; a failure is logged.
- `history.restore` writes the entry's bytes over its file after any File Bar write of that file
  ends, and records what it wrote as a restore.

## Windows Recycle Bin

On Windows the machine's trash is the Recycle Bin. This service moves an item there through the
shell's `IFileOperation` with its own adapter, the private `src/recycle_bin_native.rs`, which is its
one authored unsafe boundary, admitted by the
[Rust policy](../../docs/RUST.md#lint-policy-and-enforcement). The `trash` crate serves macOS and
Linux only: on Windows it asks the person whether to delete permanently an item the Recycle Bin
cannot take. A move names the entry below its folder's real location, as scope checked it, and
never resolves the entry itself, so a link or junction moves without what it names; a folder whose
real location has no drive path keeps the path as named.

- **Apartment.** Each move initializes COM on the calling thread as a single-threaded apartment
  with OLE1 DDE off, as `IFileOperation` requires, and an owner that cannot leave that thread
  uninitializes it once every COM object of the move is released, also after `S_FALSE`. A thread
  already in the multithreaded apartment is left as it was, and the move fails.
- **Path.** Only an absolute drive path below the root, without a `..` segment or a NUL, reaches
  `SHCreateItemFromParsingName`, as a NUL-terminated UTF-16 copy that outlives the call: the shell
  would parse a relative name against the Desktop, and a NUL would end the path at one of its
  folders. The shell refuses the verbatim `\?\` form, so a path past `MAX_PATH` also goes in plain
  form.
- **Flags.** `FOF_NO_UI` shows no progress, confirmation, error or elevation window;
  `FOFX_RECYCLEONDELETE` asks for recycling, as Windows 8 and later do; `FOF_NO_CONNECTED_ELEMENTS`
  moves the named item alone, never the files Windows connects to it, such as `page_files` beside
  `page.htm`; `FOFX_NOCOPYHOOKS` runs no shell extension copy hooks. `FOF_WANTNUKEWARNING` stays
  off, since even under `FOF_NO_UI` it asks whether to delete permanently an item that cannot be
  recycled. Electron's `shell.trashItem` also sets `FOFX_ADDUNDORECORD` and
  `FOFX_SHOWELEVATIONPROMPT`; a background service leaves no undo in Explorer and raises no UAC
  prompt, so an item that needs elevation fails.
- **Refusal.** The progress sink answers `E_ABORT` to a `PreDeleteItem` whose flags lack
  `TSF_DELETE_RECYCLE_IF_POSSIBLE`, the shell's announcement, before it begins, of a deletion it
  would make permanent, as on a volume without a Recycle Bin. Nothing is deleted, and the move is
  `trash-unavailable`, as Electron's `shell.trashItem` refuses it.
- **Outcome.** Success needs `PerformOperations` to succeed with no aborted operation, no failed
  `PostDeleteItem` and one that names the Recycle Bin's new item; a deletion without that item
  counts as a failure. Anything else is `trash-failed`.
- **Sink.** `#[implement]` makes the sink agile, so the shell may call it from any thread; its three
  reports are atomics, and the operation holds its own reference to it until it is released.

The adjacent safe tests run the real shell without moving anything into the Recycle Bin: its own
permanent deletion of a file and of a folder, refused with both kept (`trash-unavailable`); a file
another handle holds without delete sharing (`trash-failed`, kept); and a path with a NUL, which
never reaches it. A move into the Recycle Bin has no automated test, since every run would leave
items in the person's Recycle Bin. Native traces on Windows 11 moved a file, a folder, a junction
and links to a file and a folder without their targets, `page.htm` without its `page_files`, an
entry reached through a junction in its folder's path, five files from five threads at once, and a
path past `MAX_PATH`, which the shell reached through 8.3 short names. No trace covers a volume
without a Recycle Bin, which needs a removable or network volume, or a long path on a volume
without 8.3 short names.

## ripgrep

Development and test cohorts carry rg beside this executable, as `rg` (`rg.exe` on Windows), with
the notices of everything compiled into it in `licenses/ripgrep-notices.txt`; a release cohort
carries neither until the File Bar ships. [`ripgrep.json`](ripgrep.json) pins, by SHA-256, the
`@vscode/ripgrep-universal` archive, the program in it for each release platform and the notices,
and staging runs [`scripts/ripgrep.mjs`](../../scripts/ripgrep.mjs), which refuses any other bytes.
The pinned programs are ripgrep 15.0.0 with PCRE2 10.46 and its just-in-time compiler; the Linux
ones are static musl builds that allocate with jemalloc. `rg --version` names PCRE2 10.45 because
the bindings of `pcre2-sys` 0.2.10 still carry that number, while the library compiled in is the
10.46 source the crate bundles. Every program loads only system libraries. A new archive
regenerates the notices from that ripgrep's `Cargo.lock`, taking each crate it links on some
platform and the license files that crate ships, and changes the ripgrep entry of
[`THIRD_PARTY_NOTICES.md`](../../THIRD_PARTY_NOTICES.md#ripgrep) with them.

The service runs only the file beside its executable, never an rg on `PATH`, and only while its
SHA-256 equals the pin for its platform; otherwise every search answers
`unavailable{search-binary}`. Staging and the service therefore read the same pin, and a cohort
whose rg is not those bytes has no search.

## Search

Text search and Quick Open run rg with the arguments VS Code ef5ca45 builds for them. rg owns
traversal, ignore files and matching, as it does in VS Code; an in-process walker such as the
`ignore` or `grep-*` crates was rejected because it would find files VS Code does not.

- Parent ignore files are the one departure from those arguments. With `useIgnoreFiles` and
  without `useParentIgnoreFiles`, a folder below the top of its Git working tree drops
  `--no-ignore-parent` and `--no-require-git`, so the `.gitignore` files between the folder and the
  top apply, as Git applies them there, and none above the top does; VS Code applies none of them.
  Dropping only `--no-ignore-parent` was rejected: rg would then read every ancestor's
  `.gitignore` up to the filesystem root, a home directory's one for dotfiles included. The top is
  the nearest directory holding `.git`, a directory or a linked worktree's file, found when each rg
  starts.
- rg's globs match more than VS Code's (`**.js` matches at any depth), so every file rg reports is
  matched again against the request's includes and excludes with VS Code's glob semantics, as VS
  Code's `TextSearchManager` does.

## Adding a service

A service joins in `src/operations.rs`: its state as a field of `Services`, its operations as arms of
`route`, and its share of each link event that ends state: a cancel, a lane's close, a connection's
loss, a scope change and the drain. Its own modules take the shared parts they use, such as
`Scopes`, `Helpers` or a `Subscriptions` registry, and never `operations`, which depends on them;
that keeps the module graph acyclic. A filesystem call that can hang runs through `Helpers::run`.

## Diagnostic log

The process writes to the [daily diagnostic log](../../crates/local-runtime/README.md#diagnostic-log)
with role `files`: `started` once the Server admitted it; `filesystem-timeout` with the `operation`
whose filesystem call passed its deadline; `git-timeout` with the `kind` of Git command that passed
its deadline; `git-spawn-failed` with that `kind` and the I/O `error` kind when Git could not start;
`git-relay-unavailable` once when the prompt relay could not start, after which Git operations run
without prompts; `rg-refused` once for each state of the rg beside the executable that is not the
pinned program, with the I/O `error` kind when it could not be read; `rg-spawn-failed` with the I/O
`error` kind when the verified rg could not start; `history-not-recorded` with the `operation` whose
save, restore or move Local History could not record and the `error` (`busy`, `newer`, `not-found`
or the I/O kind); and one `exited` with `outcome` (`drained` with its `reason`, `idle` or `session`;
`signalled` with its `signal`; or `failed` with the `stage` it stopped at and the I/O `error` kind
that caused it, when one did).
