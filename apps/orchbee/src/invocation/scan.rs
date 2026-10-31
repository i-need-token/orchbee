//! Lexical argv pass: the format envelope walk, the option scan into `Options`, token
//! classification, help flags, and request selection (session and availability).

use super::{
    ExpectedValue, FailureEnvelope, HelpTopic, Options, OutputFormat, ParseFailure, Request,
    SelectedRequest, UsageContext, UsageReason, grammar, help, help_paths, reject_extras,
    reject_filters, resolve_help_path, resolve_words, selection_envelope, usage_with,
};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Word(String),
    HelpFlag(&'static str),
    VersionFlag(&'static str),
    SkillFlag,
    DefaultConfigFlag,
}

/// Every named option that owns the next argv token as its value, used by
/// both the format-envelope walk and the option scan so ownership is decided
/// identically in the two passes.
static VALUE_FLAGS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
    grammar::OPTIONS
        .iter()
        .filter(|option| option.value.is_some())
        .map(|option| option.spelling)
        .collect()
});

/// Phase A: scan the complete pre-`--` format envelope before any grammar
/// work. A missing, unknown, empty, or repeated selection anywhere in the
/// envelope is a silent format failure, and one valid unique selection shapes
/// every later grammar failure regardless of its position. Filter flags own
/// the next token even when it resembles a selector, so this walk applies the
/// same left-to-right ownership as the option scan below; a bare `--` is never
/// consumed as a value.
/// The longest command or family path spelled by the leading words, for
/// diagnostics raised before the grammar runs. Option values are skipped.
fn command_context(tokens: &[String]) -> UsageContext {
    let mut words = Vec::new();
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        let token = token.as_str();
        if token == "--" {
            break;
        }
        index += if token == "--format" || VALUE_FLAGS.contains(&token) {
            2
        } else {
            if !token.starts_with('-') {
                words.push(token);
            }
            1
        };
    }
    help_paths()
        .iter()
        .filter(|path| words.starts_with(path.words()))
        .max_by_key(|path| path.words().len())
        .map_or(UsageContext::Root, |path| {
            UsageContext::Path(match path.topic() {
                HelpTopic::Command(command) => command.as_str(),
                HelpTopic::Family(family) => family.as_str(),
            })
        })
}

pub(super) fn format_envelope(tokens: &[String]) -> Result<Option<OutputFormat>, ParseFailure> {
    let missing = || {
        usage_with(
            UsageReason::MissingValue { option: "--format" },
            FailureEnvelope::Silent,
        )
    };
    let invalid = |value: &str| {
        usage_with(
            UsageReason::InvalidValue {
                context: command_context(tokens),
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
            "--json" => {
                if format.replace(OutputFormat::Json).is_some() {
                    return Err(duplicate());
                }
                index += 1;
            }
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

pub(super) fn scan(tokens: Vec<String>) -> Result<SelectedRequest, ParseFailure> {
    let mut options = Options {
        format: format_envelope(&tokens)?,
        ..Options::default()
    };
    let envelope = selection_envelope(options.format);
    let mut positionals = Vec::new();
    let mut ended = false;
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
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
            "--format" => {
                options.seen.push("--format");
                index += 2;
            }
            _ if token.starts_with("--format=") => {
                options.seen.push("--format");
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
            "--version" | "-v" | "-V" => {
                positionals.push(Token::VersionFlag(match token.as_str() {
                    "-v" => "-v",
                    "-V" => "-V",
                    _ => "--version",
                }));
                index += 1;
            }
            "--skill" => {
                positionals.push(Token::SkillFlag);
                index += 1;
            }
            "--default-config" => {
                positionals.push(Token::DefaultConfigFlag);
                index += 1;
            }
            _ => {
                let (spelling, joined) = token
                    .split_once('=')
                    .map_or((token.as_str(), None), |(flag, value)| (flag, Some(value)));
                if let Some(spec) = grammar::option(spelling)
                    .filter(|spec| joined.is_none() || (spec.value.is_some() && spec.joined))
                {
                    let value = if spec.value.is_some() {
                        if let Some(value) = joined {
                            index += 1;
                            value.to_owned()
                        } else {
                            let value = tokens
                                .get(index + 1)
                                .filter(|value| value.as_str() != "--")
                                .ok_or_else(|| {
                                    usage_with(
                                        UsageReason::MissingValue {
                                            option: spec.spelling,
                                        },
                                        envelope,
                                    )
                                })?;
                            index += 2;
                            value.clone()
                        }
                    } else {
                        index += 1;
                        String::new()
                    };
                    if !spec.repeatable && options.seen.contains(&spec.spelling) {
                        return Err(usage_with(
                            UsageReason::DuplicateOption {
                                option: spec.spelling,
                            },
                            envelope,
                        ));
                    }
                    if spec.value.is_some() {
                        options.values.push((spec.spelling, value.clone()));
                    }
                    (spec.apply)(&mut options, value, envelope)?;
                    options.seen.push(spec.spelling);
                } else if token.starts_with('-') && token.len() > 1 {
                    return Err(usage_with(
                        UsageReason::UnknownOption {
                            context: help_context(
                                &positionals
                                    .iter()
                                    .filter_map(word_literal)
                                    .collect::<Vec<_>>(),
                            ),
                            option: token.clone(),
                        },
                        envelope,
                    ));
                } else {
                    positionals.push(Token::Word(token.clone()));
                    index += 1;
                }
            }
        }
    }
    let mut session = options
        .session
        .take()
        .map(|name| {
            orchbee_local_runtime::SessionName::parse(&name).map_err(|_| {
                usage_with(
                    UsageReason::InvalidValue {
                        context: UsageContext::Root,
                        option: "--session",
                        value: name,
                        expected: ExpectedValue::SessionName,
                    },
                    envelope,
                )
            })
        })
        .transpose()?;
    let command_words = positionals
        .iter()
        .filter_map(word_literal)
        .collect::<Vec<_>>();
    let availability = grammar::COMMANDS
        .iter()
        .filter(|command| command_words.starts_with(command.words))
        .max_by_key(|command| command.words.len())
        .map_or(grammar::Availability::Portable, |command| {
            command.availability
        });
    let format = options.format;
    let request = resolve(positionals, options)?;
    if matches!(&request, Request::DefaultConfig) && session.is_some() {
        return Err(usage_with(
            UsageReason::UnexpectedOption {
                context: UsageContext::Literal("orchbee --default-config"),
                option: "--session",
            },
            envelope,
        ));
    }
    if let Request::NamedSession { command, .. } = &request
        && let Some(name) = command.name()
    {
        if session.as_ref().is_some_and(|selected| selected != name) {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context: UsageContext::Path("session"),
                    first: "--session",
                    second: "<name>",
                },
                envelope,
            ));
        }
        session = Some(name.clone());
    }
    Ok(SelectedRequest {
        request,
        session,
        availability,
        format,
    })
}

