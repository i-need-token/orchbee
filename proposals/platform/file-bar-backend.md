# File Bar Backend

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Proposed. On 2026-11-27 the owner fixed the File Bar, an on-demand Rust process started by the Worker with its own Server connection, bundled ripgrep, the git CLI, the root rule and the release order, and asked for implementation to proceed on the recommended defaults. The [reserved decisions](#reserved-decisions) need acceptance before the release that depends on them; the [release gate](#release-gate) keeps every batch out of release builds until then |
| Owner             | `orchbee-files`, `orchbee-server` and `orchbee-worker` maintainers; the [File Bar frontend](../workbench/file-bar.md) owner for the wire shapes                                                                                                                                                                                                                                                                                                                           |
| Current authority | None. [`native-runtime-topology.md`](native-runtime-topology.md) excludes a generic Files or Git capability until amended under [Document changes](#document-changes)                                                                                                                                                                                                                                                                                                     |
| Exit condition    | Implement each batch, move its facts into the owners listed under [Document changes](#document-changes), then delete this proposal                                                                                                                                                                                                                                                                                                                                        |

## Process

| Fact        | Value                                                                                                                                                                                                                                                                                                                                                                                     |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Executable  | `apps/orchbee-files`, Rust, cohort name `<stem>-files`: `orchbee-files[.exe]` in release, `ob-dev-<16 hex>-files` in development, `ob-test-…-files` in tests                                                                                                                                                                                                                              |
| Placement   | Beside `orchbee-worker` in the version directory, locally and in a remote machine's store                                                                                                                                                                                                                                                                                                 |
| Crates      | `crates/files-protocol`: OBFB records, DTOs, validation and the private OBFL codec, for `orchbee-server`, `orchbee-files` and later the TUI. `crates/git-process`: git program resolution, admissible paths, process-tree ownership and bounded output, extracted from `apps/orchbee-worker/src/git_repository.rs`; the Worker and `orchbee-files` each keep their own environment policy |
| Schema      | `schemas/files/v1/`: JSON Schema and conformance corpora; the TypeScript binding lives in `apps/web` beside the OBWP codec                                                                                                                                                                                                                                                                |
| Launcher    | The Worker, on the Server's request                                                                                                                                                                                                                                                                                                                                                       |
| Cardinality | One per Worker generation, serving every Workspace of its Session on that machine                                                                                                                                                                                                                                                                                                         |
| Roles       | Diagnostic role `files`; runtime socket role `files`                                                                                                                                                                                                                                                                                                                                      |
| Children    | `git` and `rg`: on macOS and Linux through `CommandChild::spawn_with_streams` (own pipes, new process group, no other descriptor); on Windows in a `process-wrap` Job                                                                                                                                                                                                                     |

### Launch

1. A Client's first File Bar request or subscription for the machine reaches the Server. A Client
   that never shows the File Bar sends nothing, so no process, watch or git runs.
2. The Server binds a one-shot endpoint `<endpoint id>-files` in its runtime directory and asks the
   Worker on the OBCN high lane to start `orchbee-files` with it and a fresh 256-bit token.
3. The Worker resolves and verifies its sibling and starts it with `--local-endpoint <endpoint>`,
   the token on stdin and the terminal launch environment (the login PATH on macOS): in a
   kill-on-close Job on Windows, through the retained spawn on macOS and Linux. It answers at once
   with the PID and start time or a typed failure, and is the child's only reaper.
4. The child connects; each side checks the peer is the same user before reading bytes. The child
   sends the OBFL hello: version, source commit, environment, protocol fingerprint and token.
5. The Server compares the token in constant time, admits one connection per token and closes the
   endpoint; the child writes `ready\n`. A build mismatch is `unavailable{build-mismatch}`: the link
   has no compatibility window.
6. Without a ready link in 10 s the Server has the Worker terminate that PID and counts a crash.

### Lifecycle

| Situation      | Rule                                                                                                                                         |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| No subscriber  | Watches and automatic refreshes stop at once; with no operation, prompt or transfer either, the process exits after 10 minutes               |
| Collapsed bar  | The SCM subscription stays, so the count badge continues; it exists only after the File Bar was visible during the Client connection         |
| Server gone    | EOF on the link: children are terminated and the process exits 1                                                                             |
| Session drain  | New requests are refused, children terminated, exit 0                                                                                        |
| Crash          | The next demand restarts it after 0, 1, 5 or 30 s; 5 crashes in 10 minutes give `unavailable{crashed-repeatedly}` until the user restarts it |
| Successor pair | A replacement Server and Worker start a fresh process on demand                                                                              |

## Server link

- OBFL is private to one build: a 12-byte header (`OBFL`, u16 version 1, u16 kind, u32 length
  ≤ 256 KiB) on the admitted endpoint.
- It carries Client requests in an envelope of Server call ID, Client connection ID, Workspace and
  grant class; replies; subscription notices; cancels; scope updates; connection loss; Client focus;
  `agent-settled {workspace}`; and drain.
- A scope update precedes every request authorized under it. Connection loss releases that
  connection's grants, transfers, pickers, prompts and subscriptions.
- The Server parses request JSON only for the operation, Workspace and grant class, and never parses
  Put or Chunk payloads.
- `orchbee-files` keeps a 4 MiB outbound queue; a subscription that would overflow it collapses to
  one `rescan` (watch) or its latest summary (SCM). Replies are bounded by the Client's window.

## Client protocol

### Capabilities

| Capability      | Batch | Adds                                                                                                                                          |
| --------------- | ----- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `files`         | ①     | OBFB records; browsing, stat, reads, opened-file grants, watching, SCM summary and pages, diff content, Git ① operations, the relay, outcomes |
| `files-edit`    | ②     | Writes, file operations, delete, trash, undo, import, export, picker, clipboard sources                                                       |
| `files-search`  | ③     | Text search, file search, Quick Open                                                                                                          |
| `files-history` | ④     | Local History, Git ④ operations                                                                                                               |

- Names go in `schemas/connection/v1/connections.json` and
  `crates/connection-handshake/src/registry.rs`; `files` requires `stream-outcome-unknown`.
- A Server without `files` leaves that machine without a File Bar.
- After a release, additions arrive under new capability names and released records never change.
  An unknown operation closes with StreamClose Unsupported; a Client treats an unknown error code
  as `failed` and an unknown notice as absent.

### Records

OBFB has a 12-byte header (`OBFB`, u16 version 1, u16 kind, u32 length). Multiplex kinds 64 and 65
carry it only for a decoder built with `MultiplexDecoder::admitting` on a carrier that negotiated
`files`; those cases live in `workbench-multiplex-files-conformance-v1.json`.

| Kind      | Direction and carrier                             | Body                                                                                                                  | Bound            |
| --------- | ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | ---------------- |
| 1 Request | Client to Server, StreamOpen                      | JSON with `op`                                                                                                        | 16 KiB           |
| 2 Result  | Server to Client, StreamFrame Result              | JSON: the result, or `error`                                                                                          | 128 KiB          |
| 3 Notice  | Server to Client, StreamFrame Snapshot and Update | JSON                                                                                                                  | 128 KiB          |
| 4 Put     | Client to Server, StreamOpen in Request mode      | u64 transfer, u64 offset, u8 flags (`last` 1, `commit` 2), bytes                                                      | 128 KiB of bytes |
| 5 Chunk   | Server to Client, StreamFrame Result              | u64 transfer, u64 offset, u8 flags (`last` 1, `changed` 2), u8 line ending (0 not final, 1 none, 2 LF, 3 CRLF), bytes | 128 KiB of bytes |

### Streams and pacing

- Request mode: one Request or Put, one Result or Chunk, then StreamClose Complete.
- Events mode: `fs.subscribe`, `scm.subscribe` and `relay.subscribe` send a Snapshot, then Updates.
  After a Gap the Client re-lists its loaded folders (`fs`) or takes the next Snapshot (`scm`).
- Each transfer and search runs on its own lane; LaneClose cancels it and drops its staging.
- Bulk data is only pulled. The Client chooses transfer IDs (nonzero u64, unique in the
  connection), so it can pipeline the first chunk requests or Puts behind the opening request.
  Text reads are served in offset order, byte reads in any order.
- The Server's writer sends a File Bar record only when no terminal or Workbench record waits, but
  at least one every 100 ms, so neither starves the other.

| Bound                           | Value                                                      |
| ------------------------------- | ---------------------------------------------------------- |
| File Bar streams per connection | ≤ 24, of which ≤ 16 carry Put or Chunk                     |
| Retained multiplex budget       | ≤ 8 MiB of the 32 MiB                                      |
| Client data window              | ≤ 2 MiB in flight locally, ≤ 256 KiB to a remote machine   |
| Chunk length                    | Client-chosen 16–128 KiB: 128 KiB locally, 32 KiB remotely |
| Listing and status pages        | ≤ 2,000 entries and ≤ 128 KiB                              |
| Watch Update                    | ≤ 500 events and ≤ 128 KiB                                 |
| Paths per request               | ≤ 256; ≤ 1,000 for `git.check-ignore`                      |

### Identity, cancellation and errors

- A mutation carries `requestId`, 32 lowercase hexadecimal digits, nonzero. `orchbee-files` keeps the
  last 256 outcomes per Workspace for 10 minutes; a repeated ID returns the recorded outcome without
  running again, and `request.outcome` answers `completed` with the result, `running` or `unknown`.
  A crash loses the table: mutations in flight close with StreamClose OutcomeUnknown.
- StreamClose Cancelled or LaneClose cancels a call. git and rg are terminated as a tree: SIGTERM to
  the group, SIGKILL after 2 s; `TerminateJobObject` on Windows. A killed git can leave
  `index.lock`; the next operation then fails with `gitErrorCode: repository-is-locked`.
- An error is `{code, osError?, gitErrorCode?, detail?}`. Codes: `denied`, `outside-scope`,
  `link-target-outside`, `unsupported-location`, `not-found`, `exists`, `is-directory`,
  `not-directory`, `invalid-name`, `unrepresentable-name`, `permission-denied`, `read-only`,
  `disk-full`, `busy` (file in use), `too-large`, `content-newer`, `write-incomplete`,
  `trash-unavailable`, `trash-failed`, `undo-expired`, `stale-revision`, `timeout`, `cancelled`,
  `unavailable` (with a `reason`), `unsafe-repository`, `git-failed`, `invalid-regex`,
  `invalid-request` and the fallback `failed`. `remote-disconnected` is the Client's own, raised
  when the machine link drops.
- `gitErrorCode` follows VS Code's `GitErrorCodes`, classified in Rust. `detail` is content, not
  text Orchbee authored: git's stderr or rg's pattern message, at most 4 KiB, shown verbatim as
  VS Code shows it. No presentation text crosses the wire; the frontend localizes codes.

### Operations

Every request names `workspaceId`. Paths are absolute in the machine's native form, UTF-8,
≤ 4,096 bytes. An entry is `{name, type, symlink, size, mtimeMs, readonly, unrepresentable?}`, where
`type` is `file`, `directory` or `unknown` and describes a link's target.

`files`, batch ①:

| Op                                        | Request                                                                                                                  | Result                                                                                                                                                                                                                                             |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `fs.subscribe` (Events)                   | —                                                                                                                        | Snapshot: root `{path, state}` (`present`, `missing`, `none`), `os`, `separator`, `caseSensitive`. Updates: `changes` `[{type, path, directory}]` (`added`, `changed`, `deleted`), `rescan` paths, `root`, `watch` (`full`, `degraded`, `stopped`) |
| `fs.list`                                 | `path`, `cursor?`, `singleChild?`                                                                                        | `entries`, `cursor?`, `descendants?` `[{path, entries}]`; loads `path` for watch delivery                                                                                                                                                          |
| `fs.release`                              | `paths`                                                                                                                  | Unloads them                                                                                                                                                                                                                                       |
| `fs.stat`                                 | `paths`; with `source: quick-open`, one path, which may lie outside every member                                         | `items` `[{path, entry or error}]`                                                                                                                                                                                                                 |
| `fs.grant`                                | `path`, `source`: `tree`, `terminal-link`, `restored`, `os-open`, `save-as`, `diff`, `quick-open`                        | `entry`                                                                                                                                                                                                                                            |
| `fs.ungrant`                              | `paths`                                                                                                                  | —                                                                                                                                                                                                                                                  |
| `fs.read`                                 | `transfer`; `path`, `export` with `index`, or `history`; `mode` (`text`, `bytes`); `encoding?`; `forceText?`; `limit`    | `size`, `mtimeMs`, `encoding`, `bom`, `binary`                                                                                                                                                                                                     |
| `fs.read-chunk`                           | `transfer`, `offset`, `length`                                                                                           | Chunk                                                                                                                                                                                                                                              |
| `fs.guess-encoding`                       | `path`                                                                                                                   | `encoding` or null                                                                                                                                                                                                                                 |
| `transfer.close`                          | `transfer`                                                                                                               | —                                                                                                                                                                                                                                                  |
| `scm.subscribe` (Events)                  | —                                                                                                                        | Snapshot and Updates: the whole summary below                                                                                                                                                                                                      |
| `git.status-page`                         | `repository`, `revision`, `cursor?`                                                                                      | `entries` `[{x, y, path, original?, submodule}]` in porcelain order, `cursor?`; `stale-revision` after a newer status                                                                                                                              |
| `git.show`                                | `transfer`, `repository`, `ref` (`HEAD`, `index` or a commit), `path`, `mode`, `encoding?`, `limit`                      | As `fs.read`                                                                                                                                                                                                                                       |
| `git.check-ignore`                        | `repository`, `paths`                                                                                                    | `ignored` paths                                                                                                                                                                                                                                    |
| `git.refs`                                | `repository`, `cursor?`                                                                                                  | `refs` `[{name, type, commit, upstream?}]`, `remotes` `[{name, fetchUrl, pushUrl}]`, `cursor?`                                                                                                                                                     |
| `git.user-config`                         | `repository`                                                                                                             | `name?`, `email?`                                                                                                                                                                                                                                  |
| `git.output`                              | `cursor?`                                                                                                                | `text`, `cursor`, `dropped`                                                                                                                                                                                                                        |
| `git.stage`, `git.unstage`, `git.discard` | `requestId`, `repository`, `paths`                                                                                       | —                                                                                                                                                                                                                                                  |
| `git.stage-content`                       | `requestId`, `transfer`, `repository`, `path`, `size`; the index content follows as Puts, the last flagged `commit`      | —                                                                                                                                                                                                                                                  |
| `git.clean`                               | `requestId`, `repository`, `paths`, `mode` (`trash`, `permanent`)                                                        | `items` `[{path, error?}]`                                                                                                                                                                                                                         |
| `git.commit`                              | `requestId`, `repository`, `message` or `editor: true`, `amend`, `signoff`, `all` (`none`, `tracked`, `all`), `noVerify` | `commit`                                                                                                                                                                                                                                           |
| `git.fetch`                               | `requestId`, `repository`, `remote?` or `all`, `prune`, `background`                                                     | —                                                                                                                                                                                                                                                  |
| `git.pull`, `git.sync`                    | `requestId`, `repository`, `rebase`                                                                                      | —                                                                                                                                                                                                                                                  |
| `git.push`                                | `requestId`, `repository`, `remote?`, `branch?`, `setUpstream` (publish), `force` (`none`, `lease`, `force`), `tags`     | —                                                                                                                                                                                                                                                  |
| `git.init`, `git.mark-safe`               | `requestId`, `path`                                                                                                      | `repository` for `git.init`                                                                                                                                                                                                                        |
| `repository.consent`, `repository.close`  | `repository`                                                                                                             | —                                                                                                                                                                                                                                                  |
| `relay.subscribe` (Events)                | —                                                                                                                        | Updates: `prompt` `{id, kind, text, repository, path?}` (`username`, `password`, `passphrase`, `host-key`, `editor`), `closed` ID                                                                                                                  |
| `relay.answer`                            | `prompt`, `value?` or `cancelled`                                                                                        | —                                                                                                                                                                                                                                                  |
| `client.focus`                            | `focused`                                                                                                                | —                                                                                                                                                                                                                                                  |
| `request.outcome`                         | `requestId`                                                                                                              | `state`, `result?`                                                                                                                                                                                                                                 |

The SCM summary: `git` `{state, version?}` (`ok`, `missing`, `too-old`); `repositories`
`[{repository, kind, head {branch?, commit?, detached, unborn}, upstream? {name, ahead, behind},
operation, counts {merge, index, workingTree, untracked}, truncated, revision, busy?, warm, stale}]`
where `kind` is `root`, `nested`, `parent` or `file` and `operation` is `none`, `merge`, `rebase`,
`cherry-pick` or `revert`; `candidates` and `unsafe` paths; `repositoriesTruncated`. `repository` is
the canonical toplevel path.

`files-edit`, batch ②:

| Op                   | Request                                                                                                                                                                                                   | Result                                                                                     |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `fs.write`           | `requestId`, `transfer`, `path`, `encoding`, `bom`, `precondition` (`{mtimeMs, size}`, `create`, `none`), `unlock`, `history?` (`save`, `restore`); UTF-8 text follows as Puts, the last flagged `commit` | `mtimeMs`, `size`, `replaced`                                                              |
| `fs.create`          | `requestId`, `path`, `type`                                                                                                                                                                               | `entry`                                                                                    |
| `fs.rename`          | `requestId`, `from`, `to`                                                                                                                                                                                 | `entry`                                                                                    |
| `fs.move`, `fs.copy` | `requestId`, `sources`, `targetDirectory`, `conflict` (`fail`, `replace`, `increment`)                                                                                                                    | `items` `[{source, path?, error?}]`                                                        |
| `fs.exists`          | `directory`, `names`                                                                                                                                                                                      | `existing` names                                                                           |
| `fs.delete`          | `requestId`, `paths`, `mode` (`trash`, `permanent`), `capture`                                                                                                                                            | `items`, `stash?`                                                                          |
| `fs.restore`         | `requestId`, `stash`                                                                                                                                                                                      | `items`                                                                                    |
| `fs.clipboard`       | `paths`, `cut`                                                                                                                                                                                            | —                                                                                          |
| `fs.import-begin`    | `requestId`, `import`, `targetDirectory`, `items` `[{path, type, size, conflict}]` (≤ 1,000; `fs.import-items` adds more)                                                                                 | —                                                                                          |
| `fs.import-file`     | `import`, `index`, `transfer`; the bytes follow as Puts, the last flagged `commit`                                                                                                                        | The item's outcome                                                                         |
| `fs.import-end`      | `import`                                                                                                                                                                                                  | `items` `[{index, error?}]`                                                                |
| `fs.export-begin`    | `export`, `paths`                                                                                                                                                                                         | —                                                                                          |
| `fs.export-entries`  | `export`, `cursor?`                                                                                                                                                                                       | `entries` with `index`, relative `path`, `type`, `size`, `mtimeMs` or `skipped`; `cursor?` |
| `picker.open`        | `picker`                                                                                                                                                                                                  | `home`, `roots`                                                                            |
| `picker.list`        | `picker`, `path`, `cursor?`                                                                                                                                                                               | `entries`, `cursor?`                                                                       |
| `picker.close`       | `picker`                                                                                                                                                                                                  | —                                                                                          |

`files-search`, batch ③:

| Op                   | Request                                                                                                                                                                                                                                                                                | Result                                                                                                                                            |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `search.text`        | `search`, `pattern`, `regex`, `caseSensitive`, `wholeWord`, `multiline`, `folders` `[{path, includes?}]`, `includes?`, `excludes?`, `useIgnoreFiles`, `useParentIgnoreFiles`, `followSymlinks`, `encoding?`, `maxResults` ≤ 20,000, `preview` `{matchLines, charsPerLine}`, `context?` | —                                                                                                                                                 |
| `search.next`        | `search`                                                                                                                                                                                                                                                                               | `files` `[{path, matches? [{ranges, preview}], context? [{line, text}]}]`, `done`, `limitHit`, `omittedLongLines`, `symlinkLoops?`, within 250 ms |
| `search.find-files`  | `cache`, `query`, `folders`, `includes?`, `excludes?`, `useIgnoreFiles`, `useParentIgnoreFiles`, `followSymlinks`, `maxResults` ≤ 512                                                                                                                                                  | `results` `[{path, score, labelMatches?, descriptionMatches?}]`, `truncated`, `limitHit`                                                          |
| `search.close-cache` | `cache`                                                                                                                                                                                                                                                                                | —                                                                                                                                                 |

`files-history`, batch ④: `history.list` (`path` → entries `{id, source, label?, timestamp, size}`),
`history.restore`, `history.rename`, `history.delete` (one entry or all), `git.log` (paged, with
graph parents and refs), `git.blame` (paged), and `git.checkout`, `git.branch`, `git.merge`,
`git.rebase`, `git.cherry-pick`, `git.stash`, `git.tag`, `git.remote`, `git.clone`,
`git.undo-commit` and `git.abort`, each with `requestId` and the arguments of VS Code's command at
ef5ca45, fixed in the schema before ④ is released. History content is read with `fs.read {history}`.

## Scope and authorization

The Server admits a File Bar request only from a connection whose Session grants include
`terminal-create`; mutations and relay answers also need `lifecycle`. Otherwise the answer is
`denied`, and the Workspace must belong to the Session. Such a Client can already run any command
as the user, so the File Bar grants nothing beyond terminal creation; the boundary keeps the service
narrow and explicit and is not a sandbox against a Client holding these grants.

| Member                 | Established by                                        | Allows                                                                  | Ends                                             |
| ---------------------- | ----------------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------ |
| Root                   | The Server, from `Navigation::workspace_current_path` | Everything in this proposal                                             | Root change                                      |
| Consented repositories | `repository.consent` after the user's Yes or Always   | The repository's working tree                                           | `repository.close`                               |
| Opened files           | `fs.grant`                                            | That file: read, stat, watch, Local History, write in place, recreation | `fs.ungrant` or the connection's end             |
| Picker session         | `picker.open`                                         | Listing any directory: names, types, sizes, times                       | `picker.close`, or 10 minutes idle               |
| Picker selection       | `fs.grant {source: save-as}`                          | Creating and writing that one file, which then counts as opened         | As opened files                                  |
| Relay message file     | git's editor call                                     | The file git names inside that repository's git directory               | The relay's end                                  |
| Clipboard sources      | `fs.clipboard` with in-boundary paths                 | Reading, copying and moving those paths after the root moves            | The next `fs.clipboard`, or the connection's end |
| Typed Quick Open path  | `fs.stat {source: quick-open}`                        | Stat of exactly that path; nothing is listed                            | The request                                      |
| Search include         | A `search.text` include outside the root              | rg reading that directory for that search only                          | That search's end                                |

- Consents are kept per machine and path in `internal/state/git-parent-repositories.json`
  (`{version: 1, repositories}`), replaced atomically under a lock by `orchbee-files`.
- Containment is checked by `orchbee-files` when execution starts; an operation already running
  finishes under the scope that admitted it.
- Paths from Clients have no `.`, `..` or empty segments, no NUL, and no `\\?\`, `\\.\`, device or
  UNC form; a UNC root gives `unsupported-location`, as the Worker refuses `\\?\UNC\`.
- Reads (list, stat, read, watch) need the requested path lexically inside a member; links inside
  are followed, as VS Code follows them.
- Mutations need the real location of the target, or of its parent for create, rename, move, copy
  and delete, inside a member's real path or an explicit grant; otherwise `link-target-outside`.
- Real paths compare component by component: on Windows the `GetFinalPathNameByHandleW` spelling,
  which resolves junctions, 8.3 names and case; on macOS by Unicode simple case folding unless
  native tests prove the canonical spelling; on Linux bytewise. Names are never normalized.
  Junctions and other name-surrogate reparse points count as links.
- A name that is not valid UTF-8 is listed with `unrepresentable: true` and cannot be operated on.
- The root facts report `os` and `caseSensitive`: Linux sensitive, Windows and macOS insensitive, as
  in VS Code. The frontend validates names by the Worker's OS; `orchbee-files` rejects again empty
  names, `.`, `..`, separators and NUL, and on Windows `<>:"|?*`, control characters, reserved
  device names and trailing dots or spaces.
- `/`, `~` and drive roots are allowed; they are watched only in loaded folders.

It is not a generic file service:

- Its scope derives from Server-owned Workspace state and explicit user acts, never from a root a
  Client names.
- A closed, typed operation set: no argument passthrough to git or rg, no arbitrary command.
- Only Clients reach it, through a handshake capability; the CLI, plugins and Agents do not, and the
  Worker baseline gains no file or Git authority.
- Every request needs the authority of terminal creation, and the process runs only while the
  File Bar is used.
- It is the separately admitted owner of file and Git effects that
  [`connected-client-capability-service.md`](connected-client-capability-service.md) leaves optional.

## Files

### Listing

- `fs.list` returns entries unsorted, in pages, continuing from `cursor` (≤ 16 open cursors per
  connection, 30 s idle); a directory that changes between pages may repeat or miss entries, which
  the watch then reports.
- `singleChild` adds the chain of single-child directories below `path`, up to 64 levels.

### Reads

- Detection follows VS Code's `detectEncodingFromBuffer`: a BOM (UTF-8, UTF-16LE, UTF-16BE); else a
  zero byte in the first 512 bytes means binary unless its pattern indicates UTF-16; else the
  requested `encoding`, default `utf8`. `autoGuessEncoding` stays off; `fs.guess-encoding` reads
  4,096–65,536 bytes and maps chardetng's answer into VS Code's guessable set, or returns null.
- A binary file without `forceText` returns `binary: true` and no chunks. `mode: bytes` serves
  image, audio and video previews built as data: URLs.
- Text chunks carry the decoder's UTF-8 output; malformed input becomes U+FFFD and offsets count
  output bytes. The final chunk carries the line ending by VS Code's buffer rule (CRLF when carriage
  returns exceed half of all line breaks, none without breaks) and `changed` when the file's mtime
  or size moved during the read. One handle is held for the transfer.
- Decoding uses encoding_rs, plus in-house tables for VS Code encodings that WHATWG maps elsewhere
  or lacks: `iso88591`, `iso88599`, `iso885911`, `cp437`, `cp850`, `cp852`, `cp857`, `cp865`,
  `cp1125`, `koi8ru`, `koi8t`. `cp950` and `big5hkscs` decode as Big5 and `gb2312` as GBK.
- `limit` is the Client's ceiling, after VS Code's confirmations (1,024 MB local, 10 MB remote,
  50 MB Web); a larger file is `too-large` before any chunk.

### Writes

1. Before commit, Puts are transcoded from UTF-8 into the target encoding and staged in memory, at
   most 64 MiB per process, beyond that in private temporary files. Unmappable characters become `?`
   (iconv-lite parity, through encoding_rs's encoder without replacement) and are counted in
   `replaced`. UTF-8 keeps the requested BOM; UTF-16 always writes one and is encoded in-house,
   since encoding_rs has no UTF-16 encoder. A staging idle for 60 s expires.
2. The Put flagged `commit` checks the precondition by VS Code's `validateWriteFile` rule:
   `content-newer` when the file's mtime is newer than the precondition's and its size differs,
   unless the file already holds the new content. `create` fails with `exists`; `none` skips the
   check. A read-only file is `read-only` unless `unlock` clears its read-only attribute or owner
   write bit. There is no elevation and no privileged helper.
3. Commit point: the file is opened for writing without truncation, rewritten in place and set to
   its new length, so its hidden attribute, alternate data streams, ownership, hard links and link
   target survive, as in VS Code; a new file is created exclusively. No fsync.
4. A failure after the commit point is `write-incomplete` with its cause, and the editor keeps its
   dirty buffer. Success returns `mtimeMs` and `size`, then records Local History (④), whose failure
   never fails the save.

### File operations

- `fs.create` is exclusive; `fs.rename` performs a case-only rename directly; `fs.move` across
  volumes copies, then deletes the source.
- `increment` computes VS Code's `simple` name (`name copy`, `name copy 2`; the English literal is
  file-name data) and creates it exclusively, so a concurrent writer is never overwritten; results
  report final names. `replace` removes the existing target permanently first, as VS Code does.

### Delete, trash and undo

- `mode: trash` uses that machine's trash, local or remote: the Recycle Bin through an
  `IFileOperation` adapter in `orchbee-files`, which shows no window and refuses a deletion the
  shell announces as permanent; the macOS Trash through `NSFileManager trashItemAtURL`, which
  needs no Finder automation permission and offers no Put Back; the freedesktop Trash (home and
  `$topdir/.Trash-$uid`), also on headless Linux. No trash gives `trash-unavailable` and a failure
  `trash-failed`; the frontend then shows VS Code's permanent-delete confirmation and resends with
  `mode: permanent`. The trash path never falls back to permanent deletion by itself.
- `capture` first copies each file of at most 5,000,000 bytes (VS Code's `MAX_UNDO_FILE_SIZE`) into
  an in-memory stash, at most 64 MiB per process, oldest evicted, lost on exit. `fs.restore`
  recreates the files and the folders, empty, as VS Code does; an evicted stash is `undo-expired`.
- `git.clean` with `trash` trashes untracked files five at a time; `permanent` runs
  `git clean -f -q`.

### Blocking calls

A filesystem call that can hang, such as on a network volume, runs on a bounded helper thread with
a 30 s deadline (`timeout`). With 8 abandoned helpers outstanding, new filesystem work is
`unavailable{stuck-filesystem}` until they return; late results are dropped.

## Transfers

- Import covers Web upload, Desktop drag-in from the OS and Desktop paste of OS files: the Client
  reads its own files and sends them, so no outside path is ever granted.
  - `fs.exists` finds conflicts for the frontend's prompt; then `fs.import-begin`, per file
    `fs.import-file` with Puts, and `fs.import-end`.
  - Each file is staged as a hidden `.<name>.orchbee-<random>` beside its target and published
    without replacement: hard-linked to its final name, then the staging name removed; a filesystem
    without hard links uses exclusive creation. `replace` removes the existing item permanently
    first. The watcher drops events for its own staging names.
  - A failure or LaneClose removes staging; published items stay, and `fs.import-end` reports each.
- Export serves Download: `fs.export-begin` captures the selection, `fs.export-entries` pages its
  entries, and each file is read with `fs.read {export, index}`. Directory links are not followed;
  a file link is followed only when its target lies in the boundary, and is otherwise `skipped`.
  The Client writes the download.
- Cross-machine paste is not in ②. It can later be composed from export on the source machine and
  import on the target through the Client, with no new backend operation; until then Paste is
  disabled while the clipboard holds another machine's items.
- Reveal in OS belongs to Desktop (`shell.showItemInFolder`, local Session only). Open in Integrated
  Terminal uses Workbench terminal creation. Neither involves `orchbee-files`.

## Watching

- One watcher per process, reference-counted per path.
- Each Workspace root has a recursive watch: ReadDirectoryChangesW and FSEvents through `notify`. On
  Linux a walker adds non-recursive inotify watches, skipping VS Code's default
  `files.watcherExclude` (`.git/objects`, `.git/subtree-cache` and `.hg/store`, at the root and one
  level down), within min(¼ of `max_user_watches`, 131,072) watches. Past that budget or on `ENOSPC`
  the root falls back to loaded folders and reports `watch: degraded`.
- A filesystem root, a drive root or the home directory gets no recursive watch: loaded folders
  only.
- An opened file outside the root is watched through a non-recursive watch of its parent.
- Events coalesce for 75 ms by VS Code's `coalesceEvents`; throttling allows 500 events per 200 ms
  and 30,000 buffered. Overflow, a Windows buffer overflow and an FSEvents rescan flag become
  `rescan`, where VS Code drops events.
- A deletion of an opened file is held 100 ms and reported as `changed` if the file exists again;
  the frontend adds no delay of its own.
- A missing root is polled every 5,007 ms; `root` Updates report `missing` and `present`.
- A failed watcher restarts; after 5 restarts the Workspace reports `watch: stopped` until the
  process restarts, which the Explorer's Reload requests.
- Each Client receives events for its loaded folders and opened files only.

## Search and Quick Open

- rg runs with the arguments of VS Code's `getRgArgs` and file-search builder at ef5ca45, ported:
  text search `--hidden --no-require-git`, case flags, `-g` globs, `--max-filesize`,
  `--no-ignore-parent` or `--no-ignore`, `--follow`, `--encoding`, `--crlf`, `--engine auto`,
  `--regexp` or `--fixed-strings`, `--no-config`, `--no-ignore-global`, `--json`, `--multiline`,
  context flags, `-- <pattern> .`; file search `--files --hidden --case-sensitive --no-require-git`
  with the same ignore and glob flags and `--no-config`.
- Parent ignore files are the one departure from those arguments. With `useIgnoreFiles` and without
  `useParentIgnoreFiles`, when the root lies inside a Git working tree below its top, text search
  and file listing drop both `--no-ignore-parent` and `--no-require-git`, so the `.gitignore` files
  between the root and the top apply and none above the top does. The top is the first directory
  holding `.git`, a directory or a linked worktree's file, on the walk up from the root when each
  rg starts; when that is the root itself or no `.git` exists, VS Code's flags stay. Dropping only
  `--no-ignore-parent` would read every ancestor's `.gitignore` up to the filesystem root, a home
  directory's one for dotfiles included. `.ignore` and `.rgignore` files in any ancestor also
  apply. rg reads these files itself and reports only paths inside the root.
- An exclude key starting with `!` is an exception: its path and everything under it are searched
  even where an ordinary exclude or an ignore file hides them. Its glob is the key without leading
  whitespace, the `!` and a trailing `/`; empty and absolute globs are dropped, and an exception is
  never an ordinary `-g` exclude. With exceptions, `search.text` runs VS Code's second pass
  (`provideTextSearchResultsWithRgOptions`, and `getRgArgs` with `excludeExceptions`): after a first
  pass that ignores them ends without `limitHit` or cancellation, rg runs again with `--no-ignore`,
  no include or ordinary exclude, and these globs in order: when no exception starts with `**`, `!*`
  followed by every parent directory of each exception with a trailing `/`; then `e` and `e/**` for
  each exception `e`; then `!**/<name>` for each of `.git`, `.svn`, `.hg` and `.jj` that no
  exception names. The exceptions are brace-expanded first, and every glob derived from one is
  anchored with a leading `/` unless it starts with `**` or `/`. Files under an exception that the
  first pass did not report join the results, within the same `maxResults`.
- Every file rg reports, in either pass, is matched again against the request's includes and
  excludes with VS Code's glob semantics, as its `TextSearchManager` does, because rg's globs match
  more: `**.js` matches at any depth. Requests carry no `when` clause: VS Code's come only from
  settings, which are fixed, and the inputs cannot express one.
- `--json` lines are parsed with a 16 MiB bound per line; longer lines count in `omittedLongLines`.
  Results stop at `maxResults` with `limitHit` and arrive in batches of at most 128 KiB.
- A new `search.text` from the same connection and Workspace cancels the previous one; at most 4 rg
  children run per process. A pattern rg rejects is `invalid-regex` with rg's message as `detail`.
- An include the user types as an absolute, `~` or `../` path outside the root arrives as a
  `folders` entry: rg reads that directory for that one `search.text`, and a result opened from it
  is granted with `source: quick-open`.
- Quick Open lists files once per `cache` (VS Code's `cacheKey`), discarded on
  `search.close-cache`, on a root change or after 10 minutes idle. A list holds at most 500,000
  paths or 128 MiB (`truncated`). Because one process serves every Workspace on the machine, a
  Workspace keeps at most 2 lists, the one serving queries and one loading to replace it, and all
  lists in the process total at most 256 MiB, the least recently used evicted first. After an idle
  discard the next Quick Open waits for its listing. Each query is scored by a Rust port of
  VS Code's `fuzzyScorer.ts` and returns the best 512 with match ranges.
- Quick Open reaches past the root only through what the user types or picks. For an absolute, `~`
  or `../` path or a `file://` link, which the Client resolves to one absolute path,
  `fs.stat {source: quick-open}` stats exactly that path and lists nothing; the Client shows it only
  when it is a file, as VS Code's Quick Open does. Selecting that result, or a history entry
  outside the root, is `fs.grant {source: quick-open}` with `terminal-link`'s power: each is an
  explicit user act, as a terminal link is.
- Explorer find uses `search.find-files` with its 512-file cap.
- No `ignore` or `grep-*` crate: rg owns traversal and matching.

## Git

### Program and environment

- On Windows, read-only commands run `<install>\<arch>\bin\git.exe` directly, while commands that
  may run hooks, credential helpers or ssh run the `cmd\git.exe` launcher; elsewhere `git` comes from
  the launch environment's PATH. `--porcelain=v2` requires git 2.11 or later; the summary reports
  `missing` or `too-old`.
- The environment is the Worker's terminal launch environment without `GIT_DIR`, `GIT_WORK_TREE`,
  `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_COMMON_DIR`,
  `GIT_NAMESPACE` and `GIT_PREFIX`, with `LANGUAGE=en`, `LC_ALL=en_US.UTF-8`, `LANG=en_US.UTF-8`,
  `GIT_PAGER=cat`, `GIT_TERMINAL_PROMPT=0`, the relay variables, and `GIT_OPTIONAL_LOCKS=0` for
  read-only commands. There is no `GIT_HTTP_USER_AGENT`, and the user's fsmonitor and untracked
  cache stay. The Worker's isolated environment is unchanged and serves only its own reads.
- A repository containing one of the Workspace's admitted Directories gets
  `-c safe.directory=<canonical toplevel>`, as in the Worker. Others keep git's ownership check; a
  refusal lists the repository as `unsafe`, and `git.mark-safe` writes `safe.directory` to the
  user's global configuration when the user chooses it (VS Code's Manage Unsafe Repositories).

### Repositories

- Discovery runs on every root change after a 500 ms debounce: `rev-parse` at the root; one level
  below, only subdirectories holding `.git` are `rev-parse`d. A repository above the root or above an
  opened file becomes a candidate unless consented; the frontend applies
  `git.openRepositoryInParentFolders` and sends `repository.consent`.
- The display set holds at most 256 repositories per Workspace (`repositoriesTruncated`). Moving
  inside one repository rebuilds nothing. A repository left behind joins the warm pool, at most 3
  per Workspace, least recently used out: it keeps its last state, stops watching and refreshes once
  when it returns. Agent completion marks warm repositories stale.
- Each displayed repository watches its working tree and the first level of its git directory. A
  Workspace without an SCM subscriber stops these watches and refreshes.
- The machine and `repository` key the commit-message draft that the Client saves when a repository
  leaves the display set.

### Scheduling

- Per repository one write operation runs at a time and reads may run beside it; no automatic
  status runs during a write, and a newer status cancels a running one.
- A file change starts a 1,000 ms debounce, then waits until the repository is idle and a subscribed
  Client is focused (`client.focus`), runs status and cools down for 5,000 ms. A mutation refreshes
  at once. `agent-settled` refreshes every displayed repository of its Workspace at once, skipping
  the debounce and the focus wait, after any running operation, and again after a running status.
- Status is `git status --porcelain=v2 -z --branch -uall`, killed at entry 10,001 (`truncated`,
  VS Code's 10,000 limit).
- Autofetch is the Client's: when the user enabled it, `git.fetch {background: true}` every 180 s
  while focused. A background operation never prompts.
- `git show --textconv <ref>:<path>` content is cached per repository, ref and path for 3 minutes
  after its transfer closes, swept every 5 minutes, at most 64 MiB.
- Git Output is a 1 MiB in-memory ring per Workspace of command lines and output, read with
  `git.output` and never written to disk.

| Commands                                                                 | Deadline                                   |
| ------------------------------------------------------------------------ | ------------------------------------------ |
| rev-parse, user configuration, safe.directory                            | 10 s                                       |
| status                                                                   | 120 s, or cancelled by a newer refresh     |
| HEAD, refs, remotes, stash list, show, check-ignore                      | 30 s                                       |
| add, reset, checkout and discard, in batches of ≤ 30,000 path characters | 60 s per batch                             |
| stage content, undo commit, tag, remote, init, log, blame                | 60 s                                       |
| commit                                                                   | 30 min, plus the editor wait               |
| checkout of branches, branch, stash                                      | 10 min                                     |
| merge, rebase, cherry-pick                                               | 30 min                                     |
| fetch, pull, push, clone                                                 | 60 min, failing after 5 min without output |
| credential prompt                                                        | 10 min                                     |
| commit message editor                                                    | While its tab exists, at most 8 h          |

Status output is capped by entries and content by the Client's `limit`; other stdout stops at
16 MiB (`git-failed`, `output-too-large`).

### Relay

- git children get `GIT_ASKPASS` and `SSH_ASKPASS` naming the running `orchbee-files` executable,
  `SSH_ASKPASS_REQUIRE=force`, `GIT_EDITOR="<orchbee-files>" --git-editor` (`rebase --continue`
  uses `GIT_EDITOR=true`), `ORCHBEE_FILES_RELAY` (the endpoint) and `ORCHBEE_FILES_RELAY_TOKEN`
  (256 bits, one per operation). Started with these and without `--local-endpoint`, the executable
  acts as the helper; the mode is private to one build.
- The helper connects to its process's relay endpoint; each side checks the other is the same user
  (`crates/local-ipc`) before any content, and the token binds the prompt to its operation.
- Prompt kinds come from the prompt text and `SSH_ASKPASS_PROMPT`: username, password, passphrase,
  host-key confirmation and editor.
- A prompt goes to the Client that started the operation while it is connected, else to the most
  recently focused connected Client that views the Workspace and holds `lifecycle`; with none, and
  for background operations, the helper gets an empty answer at once. Esc answers empty.
- An answer is written to the helper and dropped: never logged, persisted, cached or echoed into Git
  Output. Zeroization is not claimed.
- The user's credential helpers run first, as git orders them.
- The editor prompt opens git's message file for the relay's duration; the helper returns when the
  frontend reports its tab closed everywhere.
- On a remote machine the relay runs there and ssh uses that machine's `SSH_AUTH_SOCK`; machine
  connections keep `ForwardAgent=no`.

## Local History

It ships in ④ with the timeline.

- It records File Bar saves (`history: save` or `restore`) and File Bar moves and renames, whose
  entries follow the file. An entry is a copy of the saved bytes, timestamped with the file's new
  mtime. VS Code's defaults apply: files over 256 KB are skipped, at most 50 entries per file, and a
  save within 10 s of the file's previous entry from the same source replaces it.
- It lives on the machine holding the file:

  ```text
  internal/local-history/
  ├── index.json           source path to folder; rebuildable from the folders
  ├── history.lock
  └── 000001/
      ├── entries.json     {version: 1, source, entries: [{id, file, source, label?, timestamp}]}
      └── 2026-11-27T08-30-15-123Z.ts
  ```

- `entries.json` and `index.json` are replaced atomically under `history.lock`, waited for at most
  500 ms, which Sessions sharing the data root also take; at most 4 IO threads.
- Default uninstall keeps it; `--delete-data` and the timeline's delete actions remove it. It keeps
  copies of whatever the user saved, `.env` files included.
- VS Code caps nothing in total; the proposed cap is 1 GiB, oldest entries pruned first.

## Re-rooting

- The Server sends the new root before any request under it, and `orchbee-files` switches scope at
  once.
- The new root watch starts before the old one stops; a root inside the watched tree reuses it.
- Git discovery follows after 500 ms. The Workspace's Quick Open lists and running searches are
  discarded.
- Opened-file grants, tabs and clipboard sources are unaffected.
- A request already running finishes; a later one outside the new root is `outside-scope`.

## Remote machines

- `orchbee-files` runs beside the remote Worker and connects to the remote Server. File Bar records
  share the machine hub's `session` channel (256 KiB initial credit, up to 4 MiB) with all other
  traffic, behind terminal records by the writer's priority.
- Toward a remote machine the Client uses 32 KiB chunks and a 256 KiB window.
- Trash, Local History, consents and the relay act on the remote machine.
- All six archives carry `orchbee-files` and `rg`: `windows-x86_64`, `windows-aarch64`,
  `macos-aarch64`, `macos-x86_64`, `linux-x86_64`, `linux-aarch64`.

## ripgrep

| Fact      | Value                                                                                                                                                                                                                                                                                                                 |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Binary    | The rg VS Code ships: ripgrep 15.0.0 (rev `3a612f88b8`) with PCRE2 10.46, which `pcre2-sys` 0.2.10 bundles, and its JIT (`rg --version` says 10.45, from that crate's stale bindings); the Linux programs are static musl builds for remote machines                                                                  |
| Source    | `@vscode/ripgrep-universal` at the version VS Code ef5ca45 resolves (`^1.18.0`), which carries all six targets, pinned in `apps/orchbee-files/ripgrep.json` with archive, binary and notice-text SHA-256s and fetched by `scripts/ripgrep.mjs`, as `apps/terminal-host/windows-conpty.json` is. Not a pnpm dependency |
| License   | The notices below, in `THIRD_PARTY_NOTICES.md` with their full texts shipped                                                                                                                                                                                                                                          |
| Placement | `rg[.exe]` beside `orchbee-files`; development and test cohorts stage the same verified file beside their executables                                                                                                                                                                                                 |
| Discovery | The sibling of the running executable, never PATH; its SHA-256 is checked against the digest built in from `ripgrep.json` before the first spawn per process; a mismatch makes search `unavailable{search-binary}`                                                                                                    |
| Packaging | Listed in `release.json` and on the macOS executable-mode list                                                                                                                                                                                                                                                        |
| Signing   | macOS: ad-hoc in CLI archives; Developer ID, hardened runtime and notarization inside the app, without `com.apple.security.cs.allow-jit`, so PCRE2 runs its interpreter there. Windows: as Orchbee's programs                                                                                                         |
| Antivirus | Defender scans it at first spawn; SmartScreen does not apply to child processes                                                                                                                                                                                                                                       |
| Size      | Windows x64: 5,440,872 bytes, 2,041,319 gzip-compressed. With `orchbee-files` (about 2 MB compressed, estimated) an archive grows about 4 MB, about 15% of 27.3 MB, and an installation about 10.5 MB                                                                                                                 |

| Component                                                                                  | License                                                                                                              | Notice                                                                   |
| ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| ripgrep 15.0.0 (rev `3a612f88b8`)                                                          | MIT, chosen from Unlicense OR MIT                                                                                    | "Copyright (c) 2015 Andrew Gallant" and the MIT text                     |
| PCRE2 10.46, linked statically; `--engine auto` uses it for look-around and backreferences | BSD-3-Clause with the PCRE2 exception                                                                                | The whole LICENCE, its SLJIT section included, since the JIT is built in |
| The crates rg 15.0.0 links statically                                                      | Each crate's own: mostly MIT, MIT OR Apache-2.0 or Unlicense OR MIT; `encoding_rs` adds BSD-3-Clause for WHATWG data | Generated once from rg 15.0.0's `Cargo.lock`, pinned with rg             |
| jemalloc                                                                                   | BSD-2-Clause                                                                                                         | The Linux programs link it                                               |

- The package's own LICENSE is the npm wrapper's MIT ("vscode-ripgrep", Copyright (c) Microsoft
  Corporation) and is no evidence for rg's license. The package holds only `LICENSE`,
  `package.json`, `lib/index.js` and `bin/`, so the rg and PCRE2 texts come from their sources,
  and the notice-text SHA-256s in `ripgrep.json` pin those texts.
- VS Code's own notices list rg 13.0.0 and no PCRE2, so they are stale.
- The notices ship with the first build that distributes rg, the six remote archives included.

## Dependencies

| Crate                                                                                      | Use                                                              | Status              |
| ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------- | ------------------- |
| `notify`                                                                                   | ReadDirectoryChangesW, FSEvents and inotify, without a debouncer | Approved 2026-11-25 |
| `trash`                                                                                    | macOS Trash (`NsFileManager` method) and freedesktop             | Needs admission     |
| `encoding_rs`                                                                              | Decoding and legacy encoding                                     | Needs admission     |
| `chardetng`                                                                                | `fs.guess-encoding` only                                         | Needs admission     |
| `sha2`, `tempfile`, `process-wrap`, `serde`, `serde_json`, `rustix`, `windows-sys`, `time` | Hashing, staging, children, codecs, platform calls, entry names  | In the workspace    |

`git2`, `gix` and `tokio` stay out by the owner's decision; `ignore` and `grep-*` because rg covers
them; `notify-debouncer-*` because VS Code's coalescing is ported. Admission review checks the
duplicate `windows`, `windows-sys` and `objc2` versions these crates pull in. The Recycle Bin
has its own adapter, since `trash` sets `FOF_WANTNUKEWARNING` there and asks before deleting
permanently what it cannot recycle. The adapter uses the workspace `windows` bindings and
`windows-core`, both already in the lockfile; the owner accepted its authored unsafe boundary on
2026-11-27.

## Failure containment and diagnostics

- `orchbee-files` is one failure domain: a panic on any of its threads ends the process, and the
  Server restarts it as under Lifecycle. Terminals, Workbench state and other Clients are
  unaffected; open File Bar streams close with OutcomeUnknown for mutations and
  `unavailable{restarting}` otherwise.
- A malformed OBFL frame ends the link. A malformed Client record ends only its stream: the Server
  closes it with InvalidRequest, `orchbee-files` answers `invalid-request`.
- Memory is bounded by write staging (64 MiB), the undo stash (64 MiB), Quick Open lists
  (256 MiB in total), the `git show` cache (64 MiB), the outbound queue (4 MiB) and Git Output
  (1 MiB per Workspace).
- `install_diagnostic_log("files")` writes lifecycle events, exit reasons, restarts, watch overflows
  and fallbacks, timeouts by operation kind and spawn failures to `logs/<date>.jsonl` as tokens and
  counts. Paths, names, contents, queries, git output, prompts, answers and remote URLs never enter
  it. Nothing leaves the machine; the diagnostic bundle stays user-initiated.

## Release gate

- The Server is the gate: built for the release environment, it lists no `files*` capability and
  refuses to start the process. A Client shows the File Bar only for a machine whose Server lists
  `files`; development and test Servers list them. This is the
  [Release-withheld Search](../../apps/web/docs/workbench.md#release-withheld-search) policy keyed on
  the Server, so Web, Desktop and remote machines share one gate.
- Release archives omit `orchbee-files` and `rg` until the owner releases ①; development and test
  distributions carry them.

## TUI

The GUI ships first: the floating editor has no TUI counterpart of comparable scope, and the
protocol is client-neutral, so the TUI later uses `crates/files-protocol` with no backend change.

## Evidence

| Claim                                                                                              | Evidence                                                                                                                                                                                                                                                                                                                                                                                          |
| -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust and TypeScript agree on OBFB and reject malformed records                                     | Frozen corpora checked by both bindings; single-byte mutation and random-partition tests, as in `crates/workbench-protocol`                                                                                                                                                                                                                                                                       |
| One admitted link per token; idle exit; crash backoff and stop; successor start; release refusal   | Deterministic state-machine tests with a fake clock                                                                                                                                                                                                                                                                                                                                               |
| Containment holds over `..`, links, junctions, 8.3 names, case and Unicode variants and long paths | Property tests over generated trees and paths; native tests on Windows, macOS and Linux                                                                                                                                                                                                                                                                                                           |
| Nothing changes before a write's commit point; later failures are `write-incomplete`               | Deterministic tests with a writer fault injected at each step                                                                                                                                                                                                                                                                                                                                     |
| Conflict, encoding, coalescing, rg arguments and Quick Open ranking match VS Code ef5ca45          | Differential corpora generated from `validateWriteFile`'s rule, iconv-lite and `detectEncodingFromBuffer`, `coalesceEvents`, the rg argument builders and `fuzzyScorer.ts`                                                                                                                                                                                                                        |
| Coalesced events end in the raw events' state; rg JSON and porcelain v2 parse under any split      | Property tests over generated event sequences, split and long lines, byte-encoded paths, renames, unmerged entries and paths with newlines                                                                                                                                                                                                                                                        |
| Scheduler, warm pool, re-rooting and request identity                                              | Deterministic tests with a fake clock and a fake git                                                                                                                                                                                                                                                                                                                                              |
| Platform behavior                                                                                  | System traces: killing `orchbee-files` leaves terminals intact and the File Bar recovers; HTTPS and SSH prompts end to end; a remote machine over the hub; trash on each platform, headless Linux and a Windows SSH logon included; inotify fallback under a lowered `max_user_watches`; PCRE2 patterns under the notarized macOS signature; terminal echo latency during a large remote download |

No TLA+ model: link admission, the only cross-process handover, has few orders and named tests
cover them, and a save races an external writer that follows no protocol. Tests are added only for
these gaps, as [`TESTING.md`](../../docs/TESTING.md) admits them, and corpora assert rules.

## Document changes

- [`native-runtime-topology.md`](native-runtime-topology.md): the executable set and lifetimes gain
  `orchbee-files`; the goal and non-goal lines on generic Files and Git name it as the scoped owner
  while the Worker baseline stays narrow.
- [`connected-client-capability-service.md`](connected-client-capability-service.md) names it as
  the owner of file and Git effects.
- [`ARCHITECTURE.md`](../../docs/ARCHITECTURE.md): the runtime diagram gains `apps/orchbee-files`.
- [`data-root-layout.md`](data-root-layout.md): an `internal/local-history` row (kept; safe to delete
  by hand, losing history); `internal/state` also holds parent-repository consents.
- [`release-distribution.md`](release-distribution.md): the Programs row adds `orchbee-files` and
  `rg`.
- `THIRD_PARTY_NOTICES.md`: the [ripgrep notices](#ripgrep) and the admitted crates.
- `apps/orchbee-worker/README.md` and `apps/orchbee-server/README.md`: the launch, the link, the
  relay of File Bar records, their priority and the release refusal.
- New owners: `apps/orchbee-files/README.md`, `crates/files-protocol/README.md`,
  `crates/git-process/README.md` and `schemas/files/README.md`.

## Slices

Each batch starts with its protocol slice. 1.1, 1.3 and 3.1 can start at once; the frontend builds
its TypeScript binding and a fake Server from 1.1's corpus. After 1.8 the read (1.9–1.11), watch
(1.12–1.13), Git (1.14–1.21) and trash (1.18) lines run in parallel.

Batch ①, `files`:

| ID   | Slice and interfaces                                                                                      | Files                                                                                             | Verification                           | After              |
| ---- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | -------------------------------------- | ------------------ |
| 1.1  | OBFB kinds 1–5 and the ① operations                                                                       | `schemas/files/v1/files.schema.json`, `files-conformance-v1.json`, `crates/files-protocol`        | Corpus and mutation tests              | —                  |
| 1.2  | `files` capability and OBFB admission in the multiplex                                                    | `connections.json`, `registry.rs`, `workbench-multiplex-files-conformance-v1.json`, both decoders | Corpus with and without the capability | 1.1                |
| 1.3  | Extract git resolution, admissible paths, children and bounded output                                     | `crates/git-process`, `apps/orchbee-worker`                                                       | Existing Worker tests unchanged        | —                  |
| 1.4  | Executable, signals, diagnostic role, OBFL codec, hello, token, `ready`                                   | `apps/orchbee-files`, `crates/files-protocol`                                                     | Codec and admission tests              | 1.1                |
| 1.5  | `files-start` on OBCN, sibling verification, Job and retained spawn, launch environment                   | `apps/orchbee-worker`                                                                             | Spawn and reap tests per platform      | 1.4                |
| 1.6  | One-shot endpoint, admission, demand start, idle, backoff, drain, release refusal                         | `apps/orchbee-server`                                                                             | Lifecycle state-machine tests          | 1.2, 1.5           |
| 1.7  | Relay between Client streams and OBFL, connection bounds, writer priority, connection loss                | `apps/orchbee-server`                                                                             | Deterministic delivery tests           | 1.6                |
| 1.8  | Root updates, containment, `fs.grant`, `fs.ungrant`, root facts                                           | `apps/orchbee-server`, `apps/orchbee-files`                                                       | Containment properties, native tests   | 1.7                |
| 1.9  | `fs.list`, `fs.release`, `fs.stat`                                                                        | `apps/orchbee-files`                                                                              | Page-bound tests                       | 1.8                |
| 1.10 | `fs.read`, `fs.read-chunk`, `transfer.close`, detection, UTF-8 and UTF-16                                 | `apps/orchbee-files`                                                                              | Detection corpus, transfer tests       | 1.9; `encoding_rs` |
| 1.11 | Legacy decoders, in-house tables, `fs.guess-encoding`                                                     | `apps/orchbee-files`                                                                              | iconv-lite differential                | 1.10; `chardetng`  |
| 1.12 | Watch adapters, Linux walker and budget, root polling, restarts                                           | `apps/orchbee-files`                                                                              | Platform traces                        | 1.8                |
| 1.13 | Coalescing port, throttle, `rescan`, delete hold, delivery filter, `fs.subscribe`                         | `apps/orchbee-files`                                                                              | Differential corpus, property test     | 1.12               |
| 1.14 | Discovery, candidates, consents, `repository.consent`, `repository.close`                                 | `apps/orchbee-files`                                                                              | Fake-git tests                         | 1.3, 1.8           |
| 1.15 | Status runner and parser, scheduler, `client.focus`, `agent-settled`, `scm.subscribe`, `git.status-page`  | `apps/orchbee-files`, `apps/orchbee-server`                                                       | Parser properties, scheduler tests     | 1.13, 1.14         |
| 1.16 | Display set, warm pool, stale marks, scope switch, watch handover                                         | `apps/orchbee-files`                                                                              | Deterministic tests                    | 1.15               |
| 1.17 | `git.show` with its cache, `git.check-ignore`, `git.refs`, `git.user-config`                              | `apps/orchbee-files`                                                                              | Fixture-repository tests               | 1.10, 1.15         |
| 1.18 | Trash adapters and `git.clean`                                                                            | `apps/orchbee-files`                                                                              | Platform traces                        | 1.8; `trash`       |
| 1.19 | Stage, unstage, `git.stage-content`, discard, commit, init, `git.mark-safe`, request identity, Git Output | `apps/orchbee-files`                                                                              | Temporary-repository tests             | 1.15               |
| 1.20 | Helper mode, relay endpoint, routing, `relay.subscribe`, `relay.answer`, editor commits                   | `apps/orchbee-files`                                                                              | HTTPS and SSH traces                   | 1.19               |
| 1.21 | Fetch, pull, push, sync, publish; output watchdog; background refusal                                     | `apps/orchbee-files`                                                                              | Traces against local remotes           | 1.20               |
| 1.22 | Development and test archives carry `orchbee-files`, release omits it; macOS lists; `release.json`        | Packaging scripts                                                                                 | Development package build              | 1.4                |
| 1.23 | Remote and isolation traces; Document changes                                                             | Documents above                                                                                   | System traces                          | 1.1–1.22           |

Batch ②, `files-edit`:

| ID  | Slice and interfaces                                                    | Files                                                 | Verification                                              | After     |
| --- | ----------------------------------------------------------------------- | ----------------------------------------------------- | --------------------------------------------------------- | --------- |
| 2.1 | ② operations and the `files-edit` capability                            | `schemas/files/v1`, `crates/files-protocol`, registry | Corpus tests                                              | 1.2       |
| 2.2 | `fs.write`, staging, encoders, conflict rule, `unlock`, commit boundary | `apps/orchbee-files`                                  | Commit-boundary tests; conflict and encoder differentials | 1.11, 2.1 |
| 2.3 | `fs.create`, `fs.rename`, `fs.move`, `fs.copy`, `fs.exists`, name rules | `apps/orchbee-files`                                  | Per-OS name tests                                         | 2.1       |
| 2.4 | `fs.delete`, stash, `fs.restore`                                        | `apps/orchbee-files`                                  | Deterministic tests, trash traces                         | 1.18, 2.1 |
| 2.5 | Import and staged publication                                           | `apps/orchbee-files`                                  | Failure-cut tests                                         | 2.1       |
| 2.6 | Export and link rules                                                   | `apps/orchbee-files`                                  | Example tests                                             | 2.1       |
| 2.7 | `picker.*`, `fs.clipboard`, the Save As grant                           | `apps/orchbee-files`                                  | Containment tests                                         | 2.1       |
| 2.8 | Agent-edit conflicts, remote upload and download latency                | —                                                     | System traces                                             | 2.2–2.6   |

Batch ③, `files-search`:

| ID  | Slice and interfaces                                                                                                          | Files                                                                                         | Verification                     | After    |
| --- | ----------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | -------------------------------- | -------- |
| 3.1 | rg supply, cohort staging, notices, signing, identity check                                                                   | `apps/orchbee-files/ripgrep.json`, `scripts/ripgrep.mjs`, packaging, `THIRD_PARTY_NOTICES.md` | Script checks; macOS PCRE2 trace | 1.4      |
| 3.2 | ③ operations and the `files-search` capability                                                                                | `schemas/files/v1`, `crates/files-protocol`, registry                                         | Corpus tests                     | 1.2      |
| 3.3 | Argument port with the parent ignore rule, exception pass and glob recheck; include scope; JSON parser, batches, cancellation | `apps/orchbee-files`                                                                          | Differential and property tests  | 3.1, 3.2 |
| 3.4 | Per-Workspace file lists, scorer port, `search.find-files`, `fs.stat {source: quick-open}`                                    | `apps/orchbee-files`                                                                          | Scorer differential              | 3.1, 3.2 |

Batch ④, `files-history`:

| ID  | Slice and interfaces                                                                                            | Files                                                 | Verification                               | After         |
| --- | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- | ------------------------------------------ | ------------- |
| 4.1 | ④ operations and the `files-history` capability                                                                 | `schemas/files/v1`, `crates/files-protocol`, registry | Corpus tests                               | 1.2           |
| 4.2 | Local History store, locks, caps, index rebuild, save and move hooks, `history.*`                               | `apps/orchbee-files`                                  | Deterministic tests; two-Session lock test | 2.2, 2.3, 4.1 |
| 4.3 | `git.log` with graph, file history, `git.blame`                                                                 | `apps/orchbee-files`                                  | Fixture-repository tests                   | 1.15, 4.1     |
| 4.4 | Checkout, branch, merge, rebase, cherry-pick, stash, tag, remote, clone with a picker grant, undo commit, abort | `apps/orchbee-files`                                  | Temporary-repository tests                 | 1.20, 4.1     |

Inline merge actions and the search editor need nothing beyond 2.2 and 3.3.

## Reserved decisions

On 2026-11-27 the owner asked for implementation to proceed on these recommendations. Each needs
acceptance before the release it blocks.

| #   | Decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | Reserved as                                                         | Recommendation                                                                                                                                                                                                                          | Blocks               |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| 1   | Wire contract: OBFB, the `files`, `files-edit`, `files-search` and `files-history` capabilities and `schemas/files/v1`, each frozen at its batch's release                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Public contract and compatibility across Client and Server versions | Accept                                                                                                                                                                                                                                  | Each batch's release |
| 2   | Admit `trash`, `encoding_rs` and `chardetng`; `notify` was admitted on 2026-11-25                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | Dependency admission                                                | Admit all three. `trash` wraps the macOS and freedesktop trash without authored unsafe code; `encoding_rs` is the WHATWG reference implementation; `chardetng` serves only "Guessed from content" and could be dropped                  | Release of ①         |
| 3   | Access boundary: authority equal to terminal creation; the members and lifetimes under [scope and authorization](#scope-and-authorization); reads follow links while mutations through links that leave the boundary fail closed; the Save As picker lists any directory; files pasted or dropped from the OS are one-time read grants for that copy; Quick Open stats exactly the path the user types and grants what the user selects there, history entries outside the root included; a search include outside the root lets rg read that directory for that one search; consents persist per machine; UNC roots are refused in v1 | Security and trust                                                  | Accept                                                                                                                                                                                                                                  | Release of ①         |
| 4   | Credential relay: a prompt reaches the initiating Client, else the most recently focused viewer holding `lifecycle`, else it is answered empty; background operations never prompt; secrets cross Client, Server and `orchbee-files` in memory only, without a zeroization claim                                                                                                                                                                                                                                                                                                                                                       | Security of secrets                                                 | Accept                                                                                                                                                                                                                                  | Release of ①         |
| 5   | ripgrep: the `@vscode/ripgrep-universal` source and version, about +4 MB (about 15%) per archive together with `orchbee-files`, the notices, and no JIT entitlement in the notarized macOS app                                                                                                                                                                                                                                                                                                                                                                                                                                         | Cost, a third-party asset and a security entitlement                | Accept                                                                                                                                                                                                                                  | Release of ③         |
| 6   | Stricter save conflict: compare the SHA-256 of the content read with the current file at every save and answer `content-newer` when they differ                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | A user-observable departure from VS Code                            | Accept. Agents edit files under open editors, and VS Code's modification-time and size rule silently overwrites an equal-length change or one inside the same modification-time tick; the cost is one read of the current file per save | Release of ②         |
| 7   | Local History: `internal/local-history` kept on default uninstall, copies of saved secrets, and a 1 GiB total cap that removes the oldest entries first, which VS Code does not have                                                                                                                                                                                                                                                                                                                                                                                                                                                   | Durable data, privacy and deletion                                  | Accept with the cap                                                                                                                                                                                                                     | Release of ④         |
| 8   | Cross-machine paste deferred beyond ②, with Paste disabled for another machine's items                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | Product scope                                                       | Defer; it composes later from export and import without a new backend operation                                                                                                                                                         | None                 |
