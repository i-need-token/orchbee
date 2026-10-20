//! Only the declared terminal input/output commands admit Panel-to-primary targeting.
use super::{
    FailureEnvelope, Options, ParseFailure, Resource, Target, UsageContext, UsageReason,
    missing_required, target, unexpected_argument, usage_with,
};

pub(super) fn parse(
    operands: &[&str],
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Target, ParseFailure> {
    if let Some(panel) = &options.panel {
        if !operands.is_empty() {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first: "<target>",
                    second: "--panel",
                },
                envelope,
            ));
        }
        return panel_target(panel, envelope);
    }
    let [value] = operands else {
        return Err(if operands.is_empty() {
            missing_required(context, "<target> or --panel <panel>", envelope)
        } else {
            unexpected_argument(context, operands[1], envelope)
        });
    };
    if value.starts_with("@panel:") {
        return target((*value).to_owned(), Resource::Panel, envelope);
    }
    Ok(
        match target((*value).to_owned(), Resource::Terminal, envelope)? {
            Target::Id(id) => Target::TerminalOrPanelId(id),
            other => other,
        },
    )
}

fn panel_target(value: &str, envelope: FailureEnvelope) -> Result<Target, ParseFailure> {
    Ok(match target(value.to_owned(), Resource::Panel, envelope)? {
        Target::Id(id) => Target::Selector(format!("@panel:0x{}", id.trim_start_matches('0'))),
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use crate::invocation::parse_request;
    use std::ffi::OsString;

    #[test]
    fn four_commands_admit_panel_identity_and_reject_mixed_selectors() {
        for (verb, payload) in [
            ("read", vec![]),
            ("wait-output", vec!["--match", "ready"]),
            ("send", vec!["--text", "hello"]),
            ("run", vec!["echo hello"]),
        ] {
            for selector in [
                vec!["@panel:0x1"],
                vec!["--panel", "@panel:0x1"],
                vec!["--panel", "00000000000000000000000000000001"],
            ] {
                let mut words = vec!["terminal", verb];
                words.extend(selector);
                words.extend(payload.clone());
                assert!(
                    parse_request(words.iter().map(OsString::from)).is_ok(),
                    "{words:?}"
                );
            }
            let mut words = vec!["terminal", verb, "@terminal:0x1", "--panel", "@panel:0x2"];
            words.extend(payload.clone());
            assert!(
                parse_request(words.iter().map(OsString::from)).is_err(),
                "{words:?}"
            );
        }
    }
}
