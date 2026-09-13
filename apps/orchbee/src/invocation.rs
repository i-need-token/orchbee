use std::ffi::OsString;

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
    TabList,
    TabGet,
    PanelList,
    PanelGet,
    TerminalList,
    TerminalGet,
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
    ApiSnapshot,
    ApiSchema {
        schema_id: Option<String>,
    },
    ConfigCheck,
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
}

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
            "--project" | "--workspace" | "--tab" => {
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
            "--project" | "--workspace" | "--tab" => {
                let value = match tokens.get(index + 1) {
                    Some(value) if value != "--" => value.clone(),
                    _ => return Err(usage(envelope)),
                };
                let slot = match token.as_str() {
                    "--project" => &mut options.project,
                    "--workspace" => &mut options.workspace,
                    _ => &mut options.tab,
                };
                if slot.replace(value).is_some() {
                    return Err(usage(envelope));
                }
                index += 2;
            }
            "--standalone" => {
                if options.standalone {
                    return Err(usage(envelope));
                }
                options.standalone = true;
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
        ["tab", "list"] => HelpTopic::Command(CommandPath::TabList),
        ["tab", "get"] => HelpTopic::Command(CommandPath::TabGet),
        ["panel", "list"] => HelpTopic::Command(CommandPath::PanelList),
        ["panel", "get"] => HelpTopic::Command(CommandPath::PanelGet),
        ["terminal", "list"] => HelpTopic::Command(CommandPath::TerminalList),
        ["terminal", "get"] => HelpTopic::Command(CommandPath::TerminalGet),
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
            // Bare invocation keeps TUI; a selected format or any filter is usage.
            if options.format.is_some()
                || options.project.is_some()
                || options.workspace.is_some()
                || options.tab.is_some()
                || options.standalone
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
                options.workspace.is_some() || options.tab.is_some() || options.standalone,
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
        ["tab", "list"] => {
            reject(options.project.is_some() || options.tab.is_some(), envelope)?;
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
        ["panel", "list"] => {
            reject(
                options.project.is_some() || options.workspace.is_some() || options.standalone,
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
        ["tab", "list"] => HelpTopic::Command(CommandPath::TabList),
        ["tab", "get"] => HelpTopic::Command(CommandPath::TabGet),
        ["panel", "list"] => HelpTopic::Command(CommandPath::PanelList),
        ["panel", "get"] => HelpTopic::Command(CommandPath::PanelGet),
        ["terminal", "list"] => HelpTopic::Command(CommandPath::TerminalList),
        ["terminal", "get"] => HelpTopic::Command(CommandPath::TerminalGet),
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
            || options.standalone,
        envelope,
    )
}

/// Reject the named filters; `--format` remains legal on data commands.
fn reject_filters(options: &Options, envelope: FailureEnvelope) -> Result<(), ParseFailure> {
    reject(
        options.project.is_some()
            || options.workspace.is_some()
            || options.tab.is_some()
            || options.standalone,
        envelope,
    )
}

#[cfg(test)]
mod tests;
