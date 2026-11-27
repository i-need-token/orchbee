# Orchbee Files

`orchbee-files` serves the File Bar for one Worker generation: it lists, reads and watches files and
runs Git for every Workspace of its Session on its machine. The Worker starts it when the Server
asks, and it then talks only to that Server, over the private OBFL link that
[`orchbee-files-protocol`](../../crates/files-protocol/README.md) defines. It accepts no Client
connection, reads no Orchbee setting and runs no command a Client names. Scope and containment are
checked here, against the Workspace roots only the Server sends; the Server relays File Bar records
without reading their paths.

## Executable interface

The Worker starts the service and the Git it runs starts the relay helper, so this interface is a
contract between programs rather than a command.

| Fact            | Contract                                                                                                                                             |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Invocation      | `--local-endpoint <endpoint>`: the absolute address of the one-shot endpoint the Server bound for this start                                         |
| Standard input  | The start's token, 64 lowercase hexadecimal digits and a line feed. It never appears among the arguments, which other processes of the user can read |
| Standard output | `ready` and a line feed once the Server admitted the hello                                                                                           |
| Exit            | 0 after the Server's drain; `128 + signal` after `SIGTERM` or `SIGINT` on macOS and Linux; 1 for every other end, which the Server counts as a crash |

The service names this executable as `GIT_ASKPASS`, `SSH_ASKPASS` and, with `--git-editor <file>`,
`GIT_EDITOR`, together with `ORCHBEE_FILES_RELAY` and `ORCHBEE_FILES_RELAY_TOKEN`, only on the
commands of an operation that may prompt. Started with both variables and without
`--local-endpoint`, it is the relay helper: it asks that service's relay, prints the answer or waits
until the message file is done, and exits 0, or 1 when the prompt was cancelled. It reads no data
root and writes no log.

Every end cancels the open calls first and gives their operations three seconds to end their
children, so a Git or ripgrep process tree does not outlive the call that started it. A panic on
any thread ends the whole process instead of unwinding that thread, because a half-serving process
would leave Client streams open; the Server restarts it on the next demand.

## Scope and grants

These rules are the contract a Client builds on; no single declaration holds them.

- A path is in scope when it lies lexically inside its Workspace's root, spelled as the Workbench
  reported it or as the real location `fs.subscribe` reports; inside the working tree of a
  repository consented to in that Workspace, spelled as `scm.subscribe` names it, from the consent's
  answer until `repository.close` answers or the summary stops listing it; when it is the file an
  editor prompt of the Workspace names, from the prompt's arrival until it closes, where only a
  file whose real location lies in its repository's Git directory or common directory opens a
  prompt; or when it names a file the requesting connection opened. A Client builds its paths under the root `fs.subscribe`
  reported.
- `fs.grant` from a tree, a diff or a Save As dialog needs the path in scope already; a terminal
  link, a restored tab or the operating system's open may name any path, since the person chose it
  explicitly. The entry is read before the grant is recorded, so a grant that fails grants nothing,
  and a directory is refused.
- Each successful grant of a file adds one grant, and each path of `fs.ungrant` releases one, so
  two tabs of one file release it independently; a set would let closing either tab cut off the
  other. Grants outlive their Workspace's close and end with their connection.
- Grants live in this process. After it restarts, which ends every subscription, a Client grants
  its open files again.

## ripgrep

Development and test cohorts carry rg beside this executable, as `rg` (`rg.exe` on Windows), with
the notices of everything compiled into it in `licenses/ripgrep-notices.txt`; a release cohort
carries neither until the File Bar ships. [`ripgrep.json`](ripgrep.json) pins, by SHA-256, the
`@vscode/ripgrep-universal` archive, the program in it for each release platform and the notices,
and staging runs [`scripts/ripgrep.mjs`](../../scripts/ripgrep.mjs), which refuses any other bytes.
The pinned programs are ripgrep 15.0.0 with PCRE2 10.46 and its just-in-time compiler; the Linux
ones are static musl builds that allocate with jemalloc. `rg --version` names PCRE2 10.45 because
the bindings of `pcre2-sys` 0.2.10 still carry that number, while the library compiled in is the
10.46 source the crate bundles. Every program loads only system libraries. A new archive
regenerates the notices from that ripgrep's `Cargo.lock`, taking each crate it links on some
platform and the license files that crate ships, and changes the ripgrep entry of
[`THIRD_PARTY_NOTICES.md`](../../THIRD_PARTY_NOTICES.md#ripgrep) with them.

## Adding a service

A service joins in `src/operations.rs`: its state as a field of `Services`, its operations as arms of
`route`, and its share of each link event that ends state: a cancel, a lane's close, a connection's
loss, a scope change and the drain. Its own modules take the shared parts they use, such as
`Scopes`, `Helpers` or a `Subscriptions` registry, and never `operations`, which depends on them;
that keeps the module graph acyclic. A filesystem call that can hang runs through `Helpers::run`.

## Diagnostic log

The process writes to the [daily diagnostic log](../../crates/local-runtime/README.md#diagnostic-log)
with role `files`: `started` once the Server admitted it; `filesystem-timeout` with the `operation`
whose filesystem call passed its deadline; `git-timeout` with the `kind` of Git command that passed
its deadline; `git-spawn-failed` with that `kind` and the I/O `error` kind when Git could not start;
`git-relay-unavailable` once when the prompt relay could not start, after which Git operations run
without prompts; and one `exited` with `outcome` (`drained` with its `reason`, `idle` or `session`;
`signalled` with its `signal`; or `failed` with the `stage` it stopped at and the I/O `error` kind
that caused it, when one did).
