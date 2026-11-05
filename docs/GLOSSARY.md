# Glossary

## Authority and use

Use this vocabulary for cross-workspace product concepts, code names, serialized contracts,
documentation, and localized copy. A feature owner defines its operations and narrower terms;
it must preserve these entity boundaries. [Architecture](ARCHITECTURE.md) owns dependency and
runtime boundaries; [Cross-client Interface Design](INTERFACE-DESIGN.md) owns their presentation.

The spellings below are normative names, not a claim that every existing identifier has already
been migrated. The owning source, schema, and application reference describe the currently
implemented surface. Keep a staged rename's concrete mapping and remaining consumers in its
issue; update those consumers together before claiming the rename complete. This standard does
not introduce commands, fields, aliases, types, or capabilities merely by naming a concept.

English is canonical. The Chinese column selects existing localized vocabulary; retained English
role names are intentional where the current Chinese copy uses them. Translate product prose
through its localization owner, never translate identifiers or user-supplied names. A short label
may omit a qualifier only when its surface makes the referent unambiguous.

## Product scopes

| English          | Chinese    | Meaning and distinction                                                                                                                                                                                                                                                                                            |
| ---------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Session          | 会话       | The selected Orchbee runtime scope served by a Server, containing Workbench state and coordinated execution. Clients can disconnect without ending it. A named Session can retain stored state while stopped; a running process or connection is not its definition. `Site` is the retired name.                   |
| Workbench        | Workbench  | The product organization and operation model containing Projects, Workspaces, Terminal Contexts, Tabs, and Panels, plus Client projections of that model. It is neither a Server process nor a terminal emulator. Shared state and Client-local view state have different owners.                                  |
| Project          | 项目       | A named organizational group of Workspaces. Membership can change without changing Workspace identity, directories, or terminals. A Project is not a Git repository, checkout, directory, or runtime Session. Repository tooling's workspace `project.json` is a separate engineering use of “project.”            |
| Workspace        | 工作区     | A stable unit of work with its own Terminal Context, optional Project membership, and optional primary Directory. Ungrouping or moving it does not create another Workspace. Closing it and removing its disk checkout are distinct effects.                                                                       |
| Directory        | 目录       | An admitted filesystem location associated with an execution Worker and identified independently of its path spelling. A Workspace may refer to it as its primary Directory; Project grouping is not filesystem ownership. A terminal's observed working directory is a separate fact.                             |
| Worktree         | 工作树     | A Git working tree on disk, with its own checkout and Git metadata relationship. A Workspace may use it. Closing the Workspace does not mean removing the Worktree or deleting its branch. Do not invent a Worktree identity from a Workspace ID.                                                                  |
| Terminal Context | 终端上下文 | The Workbench scope within a Workspace that owns an ordered set of Tabs. It is not a terminal process, selection, or connection.                                                                                                                                                                                   |
| Tab              | 标签页     | A member of a Terminal Context; a terminal Tab owns a Panel/Split tree. Its title, group, and display position do not define its identity. It is not an OS window or a terminal process.                                                                                                                           |
| Tab Group        | 标签页组   | An ordered grouping of Tabs within a Terminal Context. It is distinct from a Project's grouping of Workspaces.                                                                                                                                                                                                     |
| Panel            | 面板       | A stable leaf in a Tab's layout. It owns placement of a primary terminal presentation and any admitted child presentations. Moving a Panel, changing its displayed terminal, and replacing a terminal attachment are distinct operations.                                                                          |
| Pane             | 窗格       | A Client's rendered/attached surface for a Panel or an explicitly qualified plugin surface. A TUI `PaneKey` identifies a logical pane and `PaneRef` adds an attachment incarnation. These are not a second shared Panel identity. Use Panel for shared layout operations and Pane for the local surface lifecycle. |
| Window           | 窗口       | An outer application or host-terminal window, qualified by its owner when necessary. It is neither a Tab nor a Panel. `Window` is a product noun; `Windows` is an operating system.                                                                                                                                |
| Sidebar          | 边栏       | A Client's region beside the Tabs that lists Projects, Workspaces, and their Agents. Its toggle reads Hide Sidebar / Show Sidebar (隐藏边栏 / 显示边栏) in every Client; 侧栏 and 侧边栏 are not used. It is a Client projection, not a Workbench entity, and hiding it changes no Workbench state.                |

