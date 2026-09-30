//! Buffer coordinates and content fences are parsed before runtime discovery.
use super::{
    ExpectedValue, FailureEnvelope, ParseFailure, TerminalTextPoint, UsageContext, UsageReason,
    missing_required, usage_with,
};

pub(super) fn point(
    raw: Option<&str>,
    option: &'static str,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<TerminalTextPoint, ParseFailure> {
    let raw = raw.ok_or_else(|| missing_required(context, option, envelope))?;
    let failure = || {
        invalid(
            raw,
            option,
            ExpectedValue::TerminalSelectionPoint,
            context,
            envelope,
        )
    };
    let (row, col) = raw.split_once(':').ok_or_else(failure)?;
    if !digits(row) || !digits(col) {
        return Err(failure());
    }
    Ok(TerminalTextPoint {
        row: row.parse().map_err(|_| failure())?,
        col: col.parse().map_err(|_| failure())?,
    })
}
pub(super) fn number(
    raw: &str,
    option: &'static str,
    positive: bool,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<u64, ParseFailure> {
    let expected = if positive {
        ExpectedValue::TerminalContentGeneration
    } else {
        ExpectedValue::TerminalContentRevision
    };
    let failure = || invalid(raw, option, expected, context, envelope);
    if !digits(raw) {
        return Err(failure());
    }
    let value = raw.parse().map_err(|_| failure())?;
    if positive && value == 0 {
        return Err(failure());
    }
    Ok(value)
}
fn digits(raw: &str) -> bool {
    !raw.is_empty() && raw.bytes().all(|byte| byte.is_ascii_digit())
}
pub(super) fn invalid(
    raw: &str,
    option: &'static str,
    expected: ExpectedValue,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> ParseFailure {
    usage_with(
        UsageReason::InvalidValue {
            context,
            option,
            value: raw.into(),
            expected,
        },
        envelope,
    )
}
