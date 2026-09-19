use std::ffi::OsString;

use orchbee_workbench_protocol::{
    WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchPanelSplitDirection,
    WorkbenchTabMoveDirection, WorkbenchTerminalGeometry,
};

/// Legacy three-variant invocation consumed by `presentation::present`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Invocation {
    Help,
    Tui,
    Version,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UsageFailure;

const MAXIMUM_ARGUMENT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

/// A `get`/filter target validated at parse time. `Id` carries the canonical
/// 32-lowercase-hex text; `Label` carries an exact case-sensitive label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    Id(String),
    Label(String),
}

impl Target {
    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Id(value) | Self::Label(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Resource {
    Project,
    Workspace,
    Tab,
    Panel,
    Terminal,
    Schema,
}

impl Resource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Workspace => "workspace",
            Self::Tab => "tab",
            Self::Panel => "panel",
            Self::Terminal => "terminal",
            Self::Schema => "schema",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HelpTopic {
    Family(Family),
    Command(CommandPath),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Family {
    Project,
    Workspace,
    Tab,
    Panel,
    Terminal,
    Api,
    Config,
    Completion,
}

impl Family {
    /// The family's real help-path spelling, as `help_topic` recognizes it.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Workspace => "workspace",
            Self::Tab => "tab",
            Self::Panel => "panel",
            Self::Terminal => "terminal",
            Self::Api => "api",
            Self::Config => "config",
            Self::Completion => "completion",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandPath {
    Tui,
    Status,
    ProjectList,
    ProjectGet,
    ProjectCreate,
    WorkspaceList,
    WorkspaceGet,
    WorkspaceCreate,
    WorkspaceRename,
    WorkspaceAcceptDirectory,
    TabList,
    TabGet,
    TabCreate,
    TabRename,
    TabMove,
    TabClose,
    PanelList,
    PanelGet,
    PanelSplit,
    PanelClose,
    PanelMove,
    TerminalList,
    TerminalGet,
    TerminalRead,
    TerminalWaitOutput,
    TerminalSend,

    ApiSnapshot,
    ApiSchema,
    ConfigCheck,
}

impl CommandPath {
    /// The command's real help-path spelling, as `help_topic` recognizes it.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Tui => "tui",
            Self::Status => "status",
            Self::ProjectList => "project list",
            Self::ProjectGet => "project get",
            Self::ProjectCreate => "project create",
            Self::WorkspaceList => "workspace list",
            Self::WorkspaceGet => "workspace get",
            Self::WorkspaceCreate => "workspace create",
            Self::WorkspaceRename => "workspace rename",
            Self::WorkspaceAcceptDirectory => "workspace accept-directory",
            Self::TabList => "tab list",
            Self::TabGet => "tab get",
            Self::TabCreate => "tab create",
            Self::TabRename => "tab rename",
            Self::TabMove => "tab move",
            Self::TabClose => "tab close",
            Self::PanelList => "panel list",
            Self::PanelGet => "panel get",
            Self::PanelSplit => "panel split",
            Self::PanelClose => "panel close",
            Self::PanelMove => "panel move",
            Self::TerminalList => "terminal list",
            Self::TerminalGet => "terminal get",
            Self::TerminalRead => "terminal read",
            Self::TerminalWaitOutput => "terminal wait-output",
            Self::TerminalSend => "terminal send",

            Self::ApiSnapshot => "api snapshot",
            Self::ApiSchema => "api schema",
            Self::ConfigCheck => "config check",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum QueryCommand {
    Status,
    ProjectList,
    ProjectGet(Target),
    WorkspaceList {
        project: Option<Target>,
    },
    WorkspaceGet(Target),
    TabList {
        workspace: Option<Target>,
        standalone: bool,
    },
    TabGet(Target),
    PanelList {
        tab: Option<Target>,
    },
    PanelGet(Target),
    TerminalList,
    TerminalGet(Target),
    TerminalRead {
        target: Target,
        /// `Some(n)` selects the `--history --rows n` form: retained
        /// scrollback rows instead of the visible frame.
        history: Option<u16>,
    },
    TerminalWaitOutput {
        target: Target,
        /// The literal, case-sensitive text the wait searches for.
        match_text: String,
        /// The searched retained-row window (visible + history as one
        /// range), 1..=1000 — `--rows` defaults to 1000.
        rows: u16,
        /// Total wait budget in milliseconds from command start, 1..=3600000.
        timeout_ms: u32,
    },
    ApiSnapshot,
    ApiSchema {
        schema_id: Option<String>,
    },
    ConfigCheck,
}

/// The `tab create` context selector: exactly one Workspace context or the
/// standalone Terminal Context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TabContext {
    Workspace(Target),
    Standalone,
}

/// The `terminal send` payload the grammar admits: literal bytes or one named
/// key — the parser stores the validated choice so the executor never
/// re-reads argv.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalSendInput {
    /// Literal UTF-8 bytes; `enter` folds one trailing CR into the same
    /// single input record, never a second request.
    Text { text: String, enter: bool },
    /// One semantic key the Host encodes under the terminal's current mode.
    Key(TerminalSendKey),
}

/// The closed `terminal send --key` vocabulary. `CtrlC`/`CtrlD` carry the
/// Control modifier on their letter; every other key is modifier-neutral.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalSendKey {
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    CtrlC,
    CtrlD,
}

impl TerminalSendKey {
    /// The flag's public spelling, reused for receipt correlation.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Escape => "escape",
            Self::Tab => "tab",
            Self::Backspace => "backspace",
            Self::Delete => "delete",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::Home => "home",
            Self::End => "end",
            Self::PageUp => "page-up",
            Self::PageDown => "page-down",
            Self::CtrlC => "ctrl-c",
            Self::CtrlD => "ctrl-d",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "enter" => Self::Enter,
            "escape" => Self::Escape,
            "tab" => Self::Tab,
            "backspace" => Self::Backspace,
            "delete" => Self::Delete,
            "up" => Self::Up,
            "down" => Self::Down,
            "left" => Self::Left,
            "right" => Self::Right,
            "home" => Self::Home,
            "end" => Self::End,
            "page-up" => Self::PageUp,
            "page-down" => Self::PageDown,
            "ctrl-c" => Self::CtrlC,
            "ctrl-d" => Self::CtrlD,
            _ => return None,
        })
    }
}

