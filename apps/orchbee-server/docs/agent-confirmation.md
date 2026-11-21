# Agent confirmation rules

The Server decides whether a confirmation an Agent shows may be answered for a person, and sends
the key when it may. This reference owns the rules that decision is made of: how a dialog is read
from a screen, which commands and paths wait, what one Agent run remembers, and how a decision
reaches the terminal. The rules are the module `agent_confirmation`, which is pure and reads no
file and no clock of its own; `local_terminal_service/agent_confirmation.rs` connects them to the
screen sampler, the settings and the Clients. The
[proposal](../../../proposals/agent/automatic-confirmation.md) owns the design that is not built:
the Agents after Claude Code and Codex.

Every rule fails closed. A screen, a command or a path the module cannot read completely is one
that waits for the person, and each wait carries one reason from a closed list: destructive
command, leaves the Workspace, protected file, command not fully understood, command not fully
visible, marked dangerous by the Agent, not a prompt Orchbee answers, a person typed or clicked,
the screen keeps changing, the same confirmation keeps coming back, and a terminal an older
Orchbee started: its Terminal Host does not list `screen-gated-send`, so the Worker refuses the
send before anything reaches that Host, and the terminal keeps waiting for the person until it
runs under a newer Host.

## Layouts

A layout describes one confirmation dialog of one Agent as that Agent draws it. Layouts are TOML
files under `resources/agent-confirmation/`, one per Agent, compiled into the Server. They are
never read from a local override or a downloaded catalog: a rule that can press a key ships only
inside a release. A layout that does not parse or compile is an error when the bundled set is
built, which the module's tests do.

| Field    | Meaning                                                                            |
| -------- | ---------------------------------------------------------------------------------- |
| `top`    | One pattern per file: a row that can be the dialog's top border or title           |
| `fence`  | Optional, one per file: a row that text a model supplies cannot draw               |
| `name`   | The layout's name inside its Agent; part of a prompt's identity                    |
| `kind`   | `command`, `create` or `edit`                                                      |
| `accept` | The key that accepts; `enter` is the only value read                               |
| `rows`   | Patterns joined into one expression that must match the whole dialog region        |
| `text`   | Patterns that take the command or path out of each captured row, tried in order    |
| `danger` | Optional pattern over the Agent's own notes; a match marks the dialog as dangerous |

- **Region.** Rows are trimmed of trailing spaces. The region runs from the last row that matches
  `top` to the last row that is not blank, so text scrolled above the dialog is not part of it and
  nothing may follow the dialog.
- **Fence.** An Agent that draws no border and indents the model's text like its own title gives
  that text a way to draw a title. Its file names a `fence`, and then the row `top` matches must
  be the only such row on the screen, with a fence row above it. The fence row is not the
  dialog's, so the dialog starts on the screen and its own title is there; being the only one,
  the matched row is that title. A dialog taller than the screen and a screen with two titles
  are not read.
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

| Agent       | Layouts                                     | Not read                                                |
| ----------- | ------------------------------------------- | ------------------------------------------------------- |
| Claude Code | Bash command, file create, file edit        | Its trust prompt and its screens after an answer        |
| Codex       | Shell command, under the first-column fence | Every other approval, such as a patch or network access |

Codex's reason rows cannot start with the command's `$`, so the first `$` row is the command's,
and the command is every row from there to the two blank rows above the options. Where Codex
writes files through PowerShell the command carries quotes, and the command check holds it.
A Codex dialog taller than its Panel fills the Panel: no row lies above the title, the options
are shortened behind an arrow, and in a short Panel a marker row stands in place of the command
rows that are not shown. The fence, the option rows and the command check's rule for text that
is not all on the screen each make such a dialog wait.

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
  carries to the Host. A prompt is pressed once. A send whose answer leaves open whether the key
  was written counts as written; only an answer that proves the key never reached the Host lets
  the run decide again, three seconds later.
