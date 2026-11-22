# Agent confirmation rules

The Server decides whether a confirmation an Agent shows may be answered for a person, and sends
the key when it may. This reference owns the rules that decision is made of: how a dialog is read
from a screen, which files wait, what one Agent run remembers, and how a decision reaches the
terminal. The rules are the module `agent_confirmation`, which is pure and reads no file and no
clock of its own; `local_terminal_service/agent_confirmation.rs` connects them to the screen
sampler, the settings and the Clients. The
[proposal](../../../proposals/agent/automatic-confirmation.md) owns the design that is not built:
the Agents after Claude Code, Codex, Gemini CLI and OpenCode.

A command is never judged. A command confirmation whose dialog a layout reads completely, with
the one-time accept highlighted, is answered whatever the command says, also when the Agent wraps,
cuts or hides the command inside that dialog: a check of command text cannot be complete, and one
that is not gives false assurance. A command that deletes or uploads files runs like any other.

Every other rule fails closed. A screen or a file path the module cannot read completely is one
that waits for the person, and each wait carries one reason from a closed list: the file is
outside the Workspace, a protected file, a path not understood, a path not fully visible, marked
dangerous by the Agent, not a prompt Orchbee answers, a person typed or clicked, the screen keeps
changing, the same confirmation keeps coming back, and a terminal an older Orchbee started: its
Terminal Host does not list `screen-gated-send`, so the Worker refuses the send before anything
reaches that Host, and the terminal keeps waiting for the person until it runs under a newer
Host.