/// One effectful command admitted by the grammar. Every field is a
/// wire-ready value already checked against protocol bounds, so the executor
/// never re-validates argv.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MutationCommand {
    ProjectCreate {
        label: WorkbenchLabel,
    },
    WorkspaceCreate {
        project: Target,
        label: WorkbenchLabel,
    },
    WorkspaceRename {
        target: Target,
        label: WorkbenchLabel,
    },
    WorkspaceAcceptDirectory {
        target: Target,
        path: WorkbenchDirectoryPath,
    },
    TabCreate {
        context: TabContext,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
    },
    TabRename {
        target: Target,
        label: WorkbenchLabel,
    },
    TabMove {
        target: Target,
        direction: WorkbenchTabMoveDirection,
    },
    TabClose {
        target: Target,
    },
    PanelSplit {
        target: Target,
        direction: WorkbenchPanelSplitDirection,
        label: WorkbenchLabel,
        geometry: WorkbenchTerminalGeometry,
    },
    PanelClose {
        target: Target,
    },
    PanelMove {
        target: Target,
        into: Target,
        direction: WorkbenchPanelSplitDirection,
    },
    TerminalSend {
        target: Target,
        input: TerminalSendInput,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Request {
    /// `mouse` is the `--mouse off|on` selection: whether host pointer
    /// reporting is armed for the session. Only the explicit `tui` path
    /// admits the option; a bare `orchbee` always runs with it on.
    Tui {
        mouse: bool,
    },
    Version,
    RootHelp,
    Help(HelpTopic),
    Completion(CompletionShell),
    Query {
        command: QueryCommand,
        format: OutputFormat,
    },
    Mutation {
        command: MutationCommand,
        format: OutputFormat,
    },
}

/// How a parse failure is allowed to answer on stdout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureEnvelope {
    /// argv envelope or format-selection failure: stdout stays empty.
    Silent,
    /// A unique valid `--format json` selection preceded this failure.
    Json,
    /// No valid json selection: stdout stays empty.
    Text,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ParseFailureKind {
    InvalidUsage,
    InvalidIdentifier { resource: Resource, target: String },
}

/// The invocation context a usage diagnostic is anchored to. `Path` carries a
/// real help path ("project list"), so its hint points at `orchbee <path>
/// --help`; `Literal` carries display text with no such spelling
/// ("orchbee --version"), so its hint falls back to the root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UsageContext {
    Root,
    Path(&'static str),
    Literal(&'static str),
}

/// The bounded vocabulary of value shapes an `invalid-usage` diagnostic may
/// expect; rendered per locale, never serialized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExpectedValue {
    Format,
    Label,
    Dimension,
    TabDirection,
    PanelDirection,
    Shell,
    HistoryRows,
    DirectoryPath,
    MatchText,
    WaitTimeout,
    SendText,
    SendKey,
    MouseMode,
}

/// Why an `invalid-usage` failure was reached. Carried for the localized
/// human stderr diagnostic only — never serialized into the machine envelope
/// and never a second parse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UsageReason {
    /// Reserved fallback for a real failure without a more precise
    /// classification; every current exit names its concrete reason.
    #[expect(
        dead_code,
        reason = "fallback kept for exits that cannot honestly name a cause"
    )]
    Invalid,
    /// Non-Unicode argv or argv over the total byte bound; nothing echoes.
    ArgumentEnvelope,
    UnknownCommand {
        context: UsageContext,
        word: String,
    },
    UnknownOption {
        option: String,
    },
    UnexpectedOption {
        context: UsageContext,
        option: &'static str,
    },
    DuplicateOption {
        option: &'static str,
    },
    ConflictingOptions {
        context: UsageContext,
        first: &'static str,
        second: &'static str,
    },
    MissingValue {
        option: &'static str,
    },
    /// `requirement` is the display-ready required form: `'--label'` or
    /// `<tab>` — flag spellings and operand placeholders are locale-neutral.
    MissingRequired {
        context: UsageContext,
        requirement: &'static str,
    },
    /// `tab create` with neither `--workspace` nor `--standalone`.
    MissingTabContext {
        context: UsageContext,
    },
    UnexpectedArgument {
        context: UsageContext,
        value: String,
    },
    MissingCommand {
        context: UsageContext,
    },
    UnknownHelpTopic {
        path: String,
    },
    MisplacedFlag {
        context: UsageContext,
        flag: &'static str,
    },
    InvalidValue {
        context: UsageContext,
        option: &'static str,
        value: String,
        expected: ExpectedValue,
    },
    InvalidGeometry {
        context: UsageContext,
        cols: u16,
        rows: u16,
        cell_width: u16,
        cell_height: u16,
    },
}

#[derive(Clone, Debug, Eq)]
pub(crate) struct ParseFailure {
    pub(crate) kind: ParseFailureKind,
    pub(crate) envelope: FailureEnvelope,
    /// The stderr-only usage classification for `InvalidUsage` failures;
    /// `None` for other kinds and equality ignores it — the machine contract
    /// is `kind` plus `envelope`.
    pub(crate) usage: Option<UsageReason>,
}

impl PartialEq for ParseFailure {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.envelope == other.envelope
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Word(String),
    HelpFlag(&'static str),
    VersionFlag,
}

#[derive(Default)]
struct Options {
    format: Option<OutputFormat>,
    project: Option<String>,
    workspace: Option<String>,
    tab: Option<String>,
    standalone: bool,
    label: Option<String>,
    direction: Option<String>,
    cols: Option<String>,
    rows: Option<String>,
    cell_width: Option<String>,
    cell_height: Option<String>,
    confirm: bool,
    target_panel: Option<String>,
    history: bool,
    path: Option<String>,
    match_text: Option<String>,
    timeout: Option<String>,
    text: Option<String>,
    enter: bool,
    key: Option<String>,
    mouse: Option<bool>,
}

/// Every named option that owns the next argv token as its value, used by
/// both the format-envelope walk and the option scan so ownership is decided
/// identically in the two passes.
const VALUE_FLAGS: &[&str] = &[
    "--cell-height",
    "--cell-width",
    "--cols",
    "--direction",
    "--key",
    "--label",
    "--path",
    "--match",
    "--mouse",
    "--project",
    "--rows",
    "--tab",
    "--target-panel",
    "--timeout",
    "--text",
    "--workspace",
];

const MAXIMUM_LABEL_SCALARS: usize = 512;
const ZERO_IDENTITY: &str = "00000000000000000000000000000000";

fn valid_typed_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_label(value: &str) -> bool {
    let scalars = value.chars().count();
    (1..=MAXIMUM_LABEL_SCALARS).contains(&scalars)
}

fn target(
    value: String,
    resource: Resource,
    envelope: FailureEnvelope,
) -> Result<Target, ParseFailure> {
    let identifier = |target: String| ParseFailure {
        kind: ParseFailureKind::InvalidIdentifier { resource, target },
        envelope,
        usage: None,
    };
    if valid_typed_id(&value) {
        if value == ZERO_IDENTITY {
            return Err(identifier(value));
        }
        return Ok(Target::Id(value));
    }
    match resource {
        Resource::Panel | Resource::Terminal => Err(identifier(value)),
        _ if valid_label(&value) => Ok(Target::Label(value)),
        _ => Err(identifier(value)),
    }
}

/// An `invalid-usage` failure carrying the concrete reason the original exit
/// reached; the reason rides the same envelope untouched.
fn usage_with(reason: UsageReason, envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope,
        usage: Some(reason),
    }
}

fn identifier(resource: Resource, target: String, envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidIdentifier { resource, target },
        envelope,
        usage: None,
    }
}

fn selection_envelope(format: Option<OutputFormat>) -> FailureEnvelope {
    match format {
        Some(OutputFormat::Json) => FailureEnvelope::Json,
        Some(OutputFormat::Text) | None => FailureEnvelope::Text,
    }
}

/// Parse the full argv tail into a typed request. The argv envelope (UTF-8 and
/// total byte budget) is validated before any option interpretation.
pub(crate) fn parse_request(
    arguments: impl Iterator<Item = OsString>,
) -> Result<Request, ParseFailure> {
    let mut tokens = Vec::new();
    let mut total_bytes = 0_usize;
    for argument in arguments {
        let Some(text) = argument.to_str() else {
            return Err(usage_with(
                UsageReason::ArgumentEnvelope,
                FailureEnvelope::Silent,
            ));
        };
        total_bytes += text.len();
        if total_bytes > MAXIMUM_ARGUMENT_BYTES {
            return Err(usage_with(
                UsageReason::ArgumentEnvelope,
                FailureEnvelope::Silent,
            ));
        }
        tokens.push(text.to_owned());
    }
    scan(tokens)
}

