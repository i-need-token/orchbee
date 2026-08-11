# @orchbee/web

Browser composition entry for the Orchbee Workbench.

## Composition boundary

`apps/web` is the single runnable product UI for both a normal browser page and Electron's renderer. Environment adapters may provide different transport and privileged capabilities, but they compose the same Workbench, Workspace, Tab, Panel, and projected-terminal application. Desktop must not maintain a parallel renderer UI. The shared Workbench client runtime owns shared-document coordination, projected-terminal presentation, search, actions, and its narrow client-session and projection ports. It has no Electron, `MessagePort`, network, or application-lifetime dependency.

The Browser Preview uses its in-memory composition. Desktop supplies `MessagePort` adapters for the shared client-session and projection ports, then layers its Electron-only application-lifetime and Browser-access dialogs beside the common application. Its capability-scoped local projection transport declares `local-host`, allowing OBPB format-`0.0.1` render buffers to become direct typed views after structural validation. The Gateway adapter uses one authenticated control WebSocket per page and one binary WebSocket per visible terminal attachment and declares `external`, preserving the full per-cell protocol scan. Both adapters compose the same `createWorkbenchClientRuntime`; neither implements another Workbench renderer. The shared projected-terminal surface provides the compact terminal inset, balances leftover grid pixels after every container resize, and mirrors the Host-projected default background across the complete Panel viewport so the canvas never exposes a mismatched application surface. A connection notice overlays that surface without consuming terminal-grid space or moving the final row outside the viewport. Generic Workbench client payloads come from `@orchbee/workbench-client-protocol`; only Gateway authentication/framing comes from `@orchbee/gateway-protocol`, and only Electron bootstrap, application-lifetime, and Browser-access commands come from `@orchbee/desktop-client-protocol`.

The packaged `orchbee:` origin and Electron User-Agent token select Desktop mode. The User-Agent is adapter discovery for loopback development, never authentication; Desktop authority still arrives only through the validated sandboxed bootstrap. The explicit `?orchbee-runtime=gateway` marker selects an authenticated process-lifetime Gateway client on loopback or an explicitly enabled trusted LAN. An ordinary HTTP(S) browser origin selects Browser Preview. Gateway port, version, and lifetime capability must appear exactly once in the fragment. Invalid, duplicate, or incomplete values fail closed without falling back.

## Navigation

All three compositions use one TanStack Browser Router and the same path contract. Project, Workspace, and active Tab identity appear in the address. Structurally incomplete routes such as `/projects` or `/projects/$projectId/workspaces/$workspaceId` replace themselves with the first ordered available descendants; explicit stale identifiers retain the address and display the localized unavailable state. In-app navigation pushes history, while canonicalization and reconciliation replace it, so Back and Forward reproduce the displayed Client View without publishing shared-document changes. See [`docs/navigation-routing.md`](docs/navigation-routing.md).

## Local development preview

`pnpm web` renders the Workbench shell with its localized sidebar, standalone terminal context, Project and Workspace navigation, and per-tab Panel workspace. The browser composition creates one `ActionRuntime` and one in-memory `WorkbenchProjectWorkspaceRuntime`. It starts with only the standalone Terminal context. A person can create an in-memory Project by entering a folder path; the Project add control first preserves the common creation menu, then `Select Folder…` opens the browser-only path entry in place of a native picker. The browser uses that path as the initial Project Folder and terminal CWD without claiming that the path exists. `New Folder…` remains visible but disabled because the composition performs no filesystem, Git, PTY, or durable-storage effect.

The composition also owns locale and shortcut-platform setup. Native folder selection and Git worktree management remain absent until an environment adapter owns their effects. An ordinary HTTP(S) URL without the Gateway marker remains the in-memory Preview and does not connect to Desktop Main. A capability-bearing URL opened or copied by Desktop Main creates a fresh page session and Client View over the same canonical Workbench document and terminal presentations as Electron and every other authenticated page. Closing that page releases only its attachments; reopening the URL creates a fresh session and full projection snapshots.

Gateway selection copy obtains bounded UTF-8 selection bytes from Terminal Host and commits them with the Browser Clipboard API. Gateway links use a local Browser effect restricted to `http`, `https`, and `mailto`; the page receives no Electron Main effect capability. Electron continues to use its Main-owned clipboard and external-link effects.

Browser-owned request and presentation identifiers use `crypto.getRandomValues`, which remains available on the explicitly accepted HTTP LAN-preview origin. Runtime code does not depend on the secure-context-only `crypto.randomUUID` API.

For the complete local Desktop flow, run:

```sh
pnpm desktop
```

Electron loads the Web development URL and is detected from its User-Agent. Packaged Electron serves this application's production build from the standard secure `orchbee://app` origin; `apps/desktop` owns packaging and resource delivery but no second renderer implementation. In Desktop, open **Settings → Browser access**. The default action starts loopback-only access and opens the system Browser. Trusted-LAN preview must be enabled explicitly and always displays that HTTP is unencrypted and possession of the link grants full terminal control. The renderer receives sanitized Origins only; Desktop Main owns the secret-link copy and open effects.
