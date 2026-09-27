//! Panel display reports are normalized and validated before runtime admission.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, OutputFormat, ParseFailure, Request, Resource,
    Scope, Target, UsageContext, UsageReason, check_scope, target, usage_with,
};
use orchbee_workbench_protocol::{
    MetadataSource, MetadataState, MetadataTokenValue, PanelMetadataPatch, WorkbenchRevision,
};
#[derive(Default)]
pub(super) struct Options {
    pub(super) agent: Option<String>,
    pub(super) applies_to_source: Option<String>,
    pub(super) title: Option<String>,
    pub(super) display_agent: Option<String>,
    pub(super) state_labels: Vec<String>,
    pub(super) clear_title: bool,
    pub(super) clear_display_agent: bool,
    pub(super) clear_state_labels: bool,
}
impl Options {
    pub(super) fn first_option(&self) -> Option<&'static str> {
        [
            (self.agent.is_some(), "--agent"),
            (self.applies_to_source.is_some(), "--applies-to-source"),
            (self.title.is_some(), "--title"),
            (self.display_agent.is_some(), "--display-agent"),
            (!self.state_labels.is_empty(), "--state-label"),
            (self.clear_title, "--clear-title"),
            (self.clear_display_agent, "--clear-display-agent"),
            (self.clear_state_labels, "--clear-state-labels"),
        ]
        .into_iter()
        .find_map(|(seen, flag)| seen.then_some(flag))
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PanelMetadataCommand {
    pub(crate) target: Target,
    pub(crate) source: MetadataSource,
    pub(crate) patch: PanelMetadataPatch,
    pub(crate) sequence: Option<WorkbenchRevision>,
    pub(crate) ttl_milliseconds: Option<u32>,
}
pub(super) fn parse(
    value: &str,
    options: &super::Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("panel report-metadata");
    check_scope(
        options,
        Scope {
            metadata: true,
            panel_metadata: true,
            output_source: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let invalid = |option, value: &str, expected| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option,
                value: value.to_owned(),
                expected,
            },
            envelope,
        )
    };
    let common = super::workspace_metadata::fields(options, context, envelope, false)?;
    let fields = &options.panel_metadata;
    if (fields.clear_title && fields.title.is_some())
        || (fields.clear_display_agent && fields.display_agent.is_some())
        || (fields.clear_state_labels && !fields.state_labels.is_empty())
    {
        return Err(invalid("report", "", ExpectedValue::MetadataPresentation));
    }