/// Phase A: scan the complete pre-`--` format envelope before any grammar
/// work. A missing, unknown, empty, or repeated selection anywhere in the
/// envelope is a silent format failure, and one valid unique selection shapes
/// every later grammar failure regardless of its position. Filter flags own
/// the next token even when it resembles a selector, so this walk applies the
/// same left-to-right ownership as the option scan below; a bare `--` is never
/// consumed as a value.
fn format_envelope(tokens: &[String]) -> Result<Option<OutputFormat>, ParseFailure> {
    let missing = || {
        usage_with(
            UsageReason::MissingValue { option: "--format" },
            FailureEnvelope::Silent,
        )
    };
    let invalid = |value: &str| {
        usage_with(
            UsageReason::InvalidValue {
                context: UsageContext::Root,
                option: "--format",
                value: value.to_owned(),
                expected: ExpectedValue::Format,
            },
            FailureEnvelope::Silent,
        )
    };
    let duplicate = || {
        usage_with(
            UsageReason::DuplicateOption { option: "--format" },
            FailureEnvelope::Silent,
        )
    };
    let mut format = None;
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].as_str();
        match token {
            "--" => break,
            "--format" => {
                let Some(value) = tokens.get(index + 1).filter(|value| value.as_str() != "--")
                else {
                    return Err(missing());
                };
                let selected = match value.as_str() {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    _ => return Err(invalid(value)),
                };
                if format.replace(selected).is_some() {
                    return Err(duplicate());
                }
                index += 2;
            }
            _ if token.starts_with("--format=") => {
                let selected = match &token["--format=".len()..] {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    _ => return Err(invalid(&token["--format=".len()..])),
                };
                if format.replace(selected).is_some() {
                    return Err(duplicate());
                }
                index += 1;
            }
            _ if VALUE_FLAGS.contains(&token) => {
                index += match tokens.get(index + 1) {
                    Some(value) if value != "--" => 2,
                    _ => 1,
                };
            }
            _ => index += 1,
        }
    }
    Ok(format)
}

fn scan(tokens: Vec<String>) -> Result<Request, ParseFailure> {
    let mut options = Options {
        format: format_envelope(&tokens)?,
        ..Options::default()
    };
    let envelope = selection_envelope(options.format);
    let mut positionals: Vec<Token> = Vec::new();
    let mut ended = false;
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if ended {
            positionals.push(Token::Word(token.clone()));
            index += 1;
            continue;
        }
        match token.as_str() {
            "--" => {
                ended = true;
                index += 1;
            }
            // The envelope phase already proved every selector is valid and
            // unique; the option walk only applies token ownership.
            "--format" => index += 2,
            _ if token.starts_with("--format=") => index += 1,
            // `--mouse` is a TUI-scoped on/off switch; it parses eagerly so a
            // malformed value fails here rather than at command resolution.
            // The accepted form is the two-token `--mouse off|on` only — an
            // equals form is not part of the grammar and falls through to the
            // unknown-option rejection like any other `-*` token.
            "--mouse" => {
                let value = match tokens.get(index + 1) {
                    Some(value) if value != "--" => value.as_str(),
                    _ => {
                        return Err(usage_with(
                            UsageReason::MissingValue { option: "--mouse" },
                            envelope,
                        ));
                    }
                };
                let Some(selected) = mouse_option(value) else {
                    return Err(usage_with(
                        UsageReason::InvalidValue {
                            context: UsageContext::Root,
                            option: "--mouse",
                            value: value.to_owned(),
                            expected: ExpectedValue::MouseMode,
                        },
                        envelope,
                    ));
                };
                if options.mouse.replace(selected).is_some() {
                    return Err(usage_with(
                        UsageReason::DuplicateOption { option: "--mouse" },
                        envelope,
                    ));
                }
                index += 2;
            }
            _ if VALUE_FLAGS.contains(&token.as_str()) => {
                // The guard admitted this spelling, so the static name exists.
                let option = VALUE_FLAGS
                    .iter()
                    .copied()
                    .find(|flag| *flag == token.as_str())
                    .expect("flag admitted by guard");
                let value = match tokens.get(index + 1) {
                    Some(value) if value != "--" => value.clone(),
                    _ => {
                        return Err(usage_with(UsageReason::MissingValue { option }, envelope));
                    }
                };
                let slot = match token.as_str() {
                    "--project" => &mut options.project,
                    "--workspace" => &mut options.workspace,
                    "--tab" => &mut options.tab,
                    "--label" => &mut options.label,
                    "--direction" => &mut options.direction,
                    "--cols" => &mut options.cols,
                    "--rows" => &mut options.rows,
                    "--cell-width" => &mut options.cell_width,
                    "--cell-height" => &mut options.cell_height,
                    "--path" => &mut options.path,
                    "--match" => &mut options.match_text,
                    "--timeout" => &mut options.timeout,
                    "--text" => &mut options.text,
                    "--key" => &mut options.key,

                    _ => &mut options.target_panel,
                };
                if slot.replace(value).is_some() {
                    return Err(usage_with(
                        UsageReason::DuplicateOption { option },
                        envelope,
                    ));
                }
                index += 2;
            }
            "--standalone" | "--confirm" | "--history" | "--enter" => {
                let (seen, option) = if token == "--standalone" {
                    (&mut options.standalone, "--standalone")
                } else if token == "--confirm" {
                    (&mut options.confirm, "--confirm")
                } else if token == "--history" {
                    (&mut options.history, "--history")
                } else {
                    (&mut options.enter, "--enter")
                };
                if *seen {
                    return Err(usage_with(
                        UsageReason::DuplicateOption { option },
                        envelope,
                    ));
                }
                *seen = true;
                index += 1;
            }
            "--help" | "-h" => {
                positionals.push(Token::HelpFlag(if token == "--help" {
                    "--help"
                } else {
                    "-h"
                }));
                index += 1;
            }
            "--version" => {
                positionals.push(Token::VersionFlag);
                index += 1;
            }
            _ if token.starts_with('-') && token.len() > 1 => {
                return Err(usage_with(
                    UsageReason::UnknownOption {
                        option: token.clone(),
                    },
                    envelope,
                ));
            }
            _ => {
                positionals.push(Token::Word(token.clone()));
                index += 1;
            }
        }
    }
    resolve(positionals, options)
}

fn resolve(positionals: Vec<Token>, options: Options) -> Result<Request, ParseFailure> {
    let envelope = selection_envelope(options.format);
    // `--version` is legal only as the sole root invocation.
    if positionals.len() == 1 && matches!(positionals[0], Token::VersionFlag) {
        reject_extras(
            &options,
            UsageContext::Literal("orchbee --version"),
            envelope,
        )?;
        return Ok(Request::Version);
    }
    // A trailing -h/--help applies to a recognized help path only.
    let (path, suffix_help) = match positionals.last() {
        Some(Token::HelpFlag(_)) => (&positionals[..positionals.len() - 1], true),
        _ => (&positionals[..], false),
    };
    if suffix_help {
        return resolve_help_suffix(path, &options, envelope);
    }
    if let Some(flag) = path.iter().find_map(flag_literal) {
        let words: Vec<&str> = path.iter().filter_map(word_literal).collect();
        return Err(usage_with(
            UsageReason::MisplacedFlag {
                context: help_context(&words),
                flag,
            },
            envelope,
        ));
    }
    let words: Vec<&str> = path
        .iter()
        .map(|token| match token {
            Token::Word(word) => word.as_str(),
            Token::HelpFlag(_) | Token::VersionFlag => unreachable!("flags excluded above"),
        })
        .collect();
    resolve_words(&words, &options, envelope)
}

fn flag_literal(token: &Token) -> Option<&'static str> {
    match token {
        Token::HelpFlag(literal) => Some(literal),
        Token::VersionFlag => Some("--version"),
        Token::Word(_) => None,
    }
}

fn word_literal(token: &Token) -> Option<&str> {
    match token {
        Token::Word(word) => Some(word.as_str()),
        Token::HelpFlag(_) | Token::VersionFlag => None,
    }
}

/// The help-path context of already-recognized words; unrecognized paths fall
/// back to the root so a hint never points at a nonexistent topic.
fn help_context(words: &[&str]) -> UsageContext {
    match help_topic(words) {
        Some(HelpTopic::Family(family)) => UsageContext::Path(family.as_str()),
        Some(HelpTopic::Command(command)) => UsageContext::Path(command.as_str()),
        None => UsageContext::Root,
    }
}

