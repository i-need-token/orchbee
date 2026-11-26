# Workbench Right Bar

| Field             | Value                                                                                                                                                           |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Draft; phase 1, the float window with Monaco preview and diff, and chunked, versioned reads are implemented on `feat/right-bar-v1`; the rest remains future     |
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
  by the `workspace-files` capability. Results are bounded (entry count, chunk size, result bytes) and
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
  grouped into conflicts, staged and unstaged.

## Float window, as implemented

- **Placement.** Files and changes open in a float window over the Tab content area; terminals keep
  their size. Each Workspace keeps its own window, and the window outlives closing the bar.
- **Tabs.** The window has its own tab strip. A single click opens a preview tab (italic) that the
  next preview replaces in place; a double click, or opening an already open file for keeps, keeps
  it. New tabs open after the active one; closing the active tab hands over to its right neighbor,
  or its left one at the end. The staged and unstaged change of one file are separate tabs.
- **Collapse.** Esc, or choosing another terminal Tab, collapses the window to a pill naming how many
  files it holds; the pill brings it back. The close button closes the window and every file in it.
  Collapsing returns the keyboard to the terminal.
- **Editor.** Monaco loads in its own chunk the first time the window shows a file, with only the
  editor features the window uses, no language-service workers and languages registered lazily.
  Its theme is derived from the current named theme's system tokens and follows theme changes; the
  icon font is the application's own codicon font.
- **Large files.** A read names an offset and returns at most 256 KiB; text chunks end on a whole
  character. The pane joins the chunks and starts over, up to three times, when the file's size or
  modification time changes between them. Files above 1 MB open as plain text; above 50 MB, or
  above 10 MB for an index or `HEAD` side, the pane names the size and the limit instead. A
  working-tree file's first chunk carries its modification time and, up to 10 MB, the SHA-256 of
  the whole file, which saving will check.
- **Images.** PNG, JPEG, GIF, WebP and SVG files up to 10 MB show as pictures, with their pixel size;
  another binary file shows its extension and size.
- **Diff.** Monaco's diff editor: side by side where the window is at least 900 px wide, inline
  otherwise or on request; leading and trailing whitespace ignored by default; unchanged regions
  can be collapsed; previous and next change. A meta line shows added and removed lines, the number
  of changes and which two versions are compared.

## Deviations from the design discussion

- **Process.** The service is the Server's own binary re-executed, not a new artifact, so packaging
  and signing are unchanged. Under [native runtime topology](../platform/native-runtime-topology.md)
  the Worker should own filesystem and Git effects; moving the service behind the Worker is open.
- **Read-only editor.** The editor and the diff's right side are read-only until saving with a
  version precondition lands.
- **Encodings.** Text that is not UTF-8 shows as binary rather than read-only text.
- **Refresh.** No file watching or polling: views refresh on open, on switching view, on the refresh
  button and on window focus.
- **State.** Open state and width are not persisted. The bar is hidden in the mobile layout.
- **Convert to tab.** Moving float tabs into the terminal tab bar waits on the unified tab bar.
- **Bundle.** Against the branch point, the initial JavaScript and CSS grow by about 33 KB raw
  (6.4 KB gzip), mostly protocol schemas, messages and shortcut descriptors that the current
  architecture loads eagerly; chunked reads added about 0.2 KB gzip more. The bar and float window
  chunk is about 6.5 KB gzip; the editor pane is about 6 KB gzip; Monaco is about 800 KB gzip and
  loads only with the first open file.

## Later phases

1. Simple editing and saving, with a version precondition and a conflict notice.
2. Search across the Workspace (filename and content), reusing the unified search surface where it
   fits.
3. Git actions: stage, unstage, discard, commit, with explicit confirmation for destructive ones.
4. File watching with bounded, coalesced invalidation instead of focus refresh.
5. Persisted open state, width and expanded folders per Workspace.