    let normalize =
        |value: &Option<String>| value.as_deref().and_then(MetadataTokenValue::normalize);
    let mut patch = PanelMetadataPatch {
        agent: fields
            .agent
            .as_deref()
            .map(|value| {
                crate::agent_provider::reported_label(value)
                    .and_then(|label| MetadataTokenValue::parse(&label).ok())
                    .ok_or_else(|| invalid("--agent", value, ExpectedValue::MetadataAgent))
            })
            .transpose()?,
        applies_to_source: fields
            .applies_to_source
            .as_deref()
            .map(|v| {
                MetadataSource::parse(v.trim())
                    .map_err(|_| invalid("--applies-to-source", v, ExpectedValue::MetadataSource))
            })
            .transpose()?,
        title: normalize(&fields.title),
        display_agent: normalize(&fields.display_agent),
        clear_title: fields.clear_title,
        clear_display_agent: fields.clear_display_agent,
        clear_state_labels: fields.clear_state_labels,
        tokens: common.tokens,
        ..Default::default()
    };
    for raw in &fields.state_labels {
        let (state, value) = raw
            .split_once('=')
            .ok_or_else(|| invalid("--state-label", raw, ExpectedValue::MetadataStateLabel))?;
        let state = match state.trim().to_ascii_lowercase().as_str() {
            "idle" => MetadataState::Idle,
            "working" => MetadataState::Working,
            "blocked" => MetadataState::Blocked,
            "done" => MetadataState::Done,
            "unknown" => MetadataState::Unknown,
            _ => {
                return Err(invalid(
                    "--state-label",
                    raw,
                    ExpectedValue::MetadataStateLabel,
                ));
            }
        };
        let value = MetadataTokenValue::normalize(value)
            .ok_or_else(|| invalid("--state-label", raw, ExpectedValue::MetadataStateLabel))?;
        patch.state_labels.insert(state, value);
    }
    patch
        .validate()
        .map_err(|_| invalid("report", "", ExpectedValue::MetadataPresentation))?;
    Ok(Request::PanelMetadata {
        command: PanelMetadataCommand {
            target: target(value.to_owned(), Resource::Panel, envelope)?,
            source: common.source,
            patch,
            sequence: common.sequence,
            ttl_milliseconds: common.ttl_milliseconds,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse_args(args: &[&str]) -> Result<Request, ParseFailure> {
        super::super::parse_request(args.iter().map(std::ffi::OsString::from))
    }
    #[test]
    fn agent_guard_is_normalized_losslessly_or_rejected_before_admission() {
        let report = |agent: &str| {
            parse_args(&[
                "panel",
                "report-metadata",
                "@panel:abc",
                "--source=hook",
                "--title=x",
                "--agent",
                agent,
            ])
        };
        for (raw, expected) in [
            (" CLAUDE-Code.exe ", "claude"),
            ("自定义 Agent", "自定义 Agent"),
        ] {
            let Request::PanelMetadata { command, .. } = report(raw).unwrap() else {
                panic!("report")
            };
            assert_eq!(command.patch.agent.unwrap().as_str(), expected);
        }
        let boundary = "界".repeat(80);
        let Request::PanelMetadata { command, .. } = report(&boundary).unwrap() else {
            panic!("report")
        };
        assert_eq!(command.patch.agent.unwrap().as_str(), boundary);
        for raw in [
            "".to_owned(),
            " \t".to_owned(),
            "foo\tbar".to_owned(),
            "界".repeat(81),
        ] {
            assert!(report(&raw).is_err(), "{raw:?}");
        }
    }

    #[test]
    fn whole_report_normalization_and_conflicts_precede_effects() {
        let Request::PanelMetadata { command, .. } = parse_args(&[
            "panel",
            "report-metadata",
            "@panel:abc",
            "--source= hook:agent ",
            "--title=  标题\t ",
            "--display-agent= Agent ",
            "--state-label= WORKING =忙碌",
            "--state-label=working=更新",
            "--token=x=y",
            "--clear-token=x",
            "--seq=18446744073709551615",
            "--ttl-ms=86400000",
        ])
        .unwrap() else {
            panic!("report")
        };
        assert_eq!(command.patch.title.unwrap().as_str(), "标题");
        assert_eq!(
            command.patch.state_labels[&MetadataState::Working].as_str(),
            "更新"
        );
        assert!(
            command
                .patch
                .tokens
                .unwrap()
                .as_map()
                .values()
                .all(Option::is_none)
        );
        for tail in [
            vec![],
            vec!["--agent=codex"],
            vec!["--title= "],
            vec!["--state-label=working="],
            vec!["--state-label=nope=text"],
            vec!["--title=x", "--clear-title"],
            vec!["--title=", "--clear-title"],
            vec!["--display-agent= ", "--clear-display-agent"],
            vec!["--display-agent=x", "--clear-display-agent"],
            vec!["--state-label=done=x", "--clear-state-labels"],
            vec!["--clear-title", "--clear-title"],
            vec!["--title=x", "--seq=-1"],
            vec!["--clear-title", "--ttl-ms=0"],
            vec!["--title=x", "--focus"],
            vec!["--title=x", "--client=00000000000000000000000000000001"],
            vec!["--title=x", "--applies-to-source=bad source"],
        ] {
            let args = [
                vec!["panel", "report-metadata", "@panel:abc", "--source=hook"],
                tail,
            ]
            .concat();
            assert!(parse_args(&args).is_err(), "{args:?}");
        }
        for flag in [
            "--clear-title",
            "--clear-display-agent",
            "--clear-state-labels",
        ] {
            assert!(
                parse_args(&[
                    "panel",
                    "report-metadata",
                    "@panel:abc",
                    "--source=hook",
                    flag
                ])
                .is_ok()
            );
        }
        for args in [
            vec!["panel", "report-metadata", "@panel:abc", "--title=x"],
            vec!["panel", "get", "@panel:abc", "--title=x"],
            vec![
                "workspace",
                "report-metadata",
                "main",
                "--source=hook",
                "--token=x=y",
                "--title=x",
            ],
            vec!["--title=x", "--help"],
        ] {
            assert!(parse_args(&args).is_err(), "{args:?}");
        }
    }
}