fn resolve_help_suffix(
    tokens: &[Token],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let words: Vec<&str> = tokens.iter().filter_map(word_literal).collect();
    let context = help_context(&words);
    // -h/--help must be the sole suffix of a recognized help path; other flag
    // tokens, a selected format, filters, or business arguments make it usage.
    reject_extras(options, context, envelope)?;
    if let Some(flag) = tokens.iter().find_map(flag_literal) {
        return Err(usage_with(
            UsageReason::MisplacedFlag { context, flag },
            envelope,
        ));
    }
    if words.is_empty() {
        return Ok(Request::RootHelp);
    }
    help_topic(&words).map(Request::Help).ok_or_else(|| {
        usage_with(
            UsageReason::UnknownHelpTopic {
                path: words.join(" "),
            },
            envelope,
        )
    })
}

fn resolve_words(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    match words {
        [] => {
            // Bare invocation keeps TUI; a selected format or any option
            // (including `--mouse`) is usage — the flag exists only on the
            // explicit `tui` path.
            if options.format.is_some()
                || options.project.is_some()
                || options.workspace.is_some()
                || options.tab.is_some()
                || options.standalone
                || options.mouse.is_some()
                || business_options(options)
            {
                return Err(missing_command(UsageContext::Root, envelope));
            }
            Ok(Request::Tui { mouse: true })
        }
        ["tui"] => reject_extras_scope(options, TUI_SCOPE, UsageContext::Path("tui"), envelope)
            .map(|()| Request::Tui {
                mouse: options.mouse.unwrap_or(true),
            }),
        ["help", rest @ ..] => resolve_help_path(rest, options, envelope),
        ["status"] => {
            reject_filters(options, UsageContext::Path("status"), envelope)?;
            Ok(query(QueryCommand::Status, options))
        }
        ["project", "list"] => {
            reject_filters(options, UsageContext::Path("project list"), envelope)?;
            Ok(query(QueryCommand::ProjectList, options))
        }
        ["project", "get", value] => {
            reject_filters(options, UsageContext::Path("project get"), envelope)?;
            Ok(query(
                QueryCommand::ProjectGet(target((*value).to_owned(), Resource::Project, envelope)?),
                options,
            ))
        }
        ["project", "create"] => {
            check_scope(
                options,
                LABEL_SCOPE,
                UsageContext::Path("project create"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::ProjectCreate {
                    label: label_option(options, UsageContext::Path("project create"), envelope)?,
                },
                options,
            ))
        }
        ["workspace", "list"] => {
            check_scope(
                options,
                WORKSPACE_LIST_SCOPE,
                UsageContext::Path("workspace list"),
                envelope,
            )?;
            let project = options
                .project
                .clone()
                .map(|value| target(value, Resource::Project, envelope))
                .transpose()?;
            Ok(query(QueryCommand::WorkspaceList { project }, options))
        }
        ["workspace", "get", value] => {
            reject_filters(options, UsageContext::Path("workspace get"), envelope)?;
            Ok(query(
                QueryCommand::WorkspaceGet(target(
                    (*value).to_owned(),
                    Resource::Workspace,
                    envelope,
                )?),
                options,
            ))
        }
        ["workspace", "create"] => {
            check_scope(
                options,
                WORKSPACE_CREATE_SCOPE,
                UsageContext::Path("workspace create"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::WorkspaceCreate {
                    project: target(
                        required(
                            &options.project,
                            UsageContext::Path("workspace create"),
                            "'--project'",
                            envelope,
                        )?,
                        Resource::Project,
                        envelope,
                    )?,
                    label: label_option(options, UsageContext::Path("workspace create"), envelope)?,
                },
                options,
            ))
        }
        ["workspace", "rename", value] => {
            check_scope(
                options,
                LABEL_SCOPE,
                UsageContext::Path("workspace rename"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::WorkspaceRename {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    label: label_option(options, UsageContext::Path("workspace rename"), envelope)?,
                },
                options,
            ))
        }
        ["workspace", "accept-directory", value] => {
            check_scope(
                options,
                ACCEPT_DIRECTORY_SCOPE,
                UsageContext::Path("workspace accept-directory"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::WorkspaceAcceptDirectory {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    path: path_option(
                        options,
                        UsageContext::Path("workspace accept-directory"),
                        envelope,
                    )?,
                },
                options,
            ))
        }
        ["tab", "list"] => {
            check_scope(
                options,
                TAB_LIST_SCOPE,
                UsageContext::Path("tab list"),
                envelope,
            )?;
            reject_conflict(
                options.standalone && options.workspace.is_some(),
                UsageContext::Path("tab list"),
                envelope,
            )?;
            let workspace = options
                .workspace
                .clone()
                .map(|value| target(value, Resource::Workspace, envelope))
                .transpose()?;
            Ok(query(
                QueryCommand::TabList {
                    workspace,
                    standalone: options.standalone,
                },
                options,
            ))
        }
        ["tab", "get", value] => {
            reject_filters(options, UsageContext::Path("tab get"), envelope)?;
            Ok(query(
                QueryCommand::TabGet(target((*value).to_owned(), Resource::Tab, envelope)?),
                options,
            ))
        }
        ["tab", "create"] => {
            check_scope(
                options,
                TAB_CREATE_SCOPE,
                UsageContext::Path("tab create"),
                envelope,
            )?;
            reject_conflict(
                options.standalone && options.workspace.is_some(),
                UsageContext::Path("tab create"),
                envelope,
            )?;
            let context = if options.standalone {
                TabContext::Standalone
            } else {
                let Some(value) = options.workspace.clone() else {
                    return Err(usage_with(
                        UsageReason::MissingTabContext {
                            context: UsageContext::Path("tab create"),
                        },
                        envelope,
                    ));
                };
                TabContext::Workspace(target(value, Resource::Workspace, envelope)?)
            };
            Ok(mutation(
                MutationCommand::TabCreate {
                    context,
                    label: label_option(options, UsageContext::Path("tab create"), envelope)?,
                    geometry: geometry_option(options, UsageContext::Path("tab create"), envelope)?,
                },
                options,
            ))
        }
        ["tab", "rename", value] => {
            check_scope(
                options,
                LABEL_SCOPE,
                UsageContext::Path("tab rename"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::TabRename {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                    label: label_option(options, UsageContext::Path("tab rename"), envelope)?,
                },
                options,
            ))
        }
        ["tab", "move", value] => {
            check_scope(
                options,
                TAB_MOVE_SCOPE,
                UsageContext::Path("tab move"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::TabMove {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                    direction: tab_direction(options, UsageContext::Path("tab move"), envelope)?,
                },
                options,
            ))
        }
        ["tab", "close", value] => {
            check_scope(
                options,
                CONFIRM_SCOPE,
                UsageContext::Path("tab close"),
                envelope,
            )?;
            if !options.confirm {
                return Err(missing_required(
                    UsageContext::Path("tab close"),
                    "'--confirm'",
                    envelope,
                ));
            }
            Ok(mutation(
                MutationCommand::TabClose {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                },
                options,
            ))
        }
        ["panel", "list"] => {
            check_scope(
                options,
                PANEL_LIST_SCOPE,
                UsageContext::Path("panel list"),
                envelope,
            )?;
            let tab = options
                .tab
                .clone()
                .map(|value| target(value, Resource::Tab, envelope))
                .transpose()?;
            Ok(query(QueryCommand::PanelList { tab }, options))
        }
        ["panel", "get", value] => {
            reject_filters(options, UsageContext::Path("panel get"), envelope)?;
            Ok(query(
                QueryCommand::PanelGet(target((*value).to_owned(), Resource::Panel, envelope)?),
                options,
            ))
        }
        ["panel", "split", value] => {
            check_scope(
                options,
                PANEL_SPLIT_SCOPE,
                UsageContext::Path("panel split"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::PanelSplit {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                    direction: panel_direction(
                        options,
                        UsageContext::Path("panel split"),
                        envelope,
                    )?,
                    label: label_option(options, UsageContext::Path("panel split"), envelope)?,
                    geometry: geometry_option(
                        options,
                        UsageContext::Path("panel split"),
                        envelope,
                    )?,
                },
                options,
            ))
        }
        ["panel", "close", value] => {
            check_scope(
                options,
                CONFIRM_SCOPE,
                UsageContext::Path("panel close"),
                envelope,
            )?;
            if !options.confirm {
                return Err(missing_required(
                    UsageContext::Path("panel close"),
                    "'--confirm'",
                    envelope,
                ));
            }
            Ok(mutation(
                MutationCommand::PanelClose {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                },
                options,
            ))
        }
        ["panel", "move", value] => {
            check_scope(
                options,
                PANEL_MOVE_SCOPE,
                UsageContext::Path("panel move"),
                envelope,
            )?;
            Ok(mutation(
                MutationCommand::PanelMove {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                    into: target(
                        required(
                            &options.target_panel,
                            UsageContext::Path("panel move"),
                            "'--target-panel'",
                            envelope,
                        )?,
                        Resource::Panel,
                        envelope,
                    )?,
                    direction: panel_direction(
                        options,
                        UsageContext::Path("panel move"),
                        envelope,
                    )?,
                },
                options,
            ))
        }
        ["terminal", "list"] => {
            reject_filters(options, UsageContext::Path("terminal list"), envelope)?;
            Ok(query(QueryCommand::TerminalList, options))
        }
        ["terminal", "get", value] => {
            reject_filters(options, UsageContext::Path("terminal get"), envelope)?;
            Ok(query(
                QueryCommand::TerminalGet(target(
                    (*value).to_owned(),
                    Resource::Terminal,
                    envelope,
                )?),
                options,
            ))
        }
        ["terminal", "read", value] => {
            check_scope(
                options,
                TERMINAL_READ_SCOPE,
                UsageContext::Path("terminal read"),
                envelope,
            )?;
            let context = UsageContext::Path("terminal read");
            // `--history` and `--rows` are admitted as a pair only: each
            // without the other is a missing-requirement, never a default.
            let history = match (options.history, options.rows.is_some()) {
                (true, true) => Some(history_rows_option(options, context, envelope)?),
                (true, false) => {
                    return Err(missing_required(context, "'--rows'", envelope));
                }
                (false, true) => {
                    return Err(missing_required(context, "'--history'", envelope));
                }
                (false, false) => None,
            };
            Ok(query(
                QueryCommand::TerminalRead {
                    target: target((*value).to_owned(), Resource::Terminal, envelope)?,
                    history,
                },
                options,
            ))
        }
        ["terminal", "wait-output", value] => {
            check_scope(
                options,
                TERMINAL_WAIT_OUTPUT_SCOPE,
                UsageContext::Path("terminal wait-output"),
                envelope,
            )?;
            let context = UsageContext::Path("terminal wait-output");
            // `--match` is required; `--rows` and `--timeout` carry their
            // documented defaults. Everything resolves before any cohort,
            // socket, session, viewer, or terminal effect.
            Ok(query(
                QueryCommand::TerminalWaitOutput {
                    target: target((*value).to_owned(), Resource::Terminal, envelope)?,
                    match_text: wait_match_option(options, context, envelope)?,
                    rows: wait_rows_option(options, context, envelope)?,
                    timeout_ms: wait_timeout_option(options, context, envelope)?,
                },
                options,
            ))
        }
        ["terminal", "send", value] => {
            check_scope(
                options,
                TERMINAL_SEND_SCOPE,
                UsageContext::Path("terminal send"),
                envelope,
            )?;
            let context = UsageContext::Path("terminal send");
            Ok(mutation(
                MutationCommand::TerminalSend {
                    target: target((*value).to_owned(), Resource::Terminal, envelope)?,
                    input: send_input(options, context, envelope)?,
                },
                options,
            ))
        }
        ["api", "snapshot"] => {
            reject_filters(options, UsageContext::Path("api snapshot"), envelope)?;
            Ok(query(QueryCommand::ApiSnapshot, options))
        }
        ["api", "schema"] => {
            reject_filters(options, UsageContext::Path("api schema"), envelope)?;
            Ok(query(QueryCommand::ApiSchema { schema_id: None }, options))
        }
        ["api", "schema", schema_id] => {
            reject_filters(options, UsageContext::Path("api schema"), envelope)?;
            if schema_id.is_empty() {
                return Err(identifier(Resource::Schema, String::new(), envelope));
            }
            Ok(query(
                QueryCommand::ApiSchema {
                    schema_id: Some((*schema_id).to_owned()),
                },
                options,
            ))
        }
        ["config", "check"] => {
            reject_filters(options, UsageContext::Path("config check"), envelope)?;
            Ok(query(QueryCommand::ConfigCheck, options))
        }
        ["completion", shell] => {
            reject_extras(options, UsageContext::Path("completion"), envelope)?;
            let shell = match *shell {
                "bash" => CompletionShell::Bash,
                "zsh" => CompletionShell::Zsh,
                "fish" => CompletionShell::Fish,
                _ => {
                    return Err(usage_with(
                        UsageReason::InvalidValue {
                            context: UsageContext::Path("completion"),
                            option: "<shell>",
                            value: (*shell).to_owned(),
                            expected: ExpectedValue::Shell,
                        },
                        envelope,
                    ));
                }
            };
            Ok(Request::Completion(shell))
        }
        // Command-scoped catch-alls classify the real failure — an extra
        // positional, a missing operand, a missing subcommand, or an
        // unrecognized word — after every legal arm has kept precedence.
        ["tui", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tui"),
            extra,
            envelope,
        )),
        ["status", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("status"),
            extra,
            envelope,
        )),
        ["project"] => Err(missing_command(UsageContext::Path("project"), envelope)),
        ["project", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project list"),
            extra,
            envelope,
        )),
        ["project", "get"] => Err(missing_required(
            UsageContext::Path("project get"),
            "<project>",
            envelope,
        )),
        ["project", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project get"),
            extra,
            envelope,
        )),
        ["project", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("project create"),
            extra,
            envelope,
        )),
        ["project", verb, ..] => Err(unknown_command(
            UsageContext::Path("project"),
            verb,
            envelope,
        )),
        ["workspace"] => Err(missing_command(UsageContext::Path("workspace"), envelope)),
        ["workspace", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace list"),
            extra,
            envelope,
        )),
        ["workspace", "get"] => Err(missing_required(
            UsageContext::Path("workspace get"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace get"),
            extra,
            envelope,
        )),
        ["workspace", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace create"),
            extra,
            envelope,
        )),
        ["workspace", "rename"] => Err(missing_required(
            UsageContext::Path("workspace rename"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "rename", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace rename"),
            extra,
            envelope,
        )),
        ["workspace", "accept-directory"] => Err(missing_required(
            UsageContext::Path("workspace accept-directory"),
            "<workspace>",
            envelope,
        )),
        ["workspace", "accept-directory", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("workspace accept-directory"),
            extra,
            envelope,
        )),
        ["workspace", verb, ..] => Err(unknown_command(
            UsageContext::Path("workspace"),
            verb,
            envelope,
        )),
        ["tab"] => Err(missing_command(UsageContext::Path("tab"), envelope)),
        ["tab", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab list"),
            extra,
            envelope,
        )),
        ["tab", "get"] => Err(missing_required(
            UsageContext::Path("tab get"),
            "<tab>",
            envelope,
        )),
        ["tab", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab get"),
            extra,
            envelope,
        )),
        ["tab", "create", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab create"),
            extra,
            envelope,
        )),
        ["tab", "rename"] => Err(missing_required(
            UsageContext::Path("tab rename"),
            "<tab>",
            envelope,
        )),
        ["tab", "rename", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab rename"),
            extra,
            envelope,
        )),
        ["tab", "move"] => Err(missing_required(
            UsageContext::Path("tab move"),
            "<tab>",
            envelope,
        )),
        ["tab", "move", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab move"),
            extra,
            envelope,
        )),
        ["tab", "close"] => Err(missing_required(
            UsageContext::Path("tab close"),
            "<tab>",
            envelope,
        )),
        ["tab", "close", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("tab close"),
            extra,
            envelope,
        )),
        ["tab", verb, ..] => Err(unknown_command(UsageContext::Path("tab"), verb, envelope)),
        ["panel"] => Err(missing_command(UsageContext::Path("panel"), envelope)),
        ["panel", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel list"),
            extra,
            envelope,
        )),
        ["panel", "get"] => Err(missing_required(
            UsageContext::Path("panel get"),
            "<panel>",
            envelope,
        )),
        ["panel", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel get"),
            extra,
            envelope,
        )),
        ["panel", "split"] => Err(missing_required(
            UsageContext::Path("panel split"),
            "<panel>",
            envelope,
        )),
        ["panel", "split", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel split"),
            extra,
            envelope,
        )),
        ["panel", "close"] => Err(missing_required(
            UsageContext::Path("panel close"),
            "<panel>",
            envelope,
        )),
        ["panel", "close", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel close"),
            extra,
            envelope,
        )),
        ["panel", "move"] => Err(missing_required(
            UsageContext::Path("panel move"),
            "<panel>",
            envelope,
        )),
        ["panel", "move", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("panel move"),
            extra,
            envelope,
        )),
        ["panel", verb, ..] => Err(unknown_command(UsageContext::Path("panel"), verb, envelope)),
        ["terminal"] => Err(missing_command(UsageContext::Path("terminal"), envelope)),
        ["terminal", "list", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal list"),
            extra,
            envelope,
        )),
        ["terminal", "get"] => Err(missing_required(
            UsageContext::Path("terminal get"),
            "<terminal>",
            envelope,
        )),
        ["terminal", "get", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal get"),
            extra,
            envelope,
        )),
        ["terminal", "read"] => Err(missing_required(
            UsageContext::Path("terminal read"),
            "<terminal>",
            envelope,
        )),
        ["terminal", "read", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal read"),
            extra,
            envelope,
        )),
        ["terminal", "wait-output"] => Err(missing_required(
            UsageContext::Path("terminal wait-output"),
            "<terminal>",
            envelope,
        )),
        ["terminal", "wait-output", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal wait-output"),
            extra,
            envelope,
        )),
        ["terminal", "send"] => Err(missing_required(
            UsageContext::Path("terminal send"),
            "<terminal>",
            envelope,
        )),
        ["terminal", "send", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("terminal send"),
            extra,
            envelope,
        )),
        ["terminal", verb, ..] => Err(unknown_command(
            UsageContext::Path("terminal"),
            verb,
            envelope,
        )),
        ["api"] => Err(missing_command(UsageContext::Path("api"), envelope)),
        ["api", "snapshot", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("api snapshot"),
            extra,
            envelope,
        )),
        ["api", "schema", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("api schema"),
            extra,
            envelope,
        )),
        ["api", verb, ..] => Err(unknown_command(UsageContext::Path("api"), verb, envelope)),
        ["config"] => Err(missing_command(UsageContext::Path("config"), envelope)),
        ["config", "check", extra, ..] => Err(unexpected_argument(
            UsageContext::Path("config check"),
            extra,
            envelope,
        )),
        ["config", verb, ..] => Err(unknown_command(
            UsageContext::Path("config"),
            verb,
            envelope,
        )),
        ["completion"] => Err(missing_required(
            UsageContext::Path("completion"),
            "<shell>",
            envelope,
        )),
        ["completion", _, extra, ..] => Err(unexpected_argument(
            UsageContext::Path("completion"),
            extra,
            envelope,
        )),
        [word, ..] => Err(unknown_command(UsageContext::Root, word, envelope)),
    }
}

