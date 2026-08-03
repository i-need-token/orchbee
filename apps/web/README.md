# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Composition boundary

`apps/web` is the single runnable product UI for both a normal browser page and Electron's renderer.
Environment adapters may provide different transport and privileged capabilities, but they compose
the same Workbench, Workspace, Tab, Panel, and projected-terminal application. Desktop must not
maintain a parallel renderer UI. The current browser preview uses its in-memory adapter; the
Desktop runtime uses the Electron client-session, projection, and application-lifetime adapters in
this application. The URL marker `?orchbee-runtime=desktop` selects Desktop mode; an absent marker
selects Browser Preview, and invalid or duplicate values fail closed without falling back.

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
shell only through an environment adapter with accepted ownership and recovery contracts. The
current ordinary Browser page remains an in-memory preview and does not connect to Desktop Main;
the network Gateway that will make Browser pages peers of Electron remains a later slice.

For the complete local Desktop flow, run the Web server and Electron process in separate terminals:

```sh
pnpm web
pnpm desktop
```

Electron loads the same Web development URL and selects Desktop mode explicitly. `apps/desktop`
does not build, copy, or own a renderer bundle.