## Runtime roles

| English               | Chinese               | Meaning and distinction                                                                                                                                                                                                                                                                                       |
| --------------------- | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Machine               | 机器                  | A local execution destination or a saved remote connection profile. A saved Machine selects a target and Session; its label or enabled state does not prove that a Server is running. It is not a Worker or Terminal Host process.                                                                            |
| Server                | Server                | The process that owns the native Session's Workbench authority, admission, coordination, and publication. `server` commands operate this service in the selected Session. Do not rename the command family to `session`.                                                                                      |
| Worker                | Worker                | An admitted execution coordinator behind Server, routing terminal work to Terminal Host and owning its execution-side capabilities. Worker admission is a Worker protocol, not a Session or Client handshake.                                                                                                 |
| Terminal Host (Host)  | Terminal Host（Host） | The process owning PTY/process lifetime, terminal emulation, screen, and history. Use Host as shorthand only in a terminal-runtime context. An SSH hostname, an OS hosting a UI, and an outer terminal emulator are different uses and must be qualified.                                                     |
| Client                | 客户端                | A participant consuming an authority's public boundary. A GUI or TUI Client projects state and invokes operations; a headless CLI invocation is also a Client. Client-local selection and focus do not become shared Workbench authority.                                                                     |
| Client Connection     | 客户端连接            | One accepted native Client–Server transport connection and its generation-scoped registration. Its registration ends when that connection ends. One Workbench Client can own several lanes/connections; a connection generation is not a Session ID or Workbench Client ID.                                   |
| Workbench Client      | Workbench 客户端      | The TUI runtime that owns its projected Workbench state, views, notices, input dispatch, and communication lanes. It is distinct from the Server's per-connection handler and from the Session it observes. Use the qualified name for this runtime, not Terminal Session or Native Session.                  |
| Agent                 | 智能体                | An AI program/provider participant that performs work and exposes observations through an integration. It is neither an Orchbee Worker nor the terminal in which it runs.                                                                                                                                     |
| Agent Session         | 智能体会话            | An Agent's own tracked run/conversation and observation lifecycle. `AgentSession` remains a distinct domain term; provider references, process observations, and terminal placement do not turn it into an Orchbee Session. Qualify “session” wherever both could be meant.                                   |
| Terminal              | 终端                  | A runtime terminal execution identity and its PTY-backed state, exposed through Terminal Host and the coordinating protocols. Its identity is separate from its displayed Panel, title, Agent Session, and attachment. Use Terminal Runtime when distinguishing execution from the rendered terminal surface. |
| Terminal Presentation | 终端呈现              | A Workbench placement/lifecycle reference through which a terminal is displayed, including primary and child presentations. Presentation identity and runtime terminal identity are separate.                                                                                                                 |
| Viewer / Attachment   | 查看器 / 附着         | A viewing relationship and its physical attachment to a particular terminal incarnation. Use the owning protocol's `ViewerId` or `AttachmentId`; neither is a Session ID or permanent Panel identity. A controller grant is separate from permission to view.                                                 |
| Cohort                | cohort                | A matched set of executable and supporting artifacts admitted together. Its identity is distinct from Session identity, a release label, or an accepted connection generation.                                                                                                                                |

## Identifiers and external spellings

Name by meaning before applying language casing. Rust types use `UpperCamelCase`, modules and
values use `snake_case`; TypeScript types use `UpperCamelCase` and values use `camelCase`.
Crate/package paths use `kebab-case`. Serialized fields follow their owning schema, not a global
case conversion. The table gives canonical stems and established or accepted surface spellings;
an omitted surface is not permission to add it.

