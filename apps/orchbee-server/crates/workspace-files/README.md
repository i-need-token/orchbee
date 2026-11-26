# orchbee-server-workspace-files

The Workspace file service behind the window's right bar. The Server runs it in a
separate process of its own executable (`orchbee-server --workspace-files`) only after a Client
first sends `workbench.workspace-files`, so a Session whose windows never open the right bar starts
nothing. The process exits after ten idle minutes or when its standard input closes.

It reads one directory listing, one file from the working tree, the index or `HEAD`, or the change
list of `git status`, always beneath the Workspace directory the Server names, saves one
working-tree file, and stages, unstages, discards or commits. Reads write nothing: Git runs with optional locks off, so a status read never
refreshes the index.

The same executable also runs the watch lane (`orchbee-server --workspace-files-watch`), a second
process the Server starts only when a Client first watches what its right bar shows. Watching
waits on a platform watcher that is not yet admitted, so for now every root reports `unavailable`
and the right bar refreshes only when asked.

## Protocol

Standard input and output carry one JSON object per line. A request is
`{"id": 1, "root": "/abs/workspace", "operation": {...}}`, where `operation` is the
`workbench.workspace-files` operation; the answer is `{"id": 1, "outcome": {...}}`, a
`workbench.workspace-files` outcome. Requests are answered in order. A `watch` operation answers
`failed` with `malformed`, because the Server serves watches itself.

The watch lane reads one watch set per line,
`{"roots": [{"root": "/abs/workspace", "directories": [""], "files": ["a.txt"]}]}`, each replacing
the last, and writes one report per line: `{"kind": "state", "root": ..., "state": "watching"}`
once per root when it starts or its state changes, and
`{"kind": "changed", "root": ..., "files": true, "repository": false}` for the changes it
collected over 100 ms. `files` means an entry of a named directory or a named file changed;
`repository` means anything in the working tree, or the repository's index, `HEAD` or references,
changed. Objects, logs and lock files inside `.git` count for neither. A lost or overflowing event
queue reports both.

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

## Changing the repository

`git-stage` runs `git add -A` for the named paths, or for none every change beneath the Workspace
directory except conflicts. `git-unstage` runs `git reset` for the named paths or everything
staged beneath it, which on a branch with no commit removes them from the index. `git-discard`
checks tracked paths out of the index; Git checks every path first, so an untracked path or a
conflict refuses the whole list. `git-commit` commits what is staged, staging every change that is
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
- A change names at most 256 paths and a commit message is at most 32 KiB.
- Each Git read has a four-second deadline and bounded output; Git's own environment variables
  are cleared, terminal prompts are off and `core.fsmonitor` is disabled.
