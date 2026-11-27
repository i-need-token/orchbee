# Git Process

Private, experimental Rust library for starting Git from Orchbee's native owners: the Git program a
command names, the repository paths a command may be given, and one run per command under a
process-tree owner with bounded output. The Worker's worktree discovery and mutation owners use it,
and `orchbee-files` takes it up under the
[File Bar backend proposal](../../proposals/platform/file-bar-backend.md#git). Each consumer keeps
its own environment policy, which inherited variables a command drops and sets and whether it trusts
a repository, and its own deadlines and error vocabulary.

## Program

- `GIT_PROGRAM` is the name process creation resolves: `git.exe` on Windows, `git` elsewhere.
- `read_program` names the Git for a command that starts no helper program: no hook, filter,
  credential helper or ssh. On Windows, when the first `git.exe` process creation would find, in
  the running executable's folder, the system folder, the Windows folder and then `PATH`, is the Git
  for Windows launcher (`<installation>\cmd\git.exe` or `<installation>\bin\git.exe`), it names the
  one `<installation>\<architecture>\bin\git.exe` that launcher would start (`mingw64`,
  `clangarm64` or `mingw32`) and spares one process start. Any other first match, and any doubt,
  keeps the name, which process creation resolves exactly as before. Elsewhere it is the name.
- A command that may start a helper program keeps `GIT_PROGRAM`, because the launcher sets the
  search path and variables those programs use.

## Paths

- `is_admissible_path` accepts an absolute path; on Windows only a drive path, verbatim or not, and
  never a network, device or relative path.
- `canonical_path` resolves a path on the filesystem to a UTF-8 `CanonicalPath` without the verbatim
  prefix. It touches the filesystem, so it runs on the consumer's Git owner, never while serialized
  input is admitted. On Windows a network location (`\\?\UNC\`) is refused. `as_native` is the
  system's spelling; `git_spelling` is the one Git prints and takes, with forward slashes on Windows,
  as in `-c safe.directory=<path>`.
- `checkout_spelling` is the spelling under which two checkout paths Git recorded name the same
  checkout: without a trailing separator, and on Windows also without ASCII case or separator style.

## Runs

`Run` starts one command its consumer built, with the program, arguments, working directory and
environment that consumer chose, reading the null device. A Job on Windows and a new process group
on Unix own its tree, as the
[retained command](../local-runtime/README.md#retained-commands) owns it.

- Both outputs are drained without blocking; between turns the run waits for output at most 10 ms.
- Standard output past `OutputBounds::stdout` ends the run as `TooLarge`. Standard error keeps its
  last `OutputBounds::stderr_tail` bytes, because Git writes its final error after any progress.
- The consumer's check runs before every turn and while the last output is drained; its error ends
  the run as `Stopped`. Cancellation and deadlines are the consumer's.
- Whatever ends a run early, the tree is settled before the run returns. `Unsettled` reports a
  process that was created and could not be proven gone; `Spawn` reports that none was created,
  with `NotFound` when the program does not exist.

## Verification

Run from the repository root:

```sh
cargo clippy -p orchbee-git-process --all-targets --all-features --locked -- -D warnings
cargo test -p orchbee-git-process --all-targets --all-features --locked
cargo test -p orchbee-worker --bins --all-features --locked worktree
```

The Worker's worktree discovery and mutation suites run real Git through this crate.