fn resolve_help_path(
    rest: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    // A recognized topic anchors its own help path; an unrecognized one keeps
    // the literal `orchbee help` display so no hint points at a fake topic.
    let context = match help_topic(rest) {
        Some(HelpTopic::Family(family)) => UsageContext::Path(family.as_str()),
        Some(HelpTopic::Command(command)) => UsageContext::Path(command.as_str()),
        None => UsageContext::Literal("orchbee help"),
    };
    reject_extras(options, context, envelope)?;
    if rest.is_empty() {
        return Ok(Request::RootHelp);
    }
    help_topic(rest).map(Request::Help).ok_or_else(|| {
        usage_with(
            UsageReason::UnknownHelpTopic {
                path: rest.join(" "),
            },
            envelope,
        )
    })
}

/// The one recognized-help-path table shared by `help <path>` and a trailing
/// `-h`/`--help`: families map to their overview, complete command paths to
/// their command help. Anything else is not a help path at all.
fn help_topic(path: &[&str]) -> Option<HelpTopic> {
    Some(match path {
        ["tui"] => HelpTopic::Command(CommandPath::Tui),
        ["status"] => HelpTopic::Command(CommandPath::Status),
        ["project"] => HelpTopic::Family(Family::Project),
        ["workspace"] => HelpTopic::Family(Family::Workspace),
        ["tab"] => HelpTopic::Family(Family::Tab),
        ["panel"] => HelpTopic::Family(Family::Panel),
        ["terminal"] => HelpTopic::Family(Family::Terminal),
        ["api"] => HelpTopic::Family(Family::Api),
        ["config"] => HelpTopic::Family(Family::Config),
        ["completion"] => HelpTopic::Family(Family::Completion),
        ["project", "list"] => HelpTopic::Command(CommandPath::ProjectList),
        ["project", "get"] => HelpTopic::Command(CommandPath::ProjectGet),
        ["project", "create"] => HelpTopic::Command(CommandPath::ProjectCreate),
        ["workspace", "list"] => HelpTopic::Command(CommandPath::WorkspaceList),
        ["workspace", "get"] => HelpTopic::Command(CommandPath::WorkspaceGet),
        ["workspace", "create"] => HelpTopic::Command(CommandPath::WorkspaceCreate),
        ["workspace", "rename"] => HelpTopic::Command(CommandPath::WorkspaceRename),
        ["workspace", "accept-directory"] => {
            HelpTopic::Command(CommandPath::WorkspaceAcceptDirectory)
        }
        ["tab", "list"] => HelpTopic::Command(CommandPath::TabList),
        ["tab", "get"] => HelpTopic::Command(CommandPath::TabGet),
        ["tab", "create"] => HelpTopic::Command(CommandPath::TabCreate),
        ["tab", "rename"] => HelpTopic::Command(CommandPath::TabRename),
        ["tab", "move"] => HelpTopic::Command(CommandPath::TabMove),
        ["tab", "close"] => HelpTopic::Command(CommandPath::TabClose),
        ["panel", "list"] => HelpTopic::Command(CommandPath::PanelList),
        ["panel", "get"] => HelpTopic::Command(CommandPath::PanelGet),
        ["panel", "split"] => HelpTopic::Command(CommandPath::PanelSplit),
        ["panel", "close"] => HelpTopic::Command(CommandPath::PanelClose),
        ["panel", "move"] => HelpTopic::Command(CommandPath::PanelMove),
        ["terminal", "list"] => HelpTopic::Command(CommandPath::TerminalList),
        ["terminal", "get"] => HelpTopic::Command(CommandPath::TerminalGet),
        ["terminal", "read"] => HelpTopic::Command(CommandPath::TerminalRead),
        ["terminal", "wait-output"] => HelpTopic::Command(CommandPath::TerminalWaitOutput),
        ["terminal", "send"] => HelpTopic::Command(CommandPath::TerminalSend),

        ["api", "snapshot"] => HelpTopic::Command(CommandPath::ApiSnapshot),
        ["api", "schema"] => HelpTopic::Command(CommandPath::ApiSchema),
        ["config", "check"] => HelpTopic::Command(CommandPath::ConfigCheck),
        _ => return None,
    })
}

