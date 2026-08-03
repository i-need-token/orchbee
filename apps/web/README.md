# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Composition boundary

`apps/web` is the single runnable product UI for both a normal browser page and Electron's renderer.
Environment adapters may provide different transport and privileged capabilities, but they compose
the same Workbench, Workspace, Tab, Panel, and projected-terminal application. Desktop must not
maintain a parallel renderer UI. The current browser preview uses its in-memory adapter; the
accepted Desktop integration replaces the temporary `apps/desktop/src/renderer` composition with
an Electron client-session adapter in this application.

## Local development preview

`pnpm web` renders the Workbench shell with its localized sidebar, standalone terminal context,
Project and Workspace navigation, and per-tab Panel workspace. The browser composition creates one
`ActionRuntime` and one in-memory `WorkbenchProjectWorkspaceRuntime`. It starts with only the
standalone Terminal context. A person can create an in-memory Project by entering a folder path;
the Project add control first preserves the common creation menu, then `Select Folder…` opens the
browser-only path entry in place of a native picker. The browser uses that path as the initial
Project Folder and terminal CWD without claiming that the path exists. `New Folder…` remains
visible but disabled because the composition performs no filesystem, Git, PTY, or durable-storage
effect.

The composition also owns locale and shortcut-platform setup. Native folder selection, Git
worktree management, PTY rendering, terminal search, and persistence will be composed into the
shell only after their ownership and recovery contracts are accepted.
