use super::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalObserveCommand {
    pub(crate) target: Target,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) timeout_ms: Option<u32>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalControlCommand {
    pub(crate) stream: TerminalObserveCommand,
    pub(crate) takeover: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AttachTarget {
    Terminal(Target),
    Agent(AgentTarget),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalAttachCommand {
    pub(crate) target: AttachTarget,
    pub(crate) takeover: bool,
    pub(crate) timeout_ms: Option<u32>,
    pub(crate) mouse: bool,
}
pub(super) fn attach(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
    agent: bool,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path(if agent {
        "agent attach"
    } else {
        "terminal attach"
    });
    check_scope(
        options,
        Scope {
            takeover: true,
            mouse: true,
            timeout: true,
            name: agent,
            session: agent,
            panel: agent,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let selected = if agent {
        AttachTarget::Agent(agent_selector(operands, options, context, envelope)?)
    } else {
        let [value] = operands else {
            return Err(if operands.is_empty() {
                missing_required(context, "<terminal>", envelope)
            } else {
                unexpected_argument(context, operands[1], envelope)
            });
        };
        AttachTarget::Terminal(target((*value).to_owned(), Resource::Terminal, envelope)?)
    };
    Ok(Request::TerminalAttach {
        command: TerminalAttachCommand {
            target: selected,
            takeover: options.takeover,
            mouse: options.mouse.unwrap_or(true),
            timeout_ms: if options.timeout.is_some() {
                agent_wait_timeout_option(options, context, envelope)?
            } else {
                None
            },
        },
    })
}
pub(super) fn control(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    Ok(Request::TerminalControl {
        command: TerminalControlCommand {
            stream: stream(operands, options, envelope, true)?,
            takeover: options.takeover,
        },
    })
}
pub(super) fn observe(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    Ok(Request::TerminalObserve {
        command: stream(operands, options, envelope, false)?,
    })
}
fn stream(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
    control: bool,
) -> Result<TerminalObserveCommand, ParseFailure> {
    let context = UsageContext::Path(if control {
        "terminal session control"
    } else {
        "terminal session observe"
    });
    check_scope(
        options,
        Scope {
            takeover: control,
            geometry: true,
            timeout: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    for (present, flag) in [
        (options.cell_width.is_some(), "--cell-width"),
        (options.cell_height.is_some(), "--cell-height"),
    ] {
        if present {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context,
                    option: flag,
                },
                envelope,
            ));
        }
    }
    let [value] = operands else {
        return Err(if operands.is_empty() {
            missing_required(context, "<terminal>", envelope)
        } else {
            unexpected_argument(context, operands[1], envelope)
        });
    };
    let dimension = |value: &Option<String>, default, option| {
        value.as_ref().map_or(Ok(default), |value| {
            value
                .parse::<u16>()
                .ok()
                .filter(|n| (1..=4096).contains(n) && value.bytes().all(|c| c.is_ascii_digit()))
                .ok_or_else(|| {
                    usage_with(
                        UsageReason::InvalidValue {
                            context,
                            option,
                            value: value.clone(),
                            expected: ExpectedValue::Dimension,
                        },
                        envelope,
                    )
                })
        })
    };
    let columns = dimension(&options.cols, 120, "--cols")?;
    let rows = dimension(&options.rows, 40, "--rows")?;
    if u32::from(columns) * u32::from(rows) > 262144 {
        return Err(usage_with(
            UsageReason::InvalidGeometry {
                context,
                cols: columns,
                rows,
                cell_width: 0,
                cell_height: 0,
            },
            envelope,
        ));
    }
    let timeout_ms = if options.timeout.is_some() {
        agent_wait_timeout_option(options, context, envelope)?
    } else {
        None
    };
    Ok(TerminalObserveCommand {
        target: target((*value).to_owned(), Resource::Terminal, envelope)?,
        columns,
        rows,
        timeout_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attach_targets_are_explicit_and_options_are_scoped_before_effects() {
        let parse = |words: &[&str]| parse_request(words.iter().map(OsString::from));
        let Request::TerminalAttach { command } = parse(&[
            "--mouse=off",
            "terminal",
            "attach",
            "@terminal:ab",
            "--takeover",
            "--timeout=50",
            "--format=json",
        ])
        .unwrap() else {
            panic!("attach")
        };
        assert!(!command.mouse);
        assert!(command.takeover);
        assert_eq!(command.timeout_ms, Some(50));
        assert!(matches!(command.target, AttachTarget::Terminal(_)));
        for words in [
            vec!["agent", "attach", "alpha"],
            vec!["agent", "attach", "--name=alpha"],
            vec!["agent", "attach", "--session=session-1"],
            vec!["agent", "attach", "--panel=@panel:ab"],
        ] {
            let Request::TerminalAttach { command } = parse(&words).unwrap() else {
                panic!("agent attach")
            };
            assert!(command.mouse);
            assert!(!command.takeover);
            assert_eq!(command.timeout_ms, None);
            assert!(matches!(command.target, AttachTarget::Agent(_)));
        }
        for words in [
            vec!["terminal", "attach"],
            vec!["terminal", "attach", "@terminal:ab", "extra"],
            vec!["terminal", "attach", "@terminal:ab", "--cols=80"],
            vec!["terminal", "attach", "@terminal:ab", "--mouse=invalid"],
            vec![
                "terminal",
                "attach",
                "@terminal:ab",
                "--mouse=on",
                "--mouse=off",
            ],
            vec!["terminal", "attach", "@terminal:ab", "--timeout=0"],
            vec!["terminal", "attach", "@terminal:ab", "--name=alpha"],
            vec!["agent", "attach"],
            vec!["agent", "attach", "alpha", "--name=alpha"],
            vec![
                "agent",
                "attach",
                "--panel=@panel:ab",
                "--session=session-1",
            ],
            vec!["agent", "attach", "--name=alpha", "--stdin"],
        ] {
            assert!(parse(&words).is_err(), "{words:?}");
        }
    }

    fn parse(extra: &[&str]) -> Result<Request, ParseFailure> {
        parse_request(
            ["terminal", "session", "observe"]
                .into_iter()
                .chain(extra.iter().copied())
                .map(OsString::from),
        )
    }
    #[test]
    fn terminal_control_scopes_takeover_and_reuses_checked_dimensions() {
        let parse = |words: &[&str]| parse_request(words.iter().map(OsString::from));
        let Request::TerminalControl { command } = parse(&[
            "terminal",
            "session",
            "control",
            "@terminal:ab",
            "--takeover",
            "--cols=100",
        ])
        .unwrap() else {
            panic!("control")
        };
        assert!(command.takeover);
        assert_eq!((command.stream.columns, command.stream.rows), (100, 40));
        for words in [
            vec![
                "terminal",
                "session",
                "observe",
                "@terminal:ab",
                "--takeover",
            ],
            vec![
                "terminal",
                "session",
                "control",
                "@terminal:ab",
                "--takeover",
                "--takeover",
            ],
            vec!["terminal", "session", "control", "@terminal:ab", "--cols=0"],
            vec!["terminal", "session", "control", "@terminal:ab", "--stdin"],
        ] {
            assert!(parse(&words).is_err());
        }
    }
    #[test]
    fn terminal_observe_defaults_and_scoped_options() {
        let Request::TerminalObserve { command } = parse(&["@terminal:ab"]).expect("parse") else {
            panic!("stream request")
        };
        assert_eq!(
            (command.columns, command.rows, command.timeout_ms),
            (120, 40, None)
        );
        let Request::TerminalObserve { command } = parse(&[
            "@terminal:ab",
            "--cols=256",
            "--rows=1024",
            "--timeout=50",
            "--format=json",
        ])
        .expect("parse") else {
            panic!("stream request")
        };
        assert_eq!(
            (command.columns, command.rows, command.timeout_ms),
            (256, 1024, Some(50))
        );
        for extra in [
            vec![],
            vec!["@terminal:ab", "extra"],
            vec!["@terminal:ab", "--cols=0"],
            vec!["@terminal:ab", "--cols=+1"],
            vec!["@terminal:ab", "--cols=4096", "--rows=4096"],
            vec!["@terminal:ab", "--timeout=0"],
            vec!["@terminal:ab", "--cell-width=8"],
            vec!["@terminal:ab", "--stdin"],
            vec!["@terminal:ab", "--enter"],
            vec!["@terminal:ab", "--history"],
        ] {
            assert!(parse(&extra).is_err(), "{extra:?}");
        }
        assert_eq!(
            help_topic(&["terminal", "session", "observe"]),
            Some(HelpTopic::Command(CommandPath::TerminalObserve))
        );
    }
}