fn query(command: QueryCommand, options: &Options) -> Request {
    Request::Query {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    }
}

fn mutation(command: MutationCommand, options: &Options) -> Request {
    Request::Mutation {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    }
}

/// The named-option set one command admits; anything seen outside it is usage.
/// `geometry` covers all four dimension flags together: a terminal geometry
/// is only meaningful whole.
#[derive(Clone, Copy)]
struct Scope {
    project: bool,
    workspace: bool,
    tab: bool,
    standalone: bool,
    label: bool,
    direction: bool,
    geometry: bool,
    confirm: bool,
    target_panel: bool,
    /// `terminal read --history --rows <n>`: admits the flag and the
    /// history-row meaning of the shared `--rows` value option.
    history: bool,
    /// `workspace accept-directory --path <p>`: admits the literal directory
    /// path value option.
    path: bool,
    /// `terminal wait-output --match <t> [--rows <n>] [--timeout <ms>]`:
    /// admits all three flags and the wait-window meaning of `--rows`.
    wait: bool,
    /// `terminal send --text <t>`: the literal input-bytes value option.
    text: bool,
    /// `terminal send --enter`: the text form's same-record CR modifier.
    enter: bool,
    /// `terminal send --key <name>`: the semantic-key value option.
    key: bool,
    /// `tui --mouse off|on`: admits the TUI-only pointer-capture switch.
    mouse: bool,
}