| Concept                              | Type / module spelling                                                                                        | Serialized or external spelling                                                                                                                              |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Session runtime identity             | `SessionId`, `session_id`                                                                                     | `sessionId`; replace product `siteId`, including schema definitions and references                                                                           |
| Session selection name               | `SessionName`, `session_name`                                                                                 | `--session`, `--remote-session`, `ORCHBEE_SESSION`; these select a name, not a runtime ID                                                                    |
| Session runtime namespace            | `SessionScope`, `session_scope`, `session_catalog`                                                            | Existing `ORCHBEE_RUNTIME_BASE` and `ORCHBEE_RUNTIME_SCOPE` remain runtime-location inputs with their owning precedence; a scope is not a name or runtime ID |
| Session records                      | `NamedSession*`, `RetainedSession*`, `SessionStore`                                                           | Preserve the Session meaning; stored records do not prove a live Server                                                                                      |
| Workbench                            | `Workbench*`, `workbench`                                                                                     | `workbench.*` operation namespace; `orchbee.workbench.*` documents retain their owner                                                                        |
| Project                              | `WorkbenchProjectId`, `project_id` / `projectId`                                                              | `projectId`, `project`, `--project` where declared                                                                                                           |
| Workspace                            | `WorkbenchWorkspaceId`, `workspace_id` / `workspaceId`                                                        | `workspaceId`, `workspace`, `--workspace` where declared                                                                                                     |
| Directory                            | `WorkbenchDirectoryId`, `directory_id` / `directoryId`                                                        | `directoryId`; `cwd` is a working-directory value, not this identity                                                                                         |
| Worktree                             | `Worktree*`, `worktree`                                                                                       | `worktree` command/operation spelling; keep the owning path/source selectors                                                                                 |
| Terminal Context                     | `WorkbenchTerminalContextId`, `terminal_context_id`                                                           | `terminalContextId`                                                                                                                                          |
| Tab / Tab Group                      | `WorkbenchTabId`, `WorkbenchTabGroupId`, `tab` / `tab_group`                                                  | `tabId`, `groupId`; `tab` command family                                                                                                                     |
| Panel / Pane                         | `WorkbenchPanelId`, `panel`; `PaneKey`, `PaneRef`, `pane`                                                     | `panelId`, `panel`, `--panel` where declared; existing plugin `focusedPaneId` belongs to its plugin context, not a new shared entity                         |
| Window                               | `Window*`, `window`                                                                                           | Use the owning window adapter's contract; no inferred Session or Panel identifier                                                                            |
| Machine                              | `Machine*`, `machine`                                                                                         | Owning catalog `id`; `machine`, `--machine` where declared                                                                                                   |
| Server                               | `Server*`, `server`                                                                                           | `server` command family and `orchbee.server.*` machine documents                                                                                             |
| Worker                               | `WorkerId`, `worker_id`, `local_worker_service`                                                               | `workerId`, `ORCHBEE_WORKER_ID`; local Worker admission magic `OBLW`, version `2`                                                                            |
| Terminal Host                        | `TerminalHost*`, `terminal_host`                                                                              | Existing `hostEpoch`, `hostId`, and terminal protocol identities keep their distinct scopes                                                                  |
| Client Connection                    | `ClientConnection*`, `client_connection_registry`                                                             | OBLC `ClientAccepted { generation }` means Client Connection generation; no new `clientConnectionId` wire field                                              |
| Workbench Client                     | `WorkbenchClient`, `WorkbenchClientState`, `WorkbenchClientView`, `WorkbenchClientNotice`, `workbench_client` | Existing `WorkbenchClientId`, `clientId`, and `--client` identify the Client view/control participant, not an OBLC connection generation                     |
| Agent / Agent Session                | `Agent*`, `AgentSession*`, `agent`, `agent_session`                                                           | Keep Agent-schema `sessionId` and existing provider/session selectors; identical field spelling does not unify Agent and Orchbee identity domains            |
| Terminal / Presentation / Attachment | `TerminalId`, `TerminalPresentationId`, `AttachmentId`, `terminal`                                            | `terminalId`, `presentationId`, `attachmentId`, `terminal`, `--terminal`, `ORCHBEE_TERMINAL_ID` where declared                                               |
| Cohort                               | `Cohort`, `CohortArtifact`, `CohortError`, `cohort`                                                           | Existing cohort fields retain their artifact-admission meaning                                                                                               |

