# Agent confirmation rules

The Server decides whether a confirmation an Agent shows may be answered for a person. This
reference owns the rules that decision is made of: how a dialog is read from a screen, which
commands and paths wait, and what one Agent run remembers. The module is `agent_confirmation`; it
is pure, reads no file and no clock of its own, and nothing calls it yet, so no key is sent and
nothing a person can reach has changed. The
[proposal](../../../proposals/agent/automatic-confirmation.md) owns the design that is not built:
the sampler wiring, the settings, the snapshot field, the requests and both Clients.

Every rule fails closed. A screen, a command or a path the module cannot read completely is one
that waits for the person, and each wait carries one reason from a closed list: destructive
command, leaves the Workspace, protected file, command not fully understood, command not fully
visible, marked dangerous by the Agent, not a prompt Orchbee answers, a person typed or clicked,
the screen keeps changing, the same confirmation keeps coming back, and a Terminal Host too old to
check the screen.

## Layouts

A layout describes one confirmation dialog of one Agent as that Agent draws it. Layouts are TOML
files under `resources/agent-confirmation/`, one per Agent, compiled into the Server. They are
never read from a local override or a downloaded catalog: a rule that can press a key ships only
inside a release. A layout that does not parse or compile is an error when the bundled set is
built, which the module's tests do.

| Field    | Meaning                                                                            |
| -------- | ---------------------------------------------------------------------------------- |
| `top`    | One pattern per file: a row that can be the dialog's top border                    |
| `name`   | The layout's name inside its Agent; part of a prompt's identity                    |
| `kind`   | `command`, `create` or `edit`                                                      |
| `accept` | The key that accepts; `enter` is the only value read                               |
| `rows`   | Patterns joined into one expression that must match the whole dialog region        |
| `text`   | Patterns that take the command or path out of each captured row, tried in order    |
| `danger` | Optional pattern over the Agent's own notes; a match marks the dialog as dangerous |

- **Region.** Rows are trimmed of trailing spaces. The region runs from the last row that matches
  `top` to the last row that is not blank, so text scrolled above the dialog is not part of it and
  nothing may follow the dialog.
- **Whole match.** `rows` is anchored at both ends of the region. Extra rows, a missing footer,
  the highlight on another row, a pager hint and a truncated dialog all fail to match. The
  highlighted row is written into the pattern as the Agent's one-time accept; no pattern accepts
  a highlighted "always" or "do not ask again" row.
- **Captured text.** `rows` captures the command or path rows as `text` and may capture the
  Agent's own remarks as `notes`. Body rows start in a column the Agent's chrome does not use, so a
  command cannot draw a border or an option row that the pattern would take for the dialog's own.
- **Bounds.** A screen over 64 KiB, a pattern over 8 KiB, a compiled expression over 1 MiB and
  more than 16 captured rows are refused.
- **Identity.** A prompt is its layout, its kind and its captured text. Two screens with the same
  three are the same prompt.

Evidence for a layout is a set of screens captured from the real Agent, kept under
`src/agent_confirmation/tests/screens/` with user names and session identifiers replaced by
neutral text of the same width. Add a layout with its captures and their expected answers in the
same change; a layout without captures is not admitted.

Claude Code is the only Agent with layouts: its Bash command, file create and file edit dialogs.
Its trust prompt and its screens after an answer match no layout.

## Command check

The check reads text an Agent controls, so it is a trust boundary: no I/O, bounded input, no panic
path. It lists what must wait, never what may run; a command word it does not list is routine. It
never resolves quoting, escapes or expansion, so it never has to agree with a shell about them, and
it reads POSIX shells, PowerShell and `cmd` the same way.

1. **Bounds and characters.** At most 2048 bytes and five line breaks. Only printable ASCII and
   letters or digits of other scripts are read; an ellipsis means the command is not fully visible.
2. **Line breaks.** A dialog wraps long commands, so a break on screen may be a wrap inside a word,
   a wrap at a space, or a new line that starts another command. Every reading of every break is
   checked and the strongest reason found is the answer.
3. **Structure.** The text is split into simple commands at `;`, `&`, `|`, new lines, parentheses
   and braces, wherever they stand, inside quotes too. Command substitution, process substitution,
   a here-document, a trailing line continuation and an unclosed quote wait.
4. **Command word.** It waits when it contains a quote, a caret, a backslash, a backtick, a
   variable, `=` or `~`, when it is not spelled like a program name or a path to one, or when it
   is a keyword that starts a compound statement. The program is its last path step without case
   and without `.exe`, `.cmd`, `.bat` or `.com`. A listed program glued to a switch, as `cmd`
   reads `rd/s/q`, waits as that program.
