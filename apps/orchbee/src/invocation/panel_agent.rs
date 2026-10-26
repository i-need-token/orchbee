//! Validate explicit Agent reports before runtime discovery; process proof stays Server-owned.
use super::{
    ExpectedValue, FailureEnvelope, OutputFormat, ParseFailure, Request, Resource, Target,
    UsageContext, UsageReason, check_scope, missing_required, target, usage_with,
};
use orchbee_agent_observation_protocol::{AgentReportSequence, normalize_agent_report_label};
use orchbee_workbench_protocol::{
    PanelAgentReport, PanelAgentRequest, PresentationBinding, ProtocolVersion, SessionId,
    TerminalId, WorkbenchPanelId, WorkbenchRequest, WorkbenchRequestId, WorkerId,
    write_workbench_request,
};

#[derive(Default)]
pub(super) struct Options {
    pub(super) state: Option<String>,
    pub(super) message: Option<String>,
}
impl Options {
    fn flags(&self) -> [(bool, &'static str); 2] {
        [
            (self.state.is_some(), "--state"),
            (self.message.is_some(), "--message"),
        ]
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PanelAgentCommand {
    pub(crate) target: Target,
    pub(crate) presentation_id: Option<orchbee_workbench_protocol::TerminalPresentationId>,
    pub(crate) report: PanelAgentReport,
}
impl PanelAgentCommand {
    pub(crate) fn action(
        &self,
        panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
        expected_binding: orchbee_workbench_protocol::PresentationBinding,
    ) -> orchbee_workbench_protocol::PanelAgentAction {
        use orchbee_workbench_protocol::PanelAgentAction;
        match &self.presentation_id {
            Some(presentation_id) => PanelAgentAction::ReportPresentation {
                panel_id,
                presentation_id: presentation_id.clone(),
                expected_binding,
                report: Box::new(self.report.clone()),
            },
            None => PanelAgentAction::Report {
                panel_id,
                expected_binding,
                report: Box::new(self.report.clone()),
            },
        }
    }
}
pub(super) fn parse(
    verb: &str,
    value: &str,
    options: &super::Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let path = match verb {
        "report-agent" => "panel report-agent",
        "release-agent" => "panel release-agent",
        _ => "panel clear-agent-authority",
    };
    let context = UsageContext::Path(path);
    check_scope(options, context, envelope)?;
    let fields = &options.panel_agent;
    for (seen, flag) in fields
        .flags()
        .into_iter()
        .chain([(options.panel_metadata.agent.is_some(), "--agent")])
    {
        let allowed = match verb {
            "report-agent" => true,
            "release-agent" => flag == "--agent",
            _ => false,
        };
        if seen && !allowed {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context,
                    option: flag,
                },
                envelope,
            ));
        }
    }
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
    let sequence = options
        .metadata
        .sequence
        .as_deref()
        .map(|raw| {
            raw.parse::<u64>()
                .map(AgentReportSequence::new)
                .map_err(|_| invalid("--seq", raw, ExpectedValue::MetadataSequence))
        })
        .transpose()?;
    let report = if verb == "clear-agent-authority" {
        // Omitted and explicitly empty source select distinct clear operations.
        PanelAgentReport::Clear {
            source: options.output_source.clone(),
            sequence,
        }
    } else {
        let source = options
            .output_source
            .as_deref()
            .ok_or_else(|| missing_required(context, "--source", envelope))?
            .trim();
        if source.is_empty() {
            return Err(invalid("--source", source, ExpectedValue::ReportSource));
        }
        let raw = options
            .panel_metadata
            .agent
            .as_deref()
            .ok_or_else(|| missing_required(context, "--agent", envelope))?;
        let agent = normalize_agent_report_label(raw)
            .ok_or_else(|| invalid("--agent", raw, ExpectedValue::ReportAgent))?;
        match verb {
            "report-agent" => {
                let raw = fields
                    .state
                    .as_deref()
                    .ok_or_else(|| missing_required(context, "--state", envelope))?;
                let state = super::grammar::values::panel_agent_state(raw)
                    .ok_or_else(|| invalid("--state", raw, ExpectedValue::ReportState))?;
                PanelAgentReport::State {
                    source: source.into(),
                    agent,
                    state,
                    message: fields.message.clone(),
                    sequence,
                }
            }
            _ => PanelAgentReport::Release {
                source: source.into(),
                agent,
                sequence,
            },
        }
    };
    let command = PanelAgentCommand {
        target: target(value.to_owned(), Resource::Panel, envelope)?,
        presentation_id: super::report_target::parse(options, context, envelope)?,
        report,
    };
    preflight(&command).map_err(|_| invalid("report", "", ExpectedValue::ReportSize))?;
    Ok(Request::PanelAgent {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}
fn preflight(
    command: &PanelAgentCommand,
) -> Result<(), orchbee_workbench_protocol::WorkbenchWireError> {
    // Every live identity has this fixed encoded width. Validate the complete frame before connecting.
    const ID: &str = "00000000000000000000000000000001";
    write_workbench_request(
        &mut Vec::new(),
        &WorkbenchRequest::PanelAgent(PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(ID).expect("fixed identity"),
            session_id: SessionId::parse(ID).expect("fixed identity"),
            action: command.action(
                WorkbenchPanelId::parse(ID).expect("fixed identity"),
                PresentationBinding {
                    worker_id: WorkerId::parse(ID).expect("fixed identity"),
                    terminal_id: TerminalId::parse(ID).expect("fixed identity"),
                },
            ),
        }),
    )
}

#[cfg(test)]
mod tests;
