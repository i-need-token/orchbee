# orchbee-server-workspace-files

The Workspace file service behind the window's right bar. The Server runs it in a
separate process of its own executable (`orchbee-server --workspace-files`) only after a Client
first sends `workbench.workspace-files`, so a Session whose windows never open the right bar starts
nothing. The process exits after ten idle minutes or when its standard input closes.

It reads one directory listing, one file from the working tree, the index or `HEAD`, or the change
list of `git status`, always beneath the Workspace directory the Server names, saves one
working-tree file, replaces what a search found, and stages, unstages, discards or commits. Reads write nothing: Git runs with optional locks off, so a status read never
refreshes the index.

The same executable also runs the watch lane (`orchbee-server --workspace-files-watch`), a second
process the Server starts only when a Client first watches what its right bar shows. It follows
each root with the platform's own watcher through `notify`: one recursive watch per root on macOS
(FSEvents) and Windows, and on Linux one inotify watch per directory, leaving out `.git`, what Git
ignores (unless a directory holds a tracked file or is shown), and anything past 65,536 watched
directories, which reports the root `unavailable` rather than following part of it.

A third process, the search lane (`orchbee-server --workspace-files-search`), starts with a
Client's first search or file listing and searches file contents, or lists file paths, with
ripgrep's own crates, so a long search delays neither the service's requests nor the watch.

## Protocol

Standard input and output carry one JSON object per line. A request is
`{"id": 1, "root": "/abs/workspace", "operation": {...}}`, where `operation` is the
`workbench.workspace-files` operation; the answer is `{"id": 1, "outcome": {...}}`, a
`workbench.workspace-files` outcome. Requests are answered in order. A `watch`, `search` or
`list-files` operation answers `failed` with `malformed`, because the Server serves them itself.

The watch lane reads one watch set per line,
`{"roots": [{"root": "/abs/workspace", "directories": [""], "files": ["a.txt"]}]}`, each replacing
the last, and writes one report per line: `{"kind": "state", "root": ..., "state": "watching"}`
once per root when it starts or its state changes, and
`{"kind": "changed", "root": ..., "files": true, "repository": false}` for the changes it
collected over 100 ms. `files` means an entry of a named directory or a named file changed;
`repository` means a working-tree path Git does not ignore, or the repository's index, `HEAD` or
references, changed. Ignore rules are Git's own: each directory's `.gitignore`, `info/exclude` and
the user's excludes file, with `core.ignorecase`, and a tracked file is never ignored. Objects,
logs and lock files inside Git's directories count for neither. A lost or overflowing event queue
reports both; a root that is removed or moved reports `unavailable`.

The search lane reads `{"kind": "start", "id": 1, "root": "/abs/workspace", "query": {...}}` and
`{"kind": "cancel", "id": 1}` and writes `{"kind": "found", "id": 1, "files": [...]}` with the
`workbench.workspace-files` search entries, in order, and finally
`{"kind": "ended", "id": 1, "state": "complete"}`. A cancelled search still ends with a report. A
search walks the root on up to eight threads, one fewer than the machine has, and reports about 64
lines at a time, or what it has after 50 ms. `{"kind": "list", "id": 2, "root": ..., "query": {...}}`
lists files instead, with a `list-files` query, and writes `{"kind": "listed", "id": 2, "paths":
[...]}` with at most 512 Workspace-relative paths at a time, or what it has after 50 ms, before
its `ended` report; searches and listings share the lane's identities and its limit.

## Searching

A search reads every file beneath the root except `.git`, following no symbolic link. With
`useIgnoreFiles` it leaves out what `.gitignore`, `.ignore`, `info/exclude`, the user's excludes
file and the ignore files of parent directories leave out; without it, nothing. Hidden files are
searched. Each include and exclude glob matches a path anywhere beneath the root, or, starting
with `./`, from the root; one ending in `/` names a directory, and globs ignore case on Windows and
macOS. Named `paths` are searched alone, with the globs but without ignore files. A listing names
the files a search with the same `excludes` and `useIgnoreFiles` would read, in no set order.

A literal pattern matches as written, a regular expression in ripgrep's syntax; either may span
lines, a literal one matching CRLF line ends as well as LF. A file with a NUL byte is binary and
gives nothing. Text is shown as UTF-8, with each invalid sequence as U+FFFD. Matches are grouped by
the line they start on; a preview runs to the end of the last match's line, and a line longer than
1000 UTF-16 code units shows the part from 40 units before a match, several parts for matches far
apart.

## Saving

`write-file` replaces a file only while its whole content still hashes to the `expected` SHA-256,
and creates one with a null `expected` only where nothing is there yet; anything else answers
`conflict` with the file's current version and writes nothing. A save never creates a directory,
never writes through a symbolic link that leaves the root or leads nowhere, and refuses a file
whose permissions make it read-only.

