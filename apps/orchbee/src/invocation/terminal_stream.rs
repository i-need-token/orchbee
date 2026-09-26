use super::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalObserveCommand {
    pub(crate) target: Target,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) timeout_ms: Option<u32>,
}
pub(super) fn observe(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("terminal session observe");
    check_scope(
        options,
        Scope {
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
    Ok(Request::TerminalObserve {
        command: TerminalObserveCommand {
            target: target((*value).to_owned(), Resource::Terminal, envelope)?,
            columns,
            rows,
            timeout_ms,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(extra: &[&str]) -> Result<Request, ParseFailure> {
        parse_request(
            ["terminal", "session", "observe"]
                .into_iter()
                .chain(extra.iter().copied())
                .map(OsString::from),
        )
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