fn resolve(positionals: Vec<Token>, options: Options) -> Result<Request, ParseFailure> {
    let envelope = selection_envelope(options.format);
    if positionals.len() == 1 && matches!(positionals[0], Token::DefaultConfigFlag) {
        reject_extras(
            &options,
            UsageContext::Literal("orchbee --default-config"),
            envelope,
        )?;
        return Ok(Request::DefaultConfig);
    }
    if positionals.len() == 1 && matches!(positionals[0], Token::SkillFlag) {
        reject_extras(&options, UsageContext::Literal("orchbee --skill"), envelope)?;
        return Ok(Request::Skill);
    }
    // Every version spelling is legal only as the sole root invocation.
    if positionals.len() == 1 && matches!(positionals[0], Token::VersionFlag(_)) {
        reject_extras(
            &options,
            UsageContext::Literal("orchbee --version"),
            envelope,
        )?;
        return Ok(Request::Version);
    }
    // Help overrides business requirements after lexical option validation.
    // Tokens after `--`, including a literal --help operand, are only Words.
    if positionals
        .iter()
        .any(|token| matches!(token, Token::HelpFlag(_)))
    {
        return resolve_help_flag(&positionals, &options, envelope);
    }
    let path = &positionals[..];
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
            Token::HelpFlag(_) | Token::VersionFlag(_) | Token::SkillFlag => {
                unreachable!("flags excluded above")
            }
            Token::DefaultConfigFlag => unreachable!("flags excluded above"),
        })
        .collect();
    let request = resolve_words(&words, &options, envelope)?;
    if let Some(command) = grammar::COMMANDS
        .iter()
        .filter(|command| words.starts_with(command.words))
        .max_by_key(|command| command.words.len())
    {
        command.validate(&words, &options, envelope)?;
    }
    Ok(request)
}

fn flag_literal(token: &Token) -> Option<&'static str> {
    match token {
        Token::HelpFlag(literal) => Some(literal),
        Token::VersionFlag(literal) => Some(literal),
        Token::SkillFlag => Some("--skill"),
        Token::DefaultConfigFlag => Some("--default-config"),
        Token::Word(_) => None,
    }
}

fn word_literal(token: &Token) -> Option<&str> {
    match token {
        Token::Word(word) => Some(word.as_str()),
        Token::HelpFlag(_) | Token::VersionFlag(_) | Token::SkillFlag => None,
        Token::DefaultConfigFlag => None,
    }
}

/// The help-path context of already-recognized words; unrecognized paths fall
/// back to the root so a hint never points at a nonexistent topic.
fn help_context(words: &[&str]) -> UsageContext {
    match help::context_topic(words) {
        Some(topic) => UsageContext::Path(help::diagnostic_path(topic)),
        None => UsageContext::Root,
    }
}

fn resolve_help_flag(
    tokens: &[Token],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let words: Vec<&str> = tokens.iter().filter_map(word_literal).collect();
    let context = help_context(&words);
    let mut seen_help = false;
    if let Some(flag) = tokens.iter().find_map(|token| {
        if matches!(token, Token::HelpFlag(_)) && !seen_help {
            seen_help = true;
            None
        } else {
            flag_literal(token)
        }
    }) {
        return Err(usage_with(
            UsageReason::MisplacedFlag { context, flag },
            envelope,
        ));
    }
    if let Some(rest) = words.strip_prefix(&["help"]) {
        return resolve_help_path(rest, options, envelope);
    }
    if words.is_empty() {
        reject_filters(options, UsageContext::Root, envelope)?;
        return Ok(Request::RootHelp);
    }
    help::invocation_topic(&words)
        .map(Request::Help)
        .ok_or_else(|| {
            usage_with(
                UsageReason::UnknownHelpTopic {
                    path: words.join(" "),
                },
                envelope,
            )
        })
}
