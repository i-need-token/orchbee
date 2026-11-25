# Workbench Right Bar

| Field             | Value                                                                                                                                                           |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Draft; phase 1 (read-only files, changes and preview) is implemented on `feat/right-bar-v1`; editing, search and Git actions remain future                      |
| Owner             | `@orchbee/web` Workbench maintainers and `orchbee-server` maintainers                                                                                           |
| Current authority | This proposal, [`schemas/workbench/v1`](../../schemas/workbench/v1) for `workbench.workspace-files`, and `apps/orchbee-server/crates/workspace-files/README.md` |
| Exit condition    | Accept or revise the remaining phases, move maintained facts into the owning application guides, then delete this proposal                                      |

## Goal

A right bar beside the terminal that lets a person browse the active Workspace's files, see what Git
reports as changed, and preview a file or a change, without costing anything to people who never open
it and without slowing terminal work.

## Phase 1, as implemented

- **Protocol.** `workbench.workspace-files` (frames 83 and 84) is one read-only request with three
  operations: `list-directory`, `read-file` (worktree, index or `HEAD`) and `git-status`. It is gated
  by the `workspace-files` capability. Results are bounded (entry count, text size, result bytes) and
  carry closed failure reasons. The JSON Schema, the conformance corpus, the Rust records and the
  TypeScript schemas agree.
- **Server.** The Server resolves the Workspace's primary directory from its own snapshot, checks the
  Session and that the directory belongs to this machine's Worker, then asks a child process of its
  own executable (`--workspace-files`). The child starts on the first request only, keeps two lanes
  (files, Git status) so a slow `git status` never holds up a listing, answers each request within
  4 seconds, and exits after 10 idle minutes. Git runs with `GIT_*` variables removed, fsmonitor off,
  no pager and capped output. Paths are confined beneath the canonical root; symbolic links that lead
  outside are refused.
- **Web.** The right bar is a separate lazily loaded chunk. Only a window whose Server offers the
  capability shows the toggle (tab bar button, `Cmd+Option+B` / `Ctrl+Alt+B`). It has two views:
  a virtualized file tree (directories first, `.git` hidden, ignored files dimmed) and a change list
  grouped into conflicts, staged and unstaged. Selecting a file or change opens a preview floating
  over the terminal area: windowed text for a file, a folded line diff for a change.

## Deviations from the design discussion

- **Process.** The service is the Server's own binary re-executed, not a new artifact, so packaging
  and signing are unchanged. Under [native runtime topology](../platform/native-runtime-topology.md)
  the Worker should own filesystem and Git effects; moving the service behind the Worker is open.
- **No Monaco yet.** The preview uses a built-in viewer and a Myers line diff (with a whole-file
  fallback past 1000 edits) to avoid a large dependency in phase 1. Monaco remains the plan for
  editing and side-by-side diff.
- **Refresh.** No file watching or polling: views refresh on open, on switching view, on the refresh
  button and on window focus.
- **State.** Open state and width are not persisted. The bar is hidden in the mobile layout.
- **Preview.** One floating preview, not yet a tab in a unified tab bar.
- **Bundle.** The lazy chunk is about 17 KB JS and 7 KB CSS raw. The main bundle still grows by about
  22 KB raw (3.3 KB gzip), above the 10–15 KB target, mostly from protocol schemas, messages and
  shortcut descriptors that the current architecture loads eagerly.

## Later phases

1. Simple editing and saving through Monaco, loaded only when an editor opens.
2. Search across the Workspace (filename and content), reusing the unified search surface where it
   fits.
3. Git actions: stage, unstage, discard, commit, with explicit confirmation for destructive ones.
4. File watching with bounded, coalesced invalidation instead of focus refresh.
5. Persisted open state, width and expanded folders per Workspace.
