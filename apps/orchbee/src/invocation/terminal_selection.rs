//! Public screen-buffer coordinates are admitted before runtime discovery.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, Target, UsageContext, UsageReason, check_scope, missing_required, target,
    usage_with,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct TerminalTextPoint {
    pub(crate) row: u32,
    pub(crate) col: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalSelectionCommand {
    pub(crate) target: Target,
    pub(crate) anchor: TerminalTextPoint,
    pub(crate) cursor: TerminalTextPoint,
    pub(crate) revision: Option<u64>,
    pub(crate) generation: Option<u64>,
}

const CONTEXT: UsageContext = UsageContext::Path("terminal selection-read");

pub(super) fn parse(
    value: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    check_scope(
        options,
        Scope {
            selection_anchor: true,
            content_revision: true,
            cursor: true,
            generation: true,
            ..NO_OPTIONS
        },
        CONTEXT,
        envelope,
    )?;
    let point = |raw: Option<&str>, option| {
        let raw = raw.ok_or_else(|| missing_required(CONTEXT, option, envelope))?;
        let invalid = || invalid(raw, option, ExpectedValue::TerminalSelectionPoint, envelope);
        let (row, col) = raw.split_once(':').ok_or_else(invalid)?;
        if !digits(row) || !digits(col) {
            return Err(invalid());
        }
        Ok(TerminalTextPoint {
            row: row.parse().map_err(|_| invalid())?,
            col: col.parse().map_err(|_| invalid())?,
        })
    };
    let anchor = point(options.selection_anchor.as_deref(), "--anchor")?;
    let cursor = point(options.cursor.as_deref(), "--cursor")?;
    if u64::from(anchor.row.abs_diff(cursor.row)) + 1 > 4096 {
        return Err(invalid(
            options.cursor.as_deref().expect("required cursor"),
            "--cursor",
            ExpectedValue::TerminalSelectionSpan,
            envelope,
        ));
    }
    let number = |raw: &str, option, positive| {
        let expected = if positive {
            ExpectedValue::TerminalContentGeneration
        } else {
            ExpectedValue::TerminalContentRevision
        };
        let invalid = || invalid(raw, option, expected, envelope);
        if !digits(raw) {
            return Err(invalid());
        }
        let number: u64 = raw.parse().map_err(|_| invalid())?;
        if positive && number == 0 {
            return Err(invalid());
        }
        Ok(number)
    };
    Ok(Request::TerminalSelection {
        command: TerminalSelectionCommand {
            target: target(value.to_owned(), Resource::Terminal, envelope)?,
            anchor,
            cursor,
            revision: options
                .content_revision
                .as_deref()
                .map(|raw| number(raw, "--content-revision", false))
                .transpose()?,
            generation: options
                .generation
                .as_deref()
                .map(|raw| number(raw, "--generation", true))
                .transpose()?,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

fn digits(raw: &str) -> bool {
    !raw.is_empty() && raw.bytes().all(|byte| byte.is_ascii_digit())
}

fn invalid(
    raw: &str,
    option: &'static str,
    expected: ExpectedValue,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::InvalidValue {
            context: CONTEXT,
            option,
            value: raw.into(),
            expected,
        },
        envelope,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    fn parse_args(words: &[&str]) -> Result<Request, ParseFailure> {
        crate::invocation::parse_request(words.iter().map(OsString::from))
    }
    const ID: &str = "33333333333333333333333333333333";
    #[test]
    fn selection_read_admits_full_reference_coordinate_and_revision_ranges() {
        let Request::TerminalSelection { command, format } = parse_args(&[
            "terminal",
            "selection-read",
            ID,
            "--anchor=4294967295:65535",
            "--cursor",
            "4294967294:0",
            "--content-revision",
            "18446744073709551615",
            "--generation=18446744073709551615",
            "--format=json",
        ])
        .unwrap() else {
            panic!("selection command")
        };
        assert_eq!(command.anchor.row, u32::MAX);
        assert_eq!(command.anchor.col, u16::MAX);
        assert_eq!(command.revision, Some(u64::MAX));
        assert_eq!(command.generation, Some(u64::MAX));
        assert_eq!(format, OutputFormat::Json);
    }
    #[test]
    fn selection_read_rejects_malformed_missing_duplicate_and_foreign_inputs() {
        for raw in [
            "",
            "0",
            "0:0:0",
            "-1:0",
            "+1:0",
            " 1:0",
            "4294967296:0",
            "0:65536",
        ] {
            assert!(
                parse_args(&[
                    "terminal",
                    "selection-read",
                    ID,
                    "--anchor",
                    raw,
                    "--cursor",
                    "0:0"
                ])
                .is_err(),
                "{raw}"
            );
        }
        for extra in [
            vec!["--anchor", "0:0"],
            vec!["--source", "visible"],
            vec!["--generation", "0"],
            vec!["--content-revision", "18446744073709551616"],
            vec!["--content-revision", "+1"],
        ] {
            let mut words = vec![
                "terminal",
                "selection-read",
                ID,
                "--anchor",
                "0:0",
                "--cursor",
                "0:0",
            ];
            words.extend(extra);
            assert!(parse_args(&words).is_err(), "{words:?}");
        }
        assert!(parse_args(&["terminal", "selection-read", ID]).is_err());
        assert!(parse_args(&["terminal", "read", ID, "--anchor", "0:0"]).is_err());
    }
    #[test]
    fn selection_read_span_is_bounded_before_discovery_in_either_direction() {
        for (anchor, cursor, ok) in [
            ("0:0", "4095:0", true),
            ("4095:0", "0:0", true),
            ("0:0", "4096:0", false),
            ("4294967295:0", "0:0", false),
        ] {
            assert_eq!(
                parse_args(&[
                    "terminal",
                    "selection-read",
                    ID,
                    "--anchor",
                    anchor,
                    "--cursor",
                    cursor
                ])
                .is_ok(),
                ok
            );
        }
    }
}
