# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Composition boundary

`apps/web` is the single runnable product UI for both a normal browser page and Electron's renderer.
Environment adapters may provide different transport and privileged capabilities, but they compose
the same Workbench, Workspace, Tab, Panel, and projected-terminal application. Desktop must not
maintain a parallel renderer UI. The shared Workbench client runtime owns shared-document
coordination, projected-terminal presentation, search, actions, and its narrow client-session and
projection ports. It has no Electron, `MessagePort`, network, or application-lifetime dependency.

The Browser Preview uses its in-memory composition. Desktop supplies `MessagePort` adapters for the
shared client-session and projection ports, then layers its Electron-only application-lifetime
dialog beside the common application. The local Gateway adapter uses one authenticated control
WebSocket per page and one binary WebSocket per visible terminal attachment. Both adapters compose
the same `createWorkbenchClientRuntime`; neither implements another Workbench renderer. Generic
Workbench client payloads come from `@orchbee/workbench-client-protocol`; only Gateway
authentication/framing comes from `@orchbee/gateway-protocol`, and only Electron bootstrap and
application-lifetime payloads come from `@orchbee/desktop-client-protocol`.

The URL marker `?orchbee-runtime=desktop` selects Desktop mode,
`?orchbee-runtime=gateway` selects an authenticated Gateway Browser client, and an absent marker
selects Browser Preview. Gateway port, version, and lifetime capability must appear exactly once in
the fragment. Invalid, duplicate, or incomplete values fail closed without falling back.

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

The composition also owns locale and shortcut-platform setup. Native folder selection and Git
worktree management remain absent until an environment adapter owns their effects. An ordinary URL
without a runtime marker remains the in-memory Preview and does not connect to Desktop Main. A
Gateway URL printed by Desktop creates a fresh page session and Client View over the same canonical
Workbench document and terminal presentations as Electron and every other authenticated page.
Closing that page releases only its attachments; reopening the URL creates a fresh session and full
projection snapshots.

Gateway selection copy obtains bounded UTF-8 selection bytes from Terminal Host and commits them
with the Browser Clipboard API. Gateway links use a local Browser effect restricted to `http`,
`https`, and `mailto`; the page receives no Electron Main effect capability. Electron continues to
use its Main-owned clipboard and external-link effects.

For the complete local Desktop flow, run the Web server and Electron process in separate terminals:

```sh
pnpm web
pnpm desktop
```

Electron loads the same Web development URL and selects Desktop mode explicitly. `apps/desktop`
does not build, copy, or own a renderer bundle. When the local Gateway is enabled, Desktop prints
one `ORCHBEE_BROWSER_CLIENT_URL=<url>` line; open that complete fragment-bearing URL in one or more
Browser pages. Treat it as a process-lifetime secret.