A release build withholds the feature until it ships: the Server loads no layout, so it reads no
dialog and offers, shows and presses nothing, and its registry does not know the settings below.
Development and test builds answer as this reference describes; see the
[release-withheld surfaces](../../orchbee/README.md#release-withheld-surfaces).

## Layouts

A layout describes one confirmation dialog of one Agent as that Agent draws it. Layouts are TOML
files under `resources/agent-confirmation/`, one per Agent, compiled into the Server. They are
never read from a local override or a downloaded catalog: a rule that can press a key ships only
inside a release. A layout that does not parse or compile is an error when the bundled set is
built, which the module's tests do.

| Field        | Meaning                                                                             |
| ------------ | ----------------------------------------------------------------------------------- |
| `top`        | One pattern per file: a row that can be the dialog's top border or title            |
| `fence`      | Optional, one per file: a row that text a model supplies cannot draw                |
| `only`       | Optional, one per file: the row `top` matches must be the only one on the screen    |
| `name`       | The layout's name inside its Agent; part of a prompt's identity                     |
| `kind`       | `command`, `create` or `edit`                                                       |
| `accept`     | The key that accepts; `enter` is the only value read                                |
| `rows`       | Patterns joined into one expression that must match the whole dialog region         |
| `text`       | Patterns that take the command or path out of each captured row, tried in order     |
| `danger`     | Optional pattern over the Agent's own notes; a match marks the dialog as dangerous  |
| `selected`   | Optional: the capture of the option the accept key chooses, told apart by its style |
| `unselected` | With `selected`: the captures of the other options, two at least                    |

- **Region.** Rows are trimmed of trailing spaces. The region runs from the last row that matches
  `top` to the last row that is not blank, so text scrolled above the dialog is not part of it and
  nothing may follow the dialog.
- **Fence.** An Agent that draws no border and indents the model's text like its own title gives
  that text a way to draw a title. Its file names a `fence`, and then the row `top` matches must
  be the only such row on the screen, with a fence row above it. The fence row is not the
  dialog's, so the dialog starts on the screen and its own title is there; being the only one,
  the matched row is that title. A dialog taller than the screen and a screen with two titles
  are not read.
- **Only.** An Agent whose dialog is laid out from its title down, so that a dialog with any row
  on the screen has its title there, names `only` in place of a fence: the row `top` matches must
  be the only such row on the screen.
- **Selection by style.** An Agent that tells the chosen option only by colour names it with
  `selected` and the other options with `unselected`. The dialog is read only while every visible
  cell of the selected option has one style and every visible cell of the other options shares
  one other style, and only while each styled row is the text row it stands for. With one other
  option, two options drawn apart would not tell which is chosen. Without styled rows such a
  layout reads nothing.
- **Whole match.** `rows` is anchored at both ends of the region. Extra rows, a missing footer,
  the highlight on another row, a pager hint and a truncated dialog all fail to match. The
  highlighted row is written into the pattern as the Agent's one-time accept; no pattern accepts
  a highlighted "always" or "do not ask again" row.
- **Captured text.** `rows` captures the command or path rows as `text` and may capture the
  Agent's own remarks as `notes`. Body rows start in a column the Agent's chrome does not use, so a
  command cannot draw a border or an option row that the pattern would take for the dialog's own.
  The rows a command shows are its text, including a row the Agent writes in place of rows it does
  not show; a dialog that shows none of its command has no text.
- **Bounds.** A screen over 64 KiB, a pattern over 8 KiB and a compiled expression over 1 MiB are
  refused.
- **Identity.** A prompt is its layout, its kind and its captured text. Two screens with the same
  three are the same prompt.

Evidence for a layout is a set of screens captured from the real Agent, kept under
`src/agent_confirmation/tests/screens/` with user names and session identifiers replaced by
neutral text of the same width. A capture for a layout that reads styles keeps the escape
sequences the terminal drew, and its text is what a text read of the same frame gives. Add a
layout with its captures and their expected answers in the same change; a layout without captures
is not admitted.

| Agent       | Layouts                                     | Not read                                                |
| ----------- | ------------------------------------------- | ------------------------------------------------------- |
| Claude Code | Bash command, file create, file edit        | Its trust prompt and its screens after an answer        |
| Codex       | Shell command, under the first-column fence | Every other approval, such as a patch or network access |
| Gemini CLI  | Shell command, whole-file write             | Replace in a file, its trust prompt, every other tool   |
| OpenCode    | Shell command, file edit, by style          | Every other permission, and a dialog its box cuts       |

Codex's reason rows cannot start with the command's `$`, so the first `$` row is the command's,
and the command is every row from there to the two blank rows above the options. Codex writes
files through PowerShell, so such a write is a command confirmation. A row such as
`[… 9 lines] ctrl+a view all` in place of command rows Codex does not show is command text. A
Codex dialog taller than its Panel fills the Panel: no row lies above the title and the options
are shortened behind an arrow, so the fence and the option rows make such a dialog wait.

Gemini CLI puts the command, or a file's new content, in a box inside the dialog's box, so a row a
model supplies starts behind two bars and cannot draw a border, the question or an option. Four
rules come from how it draws:

- A whole-file write creates a file or replaces one through the same dialog, so it is read as an
  edit of the path in the title. Gemini CLI shortens a path beyond 35 characters with three dots;
  a path with an ellipsis or three dots in it is not all on the screen and waits.
- Its replace-in-file dialog is not read. The title joins the path and two pieces of the model's
  text with `: `, which a folder name may contain on macOS, so the path cannot be told from the
  text.
- In a short terminal it draws the inner box empty, or hides rows of it behind a row that starts
  with three dots. The dialog is still whole and is read: the empty box has no text, and the row
  with the dots is command text.
- With several confirmations pending it ends the title row in a counter such as `1 of 2`, and
  Enter answers the dialog shown. The write layout takes the counter apart from the path, and a
  path with a space, which cannot be told from a path and a counter, is not read. A long title
  with the counter takes a second row, which no layout reads.

OpenCode draws its permission dialog as a box at the bottom of the screen with a bar in its third
column, and draws the command, or a file's path and diff, behind the same bar and two spaces as
the title. A line of a command can therefore draw a second title, so the file names `only`. Four
rules come from how it draws:

- Its three options differ only in colour, and a mouse moving over the dialog with no button
  pressed selects the option under it with no change to the text. The layouts read a dialog only
  while "Allow once" is drawn apart from the two others; "Always allow" selected also changes the
  text above the options.
- The box is at most 15 rows tall and cuts a longer command with no mark: after its sixth row
  where the keys have a row of their own, after its eighth where they sit beside the options from
  80 columns on. Such a dialog is read with the rows it shows. In a Panel shorter than the box its
  last rows are cut, and no layout reads it.
- A file written whole and a file changed in place both show as an edit of the path. A path
  outside the Agent's folder is written from the home folder or in full, and a long path continues
  on rows of its own, which the path check does not take as one row.
- From about 120 columns a sidebar may write the folder at the right end of the last row; a
  sidebar's text anywhere else in the dialog fails the match.

## Path rules

A path is placed by its spelling alone; no file system is read.

- **Scope.** A path is judged against the Workspace's directory and the Agent process's working
  directory. Without both, or when the working directory is not inside the Workspace, every file
  confirmation waits.
- **Forms.** Windows drive paths, POSIX paths, and the POSIX spelling of a drive path (`/c/work`)
  are read. Drive paths compare without case; POSIX paths compare exactly. A network or device
  path, a drive-relative path, a stream, a short name, a wildcard, and a step that ends in a dot
  or a space are not read and wait.
- **Padding.** A path with a run of spaces in it is a row's padding taken with the name, and
  waits.
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
the dialog with its verdict; the run answers with nothing, one press, or a hold.
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
  when a program status report already decides the Agent's state. When a capture holds a dialog
  region of an Agent whose layouts read styles, the sampler also reads the visible rows with their
  styles at the revision and generation of the text, under the next row request sequence; a
  refused styled read leaves the decision without a capture for that turn. A turn without a
  capture changes nothing; it can only send a hold that is owed.
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
  `agents.auto_allow.gemini` turns answering on for every Gemini CLI run.
  `agents.auto_allow.opencode` turns answering on for every OpenCode run.
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