- **Refusals.** After a changed screen, pending input, a typed-on revision or a lost process the
  next turn decides again. After a person's input since the prompt appeared, the prompt is the
  person's. After a person's recent input, the run decides again once three seconds have passed.
  A Host without the gate ends pressing for the run.
- **Off.** Turning the run or its type off owes the Host one hold, which records an action at the
  current revision so that a key already on its way is refused. The hold is asked for again on
  every observation until the Host answers it or cannot type a gated key at all, the run shows as
  on until then, and nothing is pressed while a hold is owed.
- **Repeat stop.** When the same prompt has been pressed three times in a row and appears again,
  the run turns itself off, owes the hold, and says that it stopped. Another prompt answered in
  between starts the count again, and the person may turn the run on again.
- **Offer.** While off, a run offers on a settled prompt it would answer, at most twice, and never
  after the person answered "no" to offers. An offer goes away with its prompt.

## From a decision to the terminal

- **Observation.** The screen sampler brackets one screen capture between two process samples.
  When both samples name the same Agent process, the Session publishes that run for the terminal,
  and the Agent has a layout, the sampler hands the run, the capture and the later sample to the
  decision in the same turn. A terminal whose Agent has a layout is captured on every turn, also
  when a program status report already decides the Agent's state. A turn without a capture changes
  nothing; it can only send a hold that is owed.
- **Scope.** The Workspace's directory is the path of the primary Directory of the Workspace whose
  terminals include this one. The Agent's working directory is the current directory of its
  process in the later sample.
- **Runs.** One run is kept per terminal, for the exact Worker generation, Host terminal and Agent
  process it was observed with. Another process, another Worker or a terminal without a supported
  Agent starts over, which forgets "this Agent only". A run no turn has observed for ten minutes is
  dropped when another run is first seen.
- **Send.** A press is one unmodified Enter under `SendPrecondition::Screen`, and a hold is
  `SendPrecondition::ScreenHold`; the
  [Worker](../../orchbee-worker/docs/worker-send-transaction.md#screen-gated-refinement) carries
  both to the Host's gate. The send is admitted like a Client's send, to the exact physical terminal the
  sampler read: its binding is exposed, started on the current Worker, unfenced and not closing.
  Under the same locks the run is asked once more whether the press may still leave, so a choice
  that went off after the decision sends nothing.
- **Off means off.** A choice or a setting commit that turns answering off sends the owed holds
  before it is answered. The Host refuses every gated key whose prompt was first seen at or before
  the screen a hold arrived on, so once "off" is answered no key decided earlier can be typed. A
  setting changed by another editor of the settings file is read within a second and sends its
  holds then.
- **Settings.** `agents.auto_allow.claude` turns answering on for every Claude Code run.
  `agents.auto_allow.codex` turns answering on for every Codex run.
  `agents.auto_allow.offer` is true until a person answers "no" to an offer. No settings page
  shows it and it is not among the common values Clients read: the Server reads it from the
  settings file and writes it there on a "no".
- **Clients.** A Client that negotiated the `agent-confirmation` capability reads one row per
  Presentation with the Panel Agent action `confirmations`, which a State stream may follow: it is
  evaluated again after every commit and every change of a run's row. `confirmation-choose` sets
  the row to off, this run, or the Agent type, and applies the run and the saved setting together:
  "off" and "run" turn the type's setting off, "agent-type" turns it on and clears the run's own
  choice. `confirmation-decline` saves the "no". Both need the grant that navigation changes need,
  as a settings edit does. The [Workbench schemas](../../../schemas/workbench/README.md) own the
  shapes.
- **Diagnostics.** One line `orchbee-server diagnostic component=agent-confirmation outcome=<kind>`
  is written for a press (`pressed`), a press without a usable answer (`press-unknown`), a way to
  the Host without the gate (`gate-unsupported`), a prompt a person took (`refused-person`), and
  layouts that failed to load (`no-layouts`). No line carries command text.
