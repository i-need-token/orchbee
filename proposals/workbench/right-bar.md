# Workbench Right Bar

| Field             | Value                                                                                                                                                                                       |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Draft; phase 1, the float window, chunked versioned reads, change following with a platform watcher, and editing and saving are implemented on `feat/right-bar-v1`; the rest remains future |
| Owner             | `@orchbee/web` Workbench maintainers and `orchbee-server` maintainers                                                                                                                       |
| Current authority | This proposal, [`schemas/workbench/v1`](../../schemas/workbench/v1) for `workbench.workspace-files`, and `apps/orchbee-server/crates/workspace-files/README.md`                             |
| Exit condition    | Accept or revise the remaining phases, move maintained facts into the owning application guides, then delete this proposal                                                                  |

## Goal

A right bar beside the terminal that lets a person browse the active Workspace's files, see what Git
reports as changed, and preview a file or a change, without costing anything to people who never open
it and without slowing terminal work.

## Phase 1, as implemented

- **Protocol.** `workbench.workspace-files` (frames 83 and 84) is one request whose first three
  operations read: `list-directory`, `read-file` (worktree, index or `HEAD`) and `git-status`;
  `watch` and `write-file` follow below. It is gated by the `workspace-files` capability. Results are bounded (entry count, chunk size, result bytes) and
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
  the whole file, which saving checks.
- **Images.** PNG, JPEG, GIF, WebP and SVG files up to 10 MB show as pictures, with their pixel size;
  another binary file shows its extension and size.
- **Diff.** Monaco's diff editor: side by side where the window is at least 900 px wide, inline
  otherwise or on request; leading and trailing whitespace ignored by default; unchanged regions
  can be collapsed; previous and next change. A meta line shows added and removed lines, the number
  of changes and which two versions are compared.

## Following changes, as implemented

- **What is followed.** A watch follows only what shows: the open bar's Workspace directory and
  expanded folders, and the expanded float window's working-tree files. A closed bar or a collapsed
  float window follows nothing and reads again when it shows. Expanded folders are kept per
  Workspace while the window lives, so reopening the bar shows and follows them again.