const NO_OPTIONS: Scope = Scope {
    project: false,
    workspace: false,
    tab: false,
    standalone: false,
    label: false,
    direction: false,
    geometry: false,
    confirm: false,
    target_panel: false,
    history: false,
    path: false,
    wait: false,
    text: false,
    enter: false,
    key: false,
    mouse: false,
};

/// The explicit `tui` route: only `--mouse` is admitted on top of the
/// shared option grammar.
const TUI_SCOPE: Scope = Scope {
    mouse: true,
    ..NO_OPTIONS
};

const WORKSPACE_LIST_SCOPE: Scope = Scope {
    project: true,
    ..NO_OPTIONS
};

const TAB_LIST_SCOPE: Scope = Scope {
    workspace: true,
    standalone: true,
    ..NO_OPTIONS
};

const PANEL_LIST_SCOPE: Scope = Scope {
    tab: true,
    ..NO_OPTIONS
};

const WORKSPACE_CREATE_SCOPE: Scope = Scope {
    project: true,
    label: true,
    ..NO_OPTIONS
};

const LABEL_SCOPE: Scope = Scope {
    label: true,
    ..NO_OPTIONS
};

const ACCEPT_DIRECTORY_SCOPE: Scope = Scope {
    path: true,
    ..NO_OPTIONS
};

const TAB_CREATE_SCOPE: Scope = Scope {
    workspace: true,
    standalone: true,
    label: true,
    geometry: true,
    ..NO_OPTIONS
};

const TAB_MOVE_SCOPE: Scope = Scope {
    direction: true,
    ..NO_OPTIONS
};

const CONFIRM_SCOPE: Scope = Scope {
    confirm: true,
    ..NO_OPTIONS
};

const PANEL_SPLIT_SCOPE: Scope = Scope {
    label: true,
    direction: true,
    geometry: true,
    ..NO_OPTIONS
};

const PANEL_MOVE_SCOPE: Scope = Scope {
    direction: true,
    target_panel: true,
    ..NO_OPTIONS
};

const TERMINAL_READ_SCOPE: Scope = Scope {
    history: true,
    ..NO_OPTIONS
};

const TERMINAL_WAIT_OUTPUT_SCOPE: Scope = Scope {
    wait: true,
    ..NO_OPTIONS
};

const TERMINAL_SEND_SCOPE: Scope = Scope {
    text: true,
    enter: true,
    key: true,
    ..NO_OPTIONS
};

/// Any mutation-only option seen outside a mutation command.
fn business_options(options: &Options) -> bool {
    options.label.is_some()
        || options.direction.is_some()
        || options.cols.is_some()
        || options.rows.is_some()
        || options.cell_width.is_some()
        || options.cell_height.is_some()
        || options.confirm
        || options.target_panel.is_some()
        || options.history
        || options.path.is_some()
        || options.match_text.is_some()
        || options.timeout.is_some()
        || options.text.is_some()
        || options.enter
        || options.key.is_some()
}

/// The first named option present but outside `scope`, in the fixed `Options`
/// declaration order — deterministic without any argv-order state.
fn first_out_of_scope(options: &Options, scope: Scope) -> Option<&'static str> {
    if options.project.is_some() && !scope.project {
        return Some("--project");
    }
    if options.workspace.is_some() && !scope.workspace {
        return Some("--workspace");
    }
    if options.tab.is_some() && !scope.tab {
        return Some("--tab");
    }
    if options.standalone && !scope.standalone {
        return Some("--standalone");
    }
    if options.label.is_some() && !scope.label {
        return Some("--label");
    }
    if options.direction.is_some() && !scope.direction {
        return Some("--direction");
    }
    if options.cols.is_some() && !scope.geometry {
        return Some("--cols");
    }
    // `--rows` is a geometry dimension under `geometry` scope, the paired
    // history row count under `history` scope, and the searched window under
    // `wait` scope; all three spellings share the flag.
    if options.rows.is_some() && !scope.geometry && !scope.history && !scope.wait {
        return Some("--rows");
    }
    if options.cell_width.is_some() && !scope.geometry {
        return Some("--cell-width");
    }
    if options.cell_height.is_some() && !scope.geometry {
        return Some("--cell-height");
    }
    if options.confirm && !scope.confirm {
        return Some("--confirm");
    }
    if options.target_panel.is_some() && !scope.target_panel {
        return Some("--target-panel");
    }
    if options.history && !scope.history {
        return Some("--history");
    }
    if options.path.is_some() && !scope.path {
        return Some("--path");
    }
    if options.match_text.is_some() && !scope.wait {
        return Some("--match");
    }
    if options.timeout.is_some() && !scope.wait {
        return Some("--timeout");
    }
    if options.text.is_some() && !scope.text {
        return Some("--text");
    }
    if options.enter && !scope.enter {
        return Some("--enter");
    }
    if options.key.is_some() && !scope.key {
        return Some("--key");
    }
    if options.mouse.is_some() && !scope.mouse {
        return Some("--mouse");
    }
    None
}

/// Reject every named option outside the command's admitted set, naming the
/// first out-of-scope flag. `--format` is orthogonal and never scope-checked.
fn check_scope(
    options: &Options,
    scope: Scope,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    match first_out_of_scope(options, scope) {
        Some(option) => Err(usage_with(
            UsageReason::UnexpectedOption { context, option },
            envelope,
        )),
        None => Ok(()),
    }
}

fn required(
    slot: &Option<String>,
    context: UsageContext,
    requirement: &'static str,
    envelope: FailureEnvelope,
) -> Result<String, ParseFailure> {
    slot.clone()
        .ok_or_else(|| missing_required(context, requirement, envelope))
}

/// `--mouse` accepts exactly `off`/`on` as a separate token; anything else —
/// including the `--mouse=<value>` equals form — is a usage failure, never a
/// guessed default.
fn mouse_option(value: &str) -> Option<bool> {
    match value {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

/// `--label` carries a wire-ready Workbench label: non-empty and at most the
/// protocol's 512-scalar bound.
fn label_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchLabel, ParseFailure> {
    let value = required(&options.label, context, "'--label'", envelope)?;
    WorkbenchLabel::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--label",
                value,
                expected: ExpectedValue::Label,
            },
            envelope,
        )
    })
}

/// `--path` carries a wire-ready Workbench directory path: the literal
/// absolute POSIX path, at most the protocol's 4,096-byte bound. The CLI
/// never expands `~`/`$`, never creates the directory, and never probes a
/// filesystem — a malformed value is a usage failure at this boundary.
fn path_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchDirectoryPath, ParseFailure> {
    let value = required(&options.path, context, "'--path'", envelope)?;
    WorkbenchDirectoryPath::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--path",
                value,
                expected: ExpectedValue::DirectoryPath,
            },
            envelope,
        )
    })
}