The five service document identifiers are `orchbee.server.start`, `orchbee.server.stop`,
`orchbee.server.diagnose`, `orchbee.server.configuration`, and `orchbee.server.agent-manifests`.
Their producer is the `server` command family; replacing `orchbee.site.*` with
`orchbee.session.*` would misname that boundary. Document versions follow
[Architecture](ARCHITECTURE.md#data-boundaries), independently of terminology.

CLI grammar remains owned under [CLI Command Surface](cli/command-surface.md). A code rename alone
does not rename a command, flag, or environment variable. The accepted Session selector is
`ORCHBEE_SESSION`, with no `ORCHBEE_SITE` fallback or alias. The internal Server and Worker
entrypoints use `--runtime-directory <absolute-runtime-directory>`, replacing `--local-site`
without an alias. Its value is an absolute runtime directory; Session names, scopes, and runtime
IDs remain distinct concepts. Other runtime inputs retain their existing contracts. Existing GUI
renderer coordination, browser authentication, and interaction lifetimes must be classified at
their own boundary, not mechanically treated as native Client Connections because an identifier
contains `Session`.

## Retired names and platform boundaries

| Disallowed use                                                              | Canonical meaning                                                                                                                                                |
| --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Product `Site`, `site`, `site_id`, `siteId`, “站点”                         | Session and its corresponding spelling; ordinary “website,” “call site,” and third-party names are not product aliases                                           |
| `ClientSession*` for native Client–Server admission                         | `ClientConnection*`; simplify `ClientSessionConnection` to `ClientConnection`, not `ClientConnectionConnection`                                                  |
| Server `WorkbenchClientSession`                                             | `WorkbenchClientConnection`; this is not the TUI `WorkbenchClient`                                                                                               |
| `terminal_session`, `TerminalSession`, `NativeSession*` for the TUI runtime | `workbench_client`, `WorkbenchClient*`; views may retain distinct state/UI module ownership without another Session concept                                      |
| `local-site-protocol`, `LocalSiteMessage`, `OBLS` for Worker admission      | `local-worker-protocol` / `orchbee-local-worker-protocol`, `LocalWorkerMessage`, `OBLW`                                                                          |
| `local-site-runtime` / `orchbee-local-site-runtime`                         | `local-runtime` / `orchbee-local-runtime`; the library owns admitted shared local-runtime capabilities, not a second Session authority                           |
| `Windows*` / `Native*` prefixes on product types                            | Name the semantic role: `SessionName`, `RetainedSessionRecord`, `UserSettings`, `NotificationSettings`, `CohortArtifact`; do not keep old-name re-export aliases |

Platform names belong in OS/capability adapters, as governed by
[Rust](RUST.md#platform-specific-code). A platform-specific implementation can live under
`session_store/windows.rs` while exposing `SessionStore`; naming it neutrally does not assert
that every platform implements it. Actual OS APIs and adapter-owned representations retain their
accurate platform names. Do not strip `Window` from the product concept or rename an upstream API.

Reserve unqualified Session for the Orchbee runtime. Retain Agent Session with its qualifier.
For a temporary reader, control loop, connection, projection, or UI lifecycle, use that role rather
than introducing another Session synonym. Resolve collisions by naming the narrower role or using
module qualification; do not merge distinct state and UI projections solely to reuse a name.

## Change and verification

For a terminology change, trace the identity and lifecycle through producer, consumer, schema,
CLI/document projection, and both locale catalogs. Review neighboring concepts and explicit
exceptions before applying a family-wide replacement. Change serialized contracts and their
bindings/fixtures together, and retain the owning identity widths, authority, and lifecycle unless
the task separately authorizes a behavior change.

Keep task-specific inventories in the issue. Maintain this glossary when an accepted concept or
boundary changes; link to owning references rather than copying their implementation inventory.
Review the English and Chinese terms and their source-backed mapping for meaning, then run the
applicable gate under [Verification](VERIFICATION.md#merge-gate).