A replacement is written to a temporary file beside the target (`.<name>.<pid>-<time>.orchbee-save`),
given the target's permissions, synced and renamed over it, so no reader sees half of it; the
directory is synced on Unix. Just before the rename the target's size and modification time are
checked again, so a change since hashing answers `conflict`. Where a rename would lose something
the target has (another hard link, or on Unix an owner or group the service would not keep), where
the directory takes no new file, or where the platform refuses the rename, the file is truncated
and rewritten in place instead. A new file is created exclusively and written directly. A save is
never cut short inside the service; the Server waits up to 20 seconds for it.

## Replacing

`replace-matches` replaces matches a search found, each named by the line it starts on and the
UTF-16 code units of that line before it. Each file is read whole and searched again with the
search's own matcher, line by line or, for a pattern that spans lines, across the file, and its
positions are counted as a preview counts them. A file is written, as a save writes and against
the SHA-256 of what was read, only while every named match still starts a match there; it answers
`changed` otherwise, or when the save finds it changed since, and the other matches it holds are
left as they are. A file with a NUL byte, one that is not UTF-8, or one that is or would become
larger than 10 MiB answers `not-editable`.

Without `regex` the replacement is written as it stands. With it, `$1` to `$99`, `${name}` and
`$<name>` insert a group (a two-digit group that does not exist reads as the one-digit group and a
digit), `$0` and `$&` the whole match and `$$` a dollar sign, and `\n`, `\t` and `\\` a line feed,
a tab and a backslash; `\u`, `\l`, `\U` and `\L` change the case of the next group's first letter
or of all of it. `preserveCase` gives a replacement that inserts no group the matched text's case:
all upper, all lower, or a capital or small first letter, part by part where both hold the same
number of hyphens, or of underscores, but not both.

Files are replaced one at a time in the order named. A replacement runs in the lane of Git
changes, so it never holds up a read, and starts no file after 40 seconds; a file it did not start
answers `timeout`.

## Changing the repository

`git-stage` runs `git add -A` for the named paths, or for none every change beneath the Workspace
directory except conflicts. `git-unstage` runs `git reset` for the named paths or everything
staged beneath it, which on a branch with no commit removes them from the index. `git-discard`
moves each path `git status` reports as an untracked file to the system trash, on a thread of its
own (on macOS through `NSFileManager`, which asks for no permission to control Finder), and only
then checks the rest out of the index; Git checks every path first, so a conflict or a path it does
not know refuses the whole checkout, while files already in the trash stay there. A trash that
takes nothing, or not everything, answers `trash-unavailable` and checks nothing out. `git-commit` commits what is staged, staging every change that is
not a conflict first for `stageAll`, and an amend with an empty message keeps the last one. Paths
go to Git on standard input as literal, NUL-separated pathspecs and the message on standard input,
so neither is limited by a command line or read as a pattern.

Success answers `git-done`. A Git refusal answers `git-rejected` with what Git printed, decoded
lossily, stripped of escape sequences and control characters and cut to 8 KiB. Each change has 60
seconds; on Unix Git runs in a process group of its own, which gets `SIGTERM` at the deadline and
`SIGKILL` three seconds later, so hooks stop with it and no `index.lock` is left behind. On Windows
only Git itself is stopped.

## Limits

- Paths must stay beneath the canonical root; a symbolic link that leaves it is `denied`.
- A listing or change list carries at most 5000 entries and reports `truncated`.
- A file is read in chunks of at most 256 KiB from the offset a request names; a text chunk ends
  before a character it would cut, so the next chunk starts where it ended.
- A text read opens files up to 50 MiB and a byte read up to 10 MiB; a larger file answers
  `too-large` with its size. A text file with a NUL byte in its first 8 KiB, or that is not UTF-8,
  answers `binary`.
- A working-tree file carries its modification time; the first chunk of a file up to 10 MiB also
  carries the SHA-256 of the whole file. Index and `HEAD` blobs carry no version, are read up to
  10 MiB, and the last blob read is kept so its later chunks do not run Git again.
- A watch set names at most 64 roots, each with at most 256 directories and 64 files, and the
  Server keeps the whole set under 4 MiB. The lane exits after ten minutes with nothing to watch.
- A save carries at most 10 MiB of text, so a request line may reach 24 MiB.
- A search stops at 20,000 matches as `limit-reached`, and skips a line or multi-line match
  needing more than 64 MiB of memory. The lane runs eight searches at once and answers one more
  `unavailable`; the Server holds 64 across all windows, and a window's new search of a Workspace
  cancels its last. Its report lines stay under 4 MiB. The lane exits after ten idle minutes.
- A listing stops at 100,000 files as `limit-reached`. It counts against the same eight running
  and 64 held as searches, and a window's new listing of a Workspace cancels its last listing,
  not its search.
- A change names at most 256 paths and a commit message is at most 32 KiB.
- Each Git read has a four-second deadline and bounded output; Git's own environment variables
  are cleared, terminal prompts are off and `core.fsmonitor` is disabled.