/// The four geometry flags parse as u16 dimensions and must satisfy the
/// protocol geometry bounds (columns/rows 1..=4096, cell pixel dimensions
/// 0..=4096 and both zero or both nonzero, grid ≤ 262144 cells, pixel extents
/// ≤ 65535).
fn geometry_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTerminalGeometry, ParseFailure> {
    let dimension = |slot: &Option<String>, flag: &'static str| -> Result<u16, ParseFailure> {
        let value = required(
            slot,
            context,
            match flag {
                "--cols" => "'--cols'",
                "--rows" => "'--rows'",
                "--cell-width" => "'--cell-width'",
                _ => "'--cell-height'",
            },
            envelope,
        )?;
        value.parse::<u16>().map_err(|_| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: flag,
                    value,
                    expected: ExpectedValue::Dimension,
                },
                envelope,
            )
        })
    };
    let cols = dimension(&options.cols, "--cols")?;
    let rows = dimension(&options.rows, "--rows")?;
    let cell_width = dimension(&options.cell_width, "--cell-width")?;
    let cell_height = dimension(&options.cell_height, "--cell-height")?;
    WorkbenchTerminalGeometry::new(cols, rows, cell_width, cell_height).map_err(|_| {
        usage_with(
            UsageReason::InvalidGeometry {
                context,
                cols,
                rows,
                cell_width,
                cell_height,
            },
            envelope,
        )
    })
}

/// The `--rows` value under `--history`: a bounded retained-row count, not a
/// geometry dimension. 1..=1000 rows per the read-history contract.
fn history_rows_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u16, ParseFailure> {
    let value = required(&options.rows, context, "'--rows'", envelope)?;
    match value.parse::<u16>() {
        Ok(rows) if (1..=1000).contains(&rows) => Ok(rows),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--rows",
                value,
                expected: ExpectedValue::HistoryRows,
            },
            envelope,
        )),
    }
}

/// `--match` under `wait-output` is required: literal, case-sensitive,
/// non-empty UTF-8 text of at most 4096 bytes. Nothing is interpreted — no
/// regex, escape sequences, or ANSI processing.
fn wait_match_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<String, ParseFailure> {
    let value = required(&options.match_text, context, "'--match'", envelope)?;
    if value.is_empty() || value.len() > 4096 {
        return Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--match",
                value,
                expected: ExpectedValue::MatchText,
            },
            envelope,
        ));
    }
    Ok(value)
}

/// `--rows` under `wait-output`: the searched retained-row window spanning
/// visible plus scrollback as one range. 1..=1000, defaulting to 1000.
fn wait_rows_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u16, ParseFailure> {
    match options.rows.is_some() {
        true => history_rows_option(options, context, envelope),
        false => Ok(1000),
    }
}

/// `--timeout` under `wait-output`: the total wait budget in milliseconds
/// measured from command start. 1..=3600000, defaulting to 30000.
fn wait_timeout_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u32, ParseFailure> {
    let Some(value) = options.timeout.clone() else {
        return Ok(30_000);
    };
    match value.parse::<u32>() {
        Ok(timeout) if (1..=3_600_000).contains(&timeout) => Ok(timeout),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--timeout",
                value,
                expected: ExpectedValue::WaitTimeout,
            },
            envelope,
        )),
    }
}

/// The wire's single-record input bound: literal bytes plus the optional
/// folded CR must fit one input payload.
const MAXIMUM_SEND_TEXT_BYTES: usize = 4096;

/// `terminal send` admits exactly one payload form. `--text`/`--key` are
/// mutually exclusive and one is required; `--enter` is the text form's own
/// modifier and never legal alone or under `--key`. The text byte budget
/// counts the payload the wire actually carries — the literal bytes plus the
/// appended CR — so an over-limit record is refused before any connection.
fn send_input(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<TerminalSendInput, ParseFailure> {
    match (&options.text, &options.key) {
        (Some(text), None) => {
            let bytes = text.len() + usize::from(options.enter);
            if text.is_empty() || bytes > MAXIMUM_SEND_TEXT_BYTES {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--text",
                        value: text.clone(),
                        expected: ExpectedValue::SendText,
                    },
                    envelope,
                ));
            }
            Ok(TerminalSendInput::Text {
                text: text.clone(),
                enter: options.enter,
            })
        }
        (None, Some(name)) => {
            if options.enter {
                return Err(usage_with(
                    UsageReason::UnexpectedOption {
                        context,
                        option: "--enter",
                    },
                    envelope,
                ));
            }
            let Some(key) = TerminalSendKey::parse(name) else {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--key",
                        value: name.clone(),
                        expected: ExpectedValue::SendKey,
                    },
                    envelope,
                ));
            };
            Ok(TerminalSendInput::Key(key))
        }
        (Some(_), Some(_)) => Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--text",
                second: "--key",
            },
            envelope,
        )),
        (None, None) => {
            if options.enter {
                return Err(usage_with(
                    UsageReason::UnexpectedOption {
                        context,
                        option: "--enter",
                    },
                    envelope,
                ));
            }
            Err(missing_required(context, "'--text' or '--key'", envelope))
        }
    }
}

fn tab_direction(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTabMoveDirection, ParseFailure> {
    let value = required(&options.direction, context, "'--direction'", envelope)?;
    match value.as_str() {
        "previous" => Ok(WorkbenchTabMoveDirection::Previous),
        "next" => Ok(WorkbenchTabMoveDirection::Next),
        "first" => Ok(WorkbenchTabMoveDirection::First),
        "last" => Ok(WorkbenchTabMoveDirection::Last),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--direction",
                value,
                expected: ExpectedValue::TabDirection,
            },
            envelope,
        )),
    }
}

fn panel_direction(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchPanelSplitDirection, ParseFailure> {
    let value = required(&options.direction, context, "'--direction'", envelope)?;
    match value.as_str() {
        "above" => Ok(WorkbenchPanelSplitDirection::Above),
        "below" => Ok(WorkbenchPanelSplitDirection::Below),
        "left" => Ok(WorkbenchPanelSplitDirection::Left),
        "right" => Ok(WorkbenchPanelSplitDirection::Right),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--direction",
                value,
                expected: ExpectedValue::PanelDirection,
            },
            envelope,
        )),
    }
}

/// Reject the `--workspace`/`--standalone` combination when it actually fires.
fn reject_conflict(
    conflict: bool,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    if conflict {
        Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--workspace",
                second: "--standalone",
            },
            envelope,
        ))
    } else {
        Ok(())
    }
}

fn missing_required(
    context: UsageContext,
    requirement: &'static str,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::MissingRequired {
            context,
            requirement,
        },
        envelope,
    )
}

fn missing_command(context: UsageContext, envelope: FailureEnvelope) -> ParseFailure {
    usage_with(UsageReason::MissingCommand { context }, envelope)
}

fn unknown_command(context: UsageContext, word: &str, envelope: FailureEnvelope) -> ParseFailure {
    usage_with(
        UsageReason::UnknownCommand {
            context,
            word: word.to_owned(),
        },
        envelope,
    )
}

fn unexpected_argument(
    context: UsageContext,
    value: &str,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::UnexpectedArgument {
            context,
            value: value.to_owned(),
        },
        envelope,
    )
}

/// Reject any option/filter/format on commands that accept none, naming the
/// first rejected flag in the fixed declaration order (`--format` last).
fn reject_extras(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    reject_extras_scope(options, NO_OPTIONS, context, envelope)
}

/// `reject_extras` against an explicit admitted set: `--format` stays
/// rejected while the scope's own flags (today only `tui`'s `--mouse`)
/// pass through.
fn reject_extras_scope(
    options: &Options,
    scope: Scope,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    match first_out_of_scope(options, scope)
        .or_else(|| options.format.is_some().then_some("--format"))
    {
        Some(option) => Err(usage_with(
            UsageReason::UnexpectedOption { context, option },
            envelope,
        )),
        None => Ok(()),
    }
}

/// Reject the named filters; `--format` remains legal on data commands.
fn reject_filters(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<(), ParseFailure> {
    check_scope(options, NO_OPTIONS, context, envelope)
}

#[cfg(test)]
mod tests;
