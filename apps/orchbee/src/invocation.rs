use std::ffi::OsString;

use orchbee_workbench_protocol::{
    WorkbenchLabel, WorkbenchPanelSplitDirection, WorkbenchTabMoveDirection,
    WorkbenchTerminalGeometry,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandPath {
    Tui,
    Status,
    ProjectList,
    ProjectGet,
    WorkspaceList,
    WorkspaceGet,
    WorkspaceCreate,
    WorkspaceRename,
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
    ApiSnapshot,
    ApiSchema,
    ConfigCheck,
    Completion,
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
    TerminalRead(Target),
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

/// One effectful Workbench command admitted by the grammar. Every field is a
/// wire-ready value already checked against protocol bounds, so the executor
/// never re-validates argv.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MutationCommand {
    WorkspaceCreate {
        project: Target,
        label: WorkbenchLabel,
    },
    WorkspaceRename {
        target: Target,
        label: WorkbenchLabel,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Request {
    Tui,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParseFailure {
    pub(crate) kind: ParseFailureKind,
    pub(crate) envelope: FailureEnvelope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Word(String),
    HelpFlag,
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
}

/// Every named option that owns the next argv token as its value, used by
/// both the format-envelope walk and the option scan so ownership is decided
/// identically in the two passes.
const VALUE_FLAGS: &[&str] = &[
    "--cell-height",
    "--cell-width",
    "--cols",
    "--direction",
    "--label",
    "--project",
    "--rows",
    "--tab",
    "--target-panel",
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

fn usage(envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope,
    }
}

fn identifier(resource: Resource, target: String, envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidIdentifier { resource, target },
        envelope,
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
            return Err(ParseFailure {
                kind: ParseFailureKind::InvalidUsage,
                envelope: FailureEnvelope::Silent,
            });
        };
        total_bytes += text.len();
        if total_bytes > MAXIMUM_ARGUMENT_BYTES {
            return Err(ParseFailure {
                kind: ParseFailureKind::InvalidUsage,
                envelope: FailureEnvelope::Silent,
            });
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
    let silent = || ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: FailureEnvelope::Silent,
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
                    return Err(silent());
                };
                let selected = match value.as_str() {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    _ => return Err(silent()),
                };
                if format.replace(selected).is_some() {
                    return Err(silent());
                }
                index += 2;
            }
            _ if token.starts_with("--format=") => {
                let selected = match &token["--format=".len()..] {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    _ => return Err(silent()),
                };
                if format.replace(selected).is_some() {
                    return Err(silent());
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
            _ if VALUE_FLAGS.contains(&token.as_str()) => {
                let value = match tokens.get(index + 1) {
                    Some(value) if value != "--" => value.clone(),
                    _ => return Err(usage(envelope)),
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
                    _ => &mut options.target_panel,
                };
                if slot.replace(value).is_some() {
                    return Err(usage(envelope));
                }
                index += 2;
            }
            "--standalone" | "--confirm" => {
                let seen = if token == "--standalone" {
                    &mut options.standalone
                } else {
                    &mut options.confirm
                };
                if *seen {
                    return Err(usage(envelope));
                }
                *seen = true;
                index += 1;
            }
            "--help" => {
                positionals.push(Token::HelpFlag);
                index += 1;
            }
            "--version" => {
                positionals.push(Token::VersionFlag);
                index += 1;
            }
            _ if token.starts_with('-') && token.len() > 1 => {
                return Err(usage(envelope));
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
        reject_extras(&options, envelope)?;
        return Ok(Request::Version);
    }
    // A trailing --help applies to an exact complete command path only.
    let (path, suffix_help) = match positionals.last() {
        Some(Token::HelpFlag) => (&positionals[..positionals.len() - 1], true),
        _ => (&positionals[..], false),
    };
    if suffix_help {
        return resolve_help_suffix(path, &options, envelope);
    }
    if path
        .iter()
        .any(|token| matches!(token, Token::HelpFlag | Token::VersionFlag))
    {
        return Err(usage(envelope));
    }
    let words: Vec<&str> = path
        .iter()
        .map(|token| match token {
            Token::Word(word) => word.as_str(),
            Token::HelpFlag | Token::VersionFlag => unreachable!("flags excluded above"),
        })
        .collect();
    resolve_words(&words, &options, envelope)
}

fn resolve_help_suffix(
    words: &[Token],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    // --help must be the sole suffix of a complete command path; other flag
    // tokens, a selected format, filters, or business arguments make it usage.
    reject_extras(options, envelope)?;
    if words
        .iter()
        .any(|token| matches!(token, Token::HelpFlag | Token::VersionFlag))
    {
        return Err(usage(envelope));
    }
    let path: Vec<&str> = words
        .iter()
        .map(|token| match token {
            Token::Word(word) => word.as_str(),
            Token::HelpFlag | Token::VersionFlag => unreachable!("flags excluded above"),
        })
        .collect();
    let topic = match path.as_slice() {
        [] => return Ok(Request::RootHelp),
        ["tui"] => HelpTopic::Command(CommandPath::Tui),
        ["status"] => HelpTopic::Command(CommandPath::Status),
        ["project", "list"] => HelpTopic::Command(CommandPath::ProjectList),
        ["project", "get"] => HelpTopic::Command(CommandPath::ProjectGet),
        ["workspace", "list"] => HelpTopic::Command(CommandPath::WorkspaceList),
        ["workspace", "get"] => HelpTopic::Command(CommandPath::WorkspaceGet),
        ["workspace", "create"] => HelpTopic::Command(CommandPath::WorkspaceCreate),
        ["workspace", "rename"] => HelpTopic::Command(CommandPath::WorkspaceRename),
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
        ["api", "snapshot"] => HelpTopic::Command(CommandPath::ApiSnapshot),
        ["api", "schema"] => HelpTopic::Command(CommandPath::ApiSchema),
        ["config", "check"] => HelpTopic::Command(CommandPath::ConfigCheck),
        ["completion"] => HelpTopic::Command(CommandPath::Completion),
        _ => return Err(usage(envelope)),
    };
    Ok(Request::Help(topic))
}

fn resolve_words(
    words: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    match words {
        [] => {
            // Bare invocation keeps TUI; a selected format or any option is usage.
            if options.format.is_some()
                || options.project.is_some()
                || options.workspace.is_some()
                || options.tab.is_some()
                || options.standalone
                || business_options(options)
            {
                return Err(usage(envelope));
            }
            Ok(Request::Tui)
        }
        ["tui"] => reject_extras(options, envelope).map(|()| Request::Tui),
        ["help", rest @ ..] => resolve_help_path(rest, options, envelope),
        ["status"] => {
            reject_filters(options, envelope)?;
            Ok(query(QueryCommand::Status, options))
        }
        ["project", "list"] => {
            reject_filters(options, envelope)?;
            Ok(query(QueryCommand::ProjectList, options))
        }
        ["project", "get", value] => {
            reject_filters(options, envelope)?;
            Ok(query(
                QueryCommand::ProjectGet(target((*value).to_owned(), Resource::Project, envelope)?),
                options,
            ))
        }
        ["workspace", "list"] => {
            reject(
                options.workspace.is_some()
                    || options.tab.is_some()
                    || options.standalone
                    || business_options(options),
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
            reject_filters(options, envelope)?;
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
            check_scope(options, WORKSPACE_CREATE_SCOPE, envelope)?;
            Ok(mutation(
                MutationCommand::WorkspaceCreate {
                    project: target(
                        required(&options.project, envelope)?,
                        Resource::Project,
                        envelope,
                    )?,
                    label: label_option(options, envelope)?,
                },
                options,
            ))
        }
        ["workspace", "rename", value] => {
            check_scope(options, LABEL_SCOPE, envelope)?;
            Ok(mutation(
                MutationCommand::WorkspaceRename {
                    target: target((*value).to_owned(), Resource::Workspace, envelope)?,
                    label: label_option(options, envelope)?,
                },
                options,
            ))
        }
        ["tab", "list"] => {
            reject(
                options.project.is_some() || options.tab.is_some() || business_options(options),
                envelope,
            )?;
            reject(options.standalone && options.workspace.is_some(), envelope)?;
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
            reject_filters(options, envelope)?;
            Ok(query(
                QueryCommand::TabGet(target((*value).to_owned(), Resource::Tab, envelope)?),
                options,
            ))
        }
        ["tab", "create"] => {
            check_scope(options, TAB_CREATE_SCOPE, envelope)?;
            reject(options.standalone && options.workspace.is_some(), envelope)?;
            let context = if options.standalone {
                TabContext::Standalone
            } else {
                TabContext::Workspace(target(
                    required(&options.workspace, envelope)?,
                    Resource::Workspace,
                    envelope,
                )?)
            };
            Ok(mutation(
                MutationCommand::TabCreate {
                    context,
                    label: label_option(options, envelope)?,
                    geometry: geometry_option(options, envelope)?,
                },
                options,
            ))
        }
        ["tab", "rename", value] => {
            check_scope(options, LABEL_SCOPE, envelope)?;
            Ok(mutation(
                MutationCommand::TabRename {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                    label: label_option(options, envelope)?,
                },
                options,
            ))
        }
        ["tab", "move", value] => {
            check_scope(options, TAB_MOVE_SCOPE, envelope)?;
            Ok(mutation(
                MutationCommand::TabMove {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                    direction: tab_direction(options, envelope)?,
                },
                options,
            ))
        }
        ["tab", "close", value] => {
            check_scope(options, CONFIRM_SCOPE, envelope)?;
            reject(!options.confirm, envelope)?;
            Ok(mutation(
                MutationCommand::TabClose {
                    target: target((*value).to_owned(), Resource::Tab, envelope)?,
                },
                options,
            ))
        }
        ["panel", "list"] => {
            reject(
                options.project.is_some()
                    || options.workspace.is_some()
                    || options.standalone
                    || business_options(options),
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
            reject_filters(options, envelope)?;
            Ok(query(
                QueryCommand::PanelGet(target((*value).to_owned(), Resource::Panel, envelope)?),
                options,
            ))
        }
        ["panel", "split", value] => {
            check_scope(options, PANEL_SPLIT_SCOPE, envelope)?;
            Ok(mutation(
                MutationCommand::PanelSplit {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                    direction: panel_direction(options, envelope)?,
                    label: label_option(options, envelope)?,
                    geometry: geometry_option(options, envelope)?,
                },
                options,
            ))
        }
        ["panel", "close", value] => {
            check_scope(options, CONFIRM_SCOPE, envelope)?;
            reject(!options.confirm, envelope)?;
            Ok(mutation(
                MutationCommand::PanelClose {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                },
                options,
            ))
        }
        ["panel", "move", value] => {
            check_scope(options, PANEL_MOVE_SCOPE, envelope)?;
            Ok(mutation(
                MutationCommand::PanelMove {
                    target: target((*value).to_owned(), Resource::Panel, envelope)?,
                    into: target(
                        required(&options.target_panel, envelope)?,
                        Resource::Panel,
                        envelope,
                    )?,
                    direction: panel_direction(options, envelope)?,
                },
                options,
            ))
        }
        ["terminal", "list"] => {
            reject_filters(options, envelope)?;
            Ok(query(QueryCommand::TerminalList, options))
        }
        ["terminal", "get", value] => {
            reject_filters(options, envelope)?;
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
            reject_filters(options, envelope)?;
            Ok(query(
                QueryCommand::TerminalRead(target(
                    (*value).to_owned(),
                    Resource::Terminal,
                    envelope,
                )?),
                options,
            ))
        }
        ["api", "snapshot"] => {
            reject_filters(options, envelope)?;
            Ok(query(QueryCommand::ApiSnapshot, options))
        }
        ["api", "schema"] => {
            reject_filters(options, envelope)?;
            Ok(query(QueryCommand::ApiSchema { schema_id: None }, options))
        }
        ["api", "schema", schema_id] => {
            reject_filters(options, envelope)?;
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
            reject_filters(options, envelope)?;
            Ok(query(QueryCommand::ConfigCheck, options))
        }
        ["completion", shell] => {
            reject_extras(options, envelope)?;
            let shell = match *shell {
                "bash" => CompletionShell::Bash,
                "zsh" => CompletionShell::Zsh,
                "fish" => CompletionShell::Fish,
                _ => return Err(usage(envelope)),
            };
            Ok(Request::Completion(shell))
        }
        _ => Err(usage(envelope)),
    }
}

fn resolve_help_path(
    rest: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    reject_extras(options, envelope)?;
    let topic = match rest {
        [] => return Ok(Request::RootHelp),
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
        ["workspace", "list"] => HelpTopic::Command(CommandPath::WorkspaceList),
        ["workspace", "get"] => HelpTopic::Command(CommandPath::WorkspaceGet),
        ["workspace", "create"] => HelpTopic::Command(CommandPath::WorkspaceCreate),
        ["workspace", "rename"] => HelpTopic::Command(CommandPath::WorkspaceRename),
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
        ["api", "snapshot"] => HelpTopic::Command(CommandPath::ApiSnapshot),
        ["api", "schema"] => HelpTopic::Command(CommandPath::ApiSchema),
        ["config", "check"] => HelpTopic::Command(CommandPath::ConfigCheck),
        _ => return Err(usage(envelope)),
    };
    Ok(Request::Help(topic))
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
}

/// Reject every named option outside the command's admitted set. `--format`
/// is orthogonal and never scope-checked.
fn check_scope(options: &Options, scope: Scope, envelope: FailureEnvelope) -> Result<(), ParseFailure> {
    reject(
        (options.project.is_some() && !scope.project)
            || (options.workspace.is_some() && !scope.workspace)
            || (options.tab.is_some() && !scope.tab)
            || (options.standalone && !scope.standalone)
            || (options.label.is_some() && !scope.label)
            || (options.direction.is_some() && !scope.direction)
            || ((options.cols.is_some()
                || options.rows.is_some()
                || options.cell_width.is_some()
                || options.cell_height.is_some())
                && !scope.geometry)
            || (options.confirm && !scope.confirm)
            || (options.target_panel.is_some() && !scope.target_panel),
        envelope,
    )
}

fn required(slot: &Option<String>, envelope: FailureEnvelope) -> Result<String, ParseFailure> {
    slot.clone().ok_or_else(|| usage(envelope))
}

/// `--label` carries a wire-ready Workbench label: non-empty and at most the
/// protocol's 512-scalar bound.
fn label_option(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<WorkbenchLabel, ParseFailure> {
    WorkbenchLabel::parse(&required(&options.label, envelope)?).map_err(|_| usage(envelope))
}

/// The four geometry flags parse as u16 dimensions and must satisfy the
/// protocol geometry bounds (each dimension 1..=4096, grid ≤ 262144 cells,
/// pixel extents ≤ 65535).
fn geometry_option(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTerminalGeometry, ParseFailure> {
    let dimension = |slot: &Option<String>| -> Result<u16, ParseFailure> {
        required(slot, envelope)?
            .parse::<u16>()
            .map_err(|_| usage(envelope))
    };
    WorkbenchTerminalGeometry::new(
        dimension(&options.cols)?,
        dimension(&options.rows)?,
        dimension(&options.cell_width)?,
        dimension(&options.cell_height)?,
    )
    .map_err(|_| usage(envelope))
}

fn tab_direction(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTabMoveDirection, ParseFailure> {
    match required(&options.direction, envelope)?.as_str() {
        "previous" => Ok(WorkbenchTabMoveDirection::Previous),
        "next" => Ok(WorkbenchTabMoveDirection::Next),
        "first" => Ok(WorkbenchTabMoveDirection::First),
        "last" => Ok(WorkbenchTabMoveDirection::Last),
        _ => Err(usage(envelope)),
    }
}

fn panel_direction(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<WorkbenchPanelSplitDirection, ParseFailure> {
    match required(&options.direction, envelope)?.as_str() {
        "above" => Ok(WorkbenchPanelSplitDirection::Above),
        "below" => Ok(WorkbenchPanelSplitDirection::Below),
        "left" => Ok(WorkbenchPanelSplitDirection::Left),
        "right" => Ok(WorkbenchPanelSplitDirection::Right),
        _ => Err(usage(envelope)),
    }
}

fn reject(condition: bool, envelope: FailureEnvelope) -> Result<(), ParseFailure> {
    if condition {
        Err(usage(envelope))
    } else {
        Ok(())
    }
}

/// Reject any option/filter/format on commands that accept none.
fn reject_extras(options: &Options, envelope: FailureEnvelope) -> Result<(), ParseFailure> {
    reject(
        options.format.is_some()
            || options.project.is_some()
            || options.workspace.is_some()
            || options.tab.is_some()
            || options.standalone
            || business_options(options),
        envelope,
    )
}

/// Reject the named filters; `--format` remains legal on data commands.
fn reject_filters(options: &Options, envelope: FailureEnvelope) -> Result<(), ParseFailure> {
    reject(
        options.project.is_some()
            || options.workspace.is_some()
            || options.tab.is_some()
            || options.standalone
            || business_options(options),
        envelope,
    )
}

#[cfg(test)]
mod tests;
