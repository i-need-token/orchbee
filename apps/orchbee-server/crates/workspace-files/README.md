# orchbee-server-workspace-files

The read-only Workspace file service behind the window's right bar. The Server runs it in a
separate process of its own executable (`orchbee-server --workspace-files`) only after a Client
first sends `workbench.workspace-files`, so a Session whose windows never open the right bar starts
nothing. The process exits after ten idle minutes or when its standard input closes.

It reads one directory listing, one file from the working tree, the index or `HEAD`, or the change
list of `git status`, always beneath the Workspace directory the Server names. It writes nothing:
Git runs with optional locks off, so a status read never refreshes the index.

## Protocol

Standard input and output carry one JSON object per line. A request is
`{"id": 1, "root": "/abs/workspace", "operation": {...}}`, where `operation` is the
`workbench.workspace-files` operation; the answer is `{"id": 1, "outcome": {...}}`, a
`workbench.workspace-files` outcome. Requests are answered in order.

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
- Each Git command has a four-second deadline and bounded output; Git's own environment variables
  are cleared, terminal prompts are off and `core.fsmonitor` is disabled.
