//! Explicit terminal snapshot sources and data-export grammar.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputSource {
    Visible,
    Recent,
    RecentUnwrapped,
    Detection,
}

impl OutputSource {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Recent => "recent",
            Self::RecentUnwrapped => "recent-unwrapped",
            Self::Detection => "detection",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OutputSelection {
    pub(crate) source: OutputSource,
    pub(crate) lines: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OutputTarget {
    Terminal(Target),
    Agent(AgentTarget),
}

pub(super) fn request(
    target: OutputTarget,
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let selector = if options.output_source.is_some() {
        "--source"
    } else {
        "--plain"
    };
    for (present, other) in [
        (options.history, "--history"),
        (options.rows.is_some(), "--rows"),
        (options.unwrap, "--unwrap"),
    ] {
        if present {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: selector,
                    second: other,
                },
                envelope,
            ));
        }
    }
    if options.plain && options.format == Some(OutputFormat::Json) {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--plain",
                second: "--format",
            },
            envelope,
        ));
    }
    Ok(query(
        QueryCommand::TerminalOutput {
            target,
            selection: selection(options, context, envelope)?,
            plain: options.plain,
        },
        options,
    ))
}

pub(super) fn selection(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<OutputSelection, ParseFailure> {
    let source = match options.output_source.as_deref().unwrap_or("recent") {
        "visible" => OutputSource::Visible,
        "recent" => OutputSource::Recent,
        "recent-unwrapped" => OutputSource::RecentUnwrapped,
        "detection" => OutputSource::Detection,
        value => {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--source",
                    value: value.to_owned(),
                    expected: ExpectedValue::OutputSource,
                },
                envelope,
            ));
        }
    };
    let lines = options
        .lines
        .as_ref()
        .map(|value| {
            value
                .parse::<u32>()
                .ok()
                .filter(|_| !value.starts_with('+'))
                .map(|value| u16::try_from(value.min(1000)).expect("clamped to 1000"))
                .ok_or_else(|| {
                    usage_with(
                        UsageReason::InvalidValue {
                            context,
                            option: "--lines",
                            value: value.clone(),
                            expected: ExpectedValue::OutputLines,
                        },
                        envelope,
                    )
                })
        })
        .transpose()?;
    Ok(OutputSelection { source, lines })
}

pub(super) fn wait_request(
    target: Target,
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    if options.rows.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--rows",
                second: if options.output_source.is_some() {
                    "--source"
                } else if options.lines.is_some() {
                    "--lines"
                } else {
                    "--forever"
                },
            },
            envelope,
        ));
    }
    if options.forever && options.timeout.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--forever",
                second: "--timeout",
            },
            envelope,
        ));
    }
    Ok(query(
        QueryCommand::TerminalSourceWait {
            target,
            selection: selection(options, context, envelope)?,
            matcher: wait_matcher_option(options, context, envelope)?,
            timeout_ms: if options.forever {
                None
            } else {
                Some(wait_timeout_option(options, context, envelope)?)
            },
        },
        options,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
        parse_request(words.iter().map(OsString::from))
    }

    #[test]
    fn source_read_preserves_legacy_mode_and_bounds_new_physical_lines() {
        let id = "11111111111111111111111111111111";
        for (argument, expected) in [("0", 0), ("17", 17), ("4294967295", 1000)] {
            let Request::Query {
                command:
                    QueryCommand::TerminalOutput {
                        selection, plain, ..
                    },
                ..
            } = parse(&[
                "terminal",
                "read",
                id,
                "--source=recent-unwrapped",
                "--lines",
                argument,
                "--plain",
            ])
            .expect("source read")
            else {
                panic!("source query")
            };
            assert_eq!(
                selection,
                OutputSelection {
                    source: OutputSource::RecentUnwrapped,
                    lines: Some(expected)
                }
            );
            assert!(plain);
        }
        assert!(matches!(
            parse(&["terminal", "read", id]),
            Ok(Request::Query {
                command: QueryCommand::TerminalRead { .. },
                ..
            })
        ));
        assert!(matches!(
            parse(&["agent", "read", "--name", "worker", "--plain"]),
            Ok(Request::Query {
                command: QueryCommand::TerminalOutput {
                    target: OutputTarget::Agent(_),
                    selection: OutputSelection {
                        source: OutputSource::Recent,
                        lines: None
                    },
                    plain: true
                },
                ..
            })
        ));
    }

    #[test]
    fn invalid_or_mixed_source_forms_fail_before_runtime() {
        let id = "11111111111111111111111111111111";
        for extra in [
            vec!["--source", "bad"],
            vec!["--source", "visible", "--unwrap"],
            vec!["--plain", "--rows", "2"],
            vec!["--source", "recent", "--history"],
            vec!["--plain", "--format=json"],
            vec!["--plain", "--plain"],
            vec!["--plain", "--lines", "4294967296"],
            vec!["--plain", "--lines", "+1"],
            vec!["--plain", "--lines", "-1"],
        ] {
            let mut words = vec!["terminal", "read", id];
            words.extend(extra);
            assert!(parse(&words).is_err(), "{words:?}");
        }
        for words in [
            vec!["status", "--plain"],
            vec!["terminal", "wait-output", id, "--plain", "--match", "x"],
            vec!["tui", "--source", "visible"],
            vec!["--version", "--plain"],
        ] {
            assert!(parse(&words).is_err(), "{words:?}");
        }
    }

    #[test]
    fn source_wait_defaults_bounds_and_forever_preserve_legacy_grammar() {
        let id = "11111111111111111111111111111111";
        for (extra, source, lines, timeout) in [
            (
                vec!["--source=visible"],
                OutputSource::Visible,
                None,
                Some(30_000),
            ),
            (
                vec!["--lines=0"],
                OutputSource::Recent,
                Some(0),
                Some(30_000),
            ),
            (
                vec!["--lines=4294967295", "--timeout=7"],
                OutputSource::Recent,
                Some(1000),
                Some(7),
            ),
            (vec!["--forever"], OutputSource::Recent, None, None),
            (
                vec!["--source=detection", "--forever"],
                OutputSource::Detection,
                None,
                None,
            ),
        ] {
            let mut words = vec!["terminal", "wait-output", id, "--match=x"];
            words.extend(extra);
            let Request::Query {
                command:
                    QueryCommand::TerminalSourceWait {
                        selection,
                        timeout_ms,
                        ..
                    },
                ..
            } = parse(&words).unwrap()
            else {
                panic!("source wait")
            };
            assert_eq!(selection, OutputSelection { source, lines });
            assert_eq!(timeout_ms, timeout);
        }
        assert!(matches!(
            parse(&["terminal", "wait-output", id, "--match=x"]),
            Ok(Request::Query {
                command: QueryCommand::TerminalWaitOutput {
                    rows: 1000,
                    timeout_ms: 30_000,
                    ..
                },
                ..
            })
        ));
        for extra in [
            vec!["--source=visible", "--rows=2"],
            vec!["--lines=1", "--rows=2"],
            vec!["--forever", "--rows=2"],
            vec!["--forever", "--timeout=1"],
            vec!["--source=visible", "--timeout=0"],
            vec!["--source=bad"],
            vec!["--lines=+1"],
            vec!["--lines=4294967296"],
            vec!["--forever", "--forever"],
            vec!["--source=visible", "--history"],
        ] {
            let mut words = vec!["terminal", "wait-output", id, "--match=x"];
            words.extend(extra);
            assert!(parse(&words).is_err(), "{words:?}");
        }
    }
}