- **Protocol.** `watch` is a fourth `workbench.workspace-files` operation, sent only as a State
  stream. Each frame carries two revisions, `files` (a shown folder's entries or an open file) and
  `repository` (anything in the working tree, or the index, `HEAD` and references), and the watch
  state. A new scope opens its stream before the old one closes.
- **Server.** The Server unions the watches of every connection per Workspace directory and hands the
  set to a watch lane, a second child process of its own executable (`--workspace-files-watch`)
  started on the first watch. The lane collects platform events over 100 ms and reports which
  revisions moved; Git's own objects, logs and lock files move neither. A lane that exits restarts
  with backoff while any watch remains, and each Workspace reads as `unavailable` meanwhile.
- **Web.** Reports are read one second after the last change, and at least every five seconds during
  a burst that never goes quiet. Nothing reads while the window is hidden; what changed reads when it
  shows again. A terminal Agent that stops working reads at once. The bar reads only the view that
  shows the change, and the float window only a tab that shows it. A change list cut short at 5000
  entries is not followed, and while changes are not followed the bar says once that Refresh is the
  way to see them and also reads again when the window comes to the front.
- **Platform watcher.** The lane watches through `notify` without its debouncer, since the lane
  already collects over 100 ms: one recursive watch per root on macOS and Windows, and on Linux one
  inotify watch per directory that skips `.git` and directories Git ignores, up to 65,536
  directories, past which the Workspace reads as `unavailable`. Git's ignore rules come from
  `ignore`, with tracked files never ignored, so build output moves no change list.

## Editing and saving, as implemented

- **What edits.** A working-tree file read whole with its SHA-256, up to 10 MB, is editable in its
  own tab and on the right side of a diff against the working tree; both edit the same text, as VS
  Code keeps one model per file. Larger files, files read without a hash, and the index or `HEAD`
  side stay read-only and say why when typed into. Undo, find and replace, and indentation are
  Monaco's own.
- **Keeping edits.** The text of each edited file lives outside the editor, per Workspace and path,
  so edits survive collapsing the window, switching tabs and switching Workspaces. It is released,
  edits and all, only when no tab shows the file, and closing such a tab asks first: Save, Don't
  save or Cancel. A tab with unsaved edits shows a dot where its close button sits; the first edit
  keeps a preview tab. Quitting the window does not yet ask.
- **Saving.** `Cmd+S` / `Ctrl+S` while the keyboard is in the float window, or the window's Save
  button, sends `write-file` with the whole text and the SHA-256 the file was read or last saved
  at. Outside the window the key stays the terminal's. The service writes only while the file
  still hashes to that value, through a temporary file renamed over it, keeping its permissions;
  where a rename would lose hard links, owner or group, or the directory takes no new file, it
  writes in place. A file that must not exist yet is created exclusively. A save never creates
  directories, never writes through a symbolic link that leads outside the Workspace or nowhere,
  and leaves a read-only file unwritten. The Server waits 20 seconds for a save, which the service
  never cuts short, and answers it only to a connection that may also start and end terminals.
- **Conflicts.** A read that started before the latest save does not land. When the file changed
  on disk under unsaved edits, or a save finds it changed, a notice above the editor offers
  Compare (a diff of the disk text against the edits), Overwrite (a save over the version just
  seen, or Save when the file is gone) and Discard my edits (read the disk again, which Undo can
  bring back). A file whose disk text matches the edits counts as saved. Failed saves say whether
  the file or folder refused writing, the folder is gone, the Server could not save, or no answer
  came, which leaves unknown whether it saved.

## Changing the repository, as implemented

- **Staging.** Each change row offers, on hover or on the change the keyboard is on, Open file
  (except for deletions), Discard (any unstaged change) and Stage or Unstage; Space
  stages or unstages the keyboard's change and Enter previews its diff. Group headers offer Stage
  All Changes and Discard All Changes for unstaged changes, Unstage All Changes for staged ones and
  Stage All Conflicts for conflicts. Staging every unstaged change leaves conflicts alone; staging
  a conflict marks it resolved, as `git add` does. Unstaging a rename unstages both of its paths.
- **Discarding.** Discard asks first, naming the file or the count and listing up to eight paths,
  and then puts tracked files back to what the index holds and moves untracked files to the system
  trash of the computer running the Server, where they can be restored. The question says which
  files go to the trash; while anything would be lost for good, Cancel is focused, and a discard of
  untracked files alone focuses Move to Trash instead. Files the trash did not take stay where they
  were, and nothing tracked is checked out then.
- **Committing.** The message box grows from one line to ten; `Cmd+Enter` / `Ctrl+Enter` or Commit
  commits what is staged, across the whole repository as Git does. With nothing staged and
  unstaged changes present, Commit asks whether to stage everything and commit, with that button
  focused. A menu beside Commit offers Stage all and commit and Amend last commit; an amend with an
  empty message keeps the last message. `↑` on the first line and `↓` on the last walk through up
  to 50 messages committed before, per Workspace, kept in memory only. A message typed into a
  Workspace stays with it while switching. A commit that lands empties the box unless something
  else was typed meanwhile.
- **Running.** One change runs at a time and the list reads the repository again after each,
  whatever happened. Git's own words for a refusal, such as a failing hook or a missing identity,
  show under the commit box, cut to 8 KiB with escape sequences removed, until dismissed or the
  next change; other failures say whether the folder is not a repository, Git timed out, the
  Server refused, or no answer came. Changes run on a lane and a Server thread of their own, one at
  a time across Workspaces, so file reads and the change list never wait behind a slow hook. A
  change has 60 seconds, after which its whole process group is stopped (on Windows, only Git
  itself); the Server answers it only to a connection that may also start and end terminals.

## Deviations from the design discussion

- **Process.** The service is the Server's own binary re-executed, not a new artifact, so packaging
  and signing are unchanged. Under [native runtime topology](../platform/native-runtime-topology.md)
  the Worker should own filesystem and Git effects; moving the service behind the Worker is open.
- **Saving.** Files are saved whole rather than as edits, up to 10 MB, with the version check done
  by SHA-256 of the content. There is no unsaved-edit recovery after the window closes, no prompt
  when the window itself closes with unsaved edits, and no auto save.
- **Encodings.** Text that is not UTF-8 shows as binary rather than read-only text.
- **Refresh.** Git status stops following at 5000 entries rather than the design's 10,000, because
  that is where its list is cut.
- **Git actions.** A running commit cannot be cancelled, a failed commit offers no "view in
  terminal", rows and group headers have no context menu, and the header and row buttons are reached
  by pointer only (the keyboard path is Space and Enter). Staging a conflict that still holds
  conflict markers does not warn. Discard All lists only the changes shown when the list was cut at
  5000. Git 2.25 or later is needed.
- **State.** Open state and width are not persisted. The bar is hidden in the mobile layout.
- **Convert to tab.** Moving float tabs into the terminal tab bar waits on the unified tab bar.
- **Bundle.** Against the branch point, the initial JavaScript and CSS grow by about 33 KB raw (6.4
  KB gzip), mostly protocol schemas, messages and shortcut descriptors that the current architecture
  loads eagerly; chunked reads added about 0.2 KB gzip more and change following about 0.8 KB gzip.
  Editing and saving added about 24 KB raw (5.2 KB gzip), nearly all of it message descriptors and
  both locales' strings, which the application loads eagerly with their translator descriptions; the
  right bar's descriptors alone are about 13 KB raw. Changing the repository added about 13 KB raw
  (2.4 KB gzip), again nearly all messages and the protocol's new operations. Moving untracked files
  to the trash added about 3.9 KB raw (0.7 KB gzip), all messages in both locales. Together this
  puts the right bar about 5.7 KB gzip over the design's 10 KB budget for the main bundle; loading
  the right bar's messages with its chunk, or dropping descriptions from the built catalog, would
  bring it back under. The bar and float window chunk is about 13.8 KB gzip; the editor pane is
  about 7.3 KB gzip; Monaco is about 800 KB gzip and loads only with the first open file.

## Later phases

1. Search across the Workspace (filename and content), reusing the unified search surface where it
   fits.
2. Persisted open state, width and expanded folders per Workspace across restarts.