5. **Families.** `command/families.rs` lists programs with the options and subcommands that make
   them wait, wherever those stand in the command. Before a word is compared, quotes, carets and
   backslashes are removed from it and case is dropped. The families are: shells and interpreters
   given code inline, with `pwsh`, `powershell`, `cmd` and `osascript` waiting in every form;
   words that run another command; delete, overwrite and disk tools; Git commands that discard,
   rewrite or publish, among them every `push`, `checkout` and `restore`; elevation; recursive
   permission and owner changes; process, service, machine, registry and package-manager changes;
   links; remote copies; and the destructive or outward subcommands of container, cluster,
   infrastructure, hosting, cloud and publishing tools. A delete with a pattern such as `*` waits.
6. **Statements and downloads.** `DROP`, `TRUNCATE`, `FLUSHALL`, `FLUSHDB` and `DELETE FROM` wait
   in any command. A download program in a command that also feeds a shell waits.
7. **Variables and places.** An argument with a variable waits: as leaving the Workspace for a home
   form such as `$HOME` or `%USERPROFILE%`, otherwise as not understood. Every argument, the value
   after an `=` and a drive path inside an argument are placed by the path rules below.
   Redirection targets are placed like arguments; a redirection to the null device is the one
   place outside the Workspace a command may name. A word that starts with `/` and is not inside
   the Workspace waits, which also holds Windows switches such as `/s`.

Every spelling the check covers is a row of `src/agent_confirmation/tests/commands.tsv`: the
expected answer, the shell, and the command. Add a spelling as a row first, then make the check
agree.

The check cannot catch, and product copy must not claim otherwise:

- A script, Makefile target, package script or test the Agent wrote or changed and then runs.
- An alias, a shell function, or a program that deletes or overwrites by itself, and a destructive
  command of a tool the families do not list.
- A place named inside a quoted argument, inside an option the check does not split, or reached
  through a link that already exists or through an earlier `cd`.
- An Agent that draws a command other than the one it runs.

## Path rules

A path is placed by its spelling alone; no file system is read.

- **Scope.** A path is judged against the Workspace's directory and the Agent process's working
  directory. Without both, or when the working directory is not inside the Workspace, every
  command and every path waits.
- **Forms.** Windows drive paths, POSIX paths, and the POSIX spelling of a drive path (`/c/work`)
  are read. Drive paths compare without case; POSIX paths compare exactly. A network or device
  path, a drive-relative path, a stream, a short name, a wildcard, and a step that ends in a dot
  or a space are not read and wait.
- **Inside.** A relative path without a `..` step is inside. An absolute path is inside when its
  steps start with the Workspace's. A `..` step, `~`, and a variable wait.
- **Protected names** wait wherever they are, compared without case, without a stream and without
  trailing dots and spaces: `.git`; `.env` and `.env.*`; `.claude`, `.codex`, `.gemini`,
  `.opencode`, `.hermes`, `.mcp.json`, `opencode.json`, `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`;
  `.bashrc`, `.bash_profile`, `.bash_login`, `.profile`, `.zshrc`, `.zprofile`, `.zshenv`,
  `.zlogin`, `config.fish`; and PowerShell profile files.
- A file confirmation carries exactly one path row. The content of the change is not judged.

## Run decision

One Agent run keeps its state in Server memory. It ends with the Agent process and is forgotten by
a Server restart. Each sampler turn gives the run the frame it read, whether the Agent waits, and
the dialog with its answer from the checks; the run answers with nothing, one press, or a hold.
[`AgentConfirmationPress.tla`](../formal/tla/AgentConfirmationPress.tla) checks the order of these
steps against the Terminal Host's gate.

- **On.** The run is on by the person's choice for this run or by the saved setting for its Agent
  type.
- **Settle.** A prompt is pressed only after it has been the same prompt for one second across
  three observations. Another prompt starts its own wait.
- **Press.** The press names the frame read in that turn and the revision at which the prompt was
  first seen, which the [screen-gated key](../../terminal-host/docs/runtime-model.md#screen-gated-input)
  carries to the Host. A prompt is pressed once; a send without an answer counts as written.
- **Refusals.** After a changed screen, pending input, a typed-on revision or a lost process the
  next turn decides again. After a person's input since the prompt appeared, the prompt is the
  person's. After a person's recent input, the run decides again once three seconds have passed.
  A Host without the gate ends pressing for the run.
- **Off.** Turning the run or its type off owes the Host one hold, which records an action at the
  current revision so that a key already on its way is refused. The run shows as on until the hold
  is answered.
- **Repeat stop.** When the same prompt has been pressed three times in a row and appears again,
  the run turns itself off, owes the hold, and says that it stopped. Another prompt answered in
  between starts the count again, and the person may turn the run on again.
- **Offer.** While off, a run offers on a settled prompt it would answer, at most twice, and never
  after the person answered "no" to offers. An offer goes away with its prompt.
