# Windows Agent integrations

This record owns the Windows refinement of native `agent integration list`,
`explain`, `enable`, and `disable`. The CLI owns local onboarding; it never starts
a Site. The shared provider classifier and mutation workflow remain authoritative
for ownership, version direction, plans, receipts, and residuals.

## Model and boundaries

| Field          | Contract                                                                                                                                                                                                                                                                                                      |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | Windows users need to install, inspect, and remove the observation integration from the same CLI that launches their terminals.                                                                                                                                                                               |
| Inputs         | Provider ID, optional capsule source, native user profile, PATH, and Codex configuration root are untrusted paths. Empty CODEX_HOME uses the profile default; an invalid nonempty override never falls back.                                                                                                  |
| Data and owner | The provider owns its configuration. Orchbee owns only its versioned capsule and explicitly marked integration entries. Foreign or newer entries remain untouched.                                                                                                                                            |
| Outputs        | Existing integration snapshots, plans, mutation receipts, typed refusals, and retained residual paths. Provider exit alone does not invent an inspected state.                                                                                                                                                |
| Workflow       | Admit platform and budget → prepare the capsule for enable → inspect provider ownership → run the provider vector → report observation or uncertainty. Cancellation stops subsequent effects and terminates the owned command tree.                                                                           |
| Publication    | Stage beside the target; copy regular entries only; validate the capsule; publish using atomic no-replace MoveFile. An occupied destination is a conflict. Backups use the same primitive.                                                                                                                    |
| Paths          | Local absolute drive paths only for managed effects. Reparse points and ambiguous components are refused. Ancestor directory handles deny delete sharing during each filesystem operation.                                                                                                                    |
| Process        | Resolve bounded absolute PATH entries. Native EXE and Windows CMD/BAT shims are admitted; PowerShell scripts are not implicitly executed. Arguments remain structured and Rust's batch-command quoting applies. Each command starts suspended, joins an owned Job Object, and resumes only after containment. |
| Bounds         | Existing 40-second invocation and 3.6-second command slot remain. Execution has a 3-second bound, stdout has a 256 KiB cap, and teardown polls without an unbounded wait.                                                                                                                                     |
| Transitions    | Before admission, cancellation/deadline is a no-effect result. After publication or provider launch, failures retain attempted steps and uncertainty. No retry may replace foreign data.                                                                                                                      |
| Dependencies   | Windows-only process-wrap 10.0.1 supplies safe suspended Job Object assignment missing from std; only std, job-object, and creation-flags are enabled. The matching windows 0.62.2 binding supplies the creation-flag type. Existing local-ipc owns nonblocking child output. No async runtime is added.      |
| Rollback       | Disable uses the existing ownership guards. Failed or interrupted publication reports retained staging/backup paths; the CLI never sweeps unrelated files.                                                                                                                                                    |
| Evidence       | Windows filesystem tests cover no-replace, Unicode paths, and reparse refusal; process tests cover argument preservation, output limits, cancellation, timeout, and descendants; isolated public CLI tests cover Codex onboarding and configuration isolation.                                                |
| Limits         | Provider authentication, remote installs, paid Agent tasks, and TUI interaction are outside onboarding evidence. TLA execution is excluded by user instruction. Review is a same-author adversarial pass, not independent evidence.                                                                           |

Windows manages Codex hooks and the existing OpenCode, Pi and OMP JavaScript modules.
The other provider rows retain `manual-only` until their Windows hook execution path
is supported. Inspection and mutation share the same platform admission policy.

OpenCode uses its ESM entry; the Pi/OMP installed CommonJS bootstrap dynamically imports
the capsule's **ESM** entry and calls its named registration function. Importing their
CommonJS wrapper and looking for that named export would silently skip registration.
This Windows refinement reuses payload 0.4.1 and changes no payload files or non-Windows
runtime behavior. The deterministic bootstrap bytes and file URL remain the ownership
proof: changed, foreign or differently versioned entries are refused without replacement.
Windows doctor derives the capsule root from the full `bundle/<provider>/<entry>` suffix,
so a healthy installed module is inspected against its capsule manifest instead of the
bundle directory. This is an offline file check and does not establish a live connection.

Public CLI witnesses use an isolated Unicode profile to install, inspect and remove all
three modules. Real Node loads each installed bootstrap and reports ordered provider
events through a private Windows pipe; a separate native ConPTY witness checks console
correlation into canonical sessions. These controlled provider APIs establish the
integration boundary, not provider authentication or the behavior of arbitrary releases.

## Dependency review

[process-wrap](https://docs.rs/process-wrap/10.0.1/process_wrap/) is MIT OR Apache-2.0,
has no build script, and adds indexmap 2.14.2 with already-resolved hashbrown 0.17.1.
The Windows binding adds windows-core, collections, future, numerics, result, strings,
threading, and its implement/interface proc macros. The proc macros reuse the existing
syn/quote/proc-macro2 versions. No duplicate package version or async runtime is added. Its Windows unsafe calls remain
inside the dependency; authored Rust keeps `unsafe_code = "forbid"`. The adapter
never calls its blocking wait method. It explicitly terminates the Job on every
exit path, including normal leader completion; dropping the std wrapper alone
does not provide kill-on-drop. Abrupt termination of Orchbee by an uncatchable OS
action remains outside this cooperative teardown claim.
