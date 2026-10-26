//! Panel display reports are normalized and validated before runtime admission.
use super::{
    ExpectedValue, FailureEnvelope, OutputFormat, ParseFailure, Request, Resource, Target,
    UsageContext, UsageReason, check_scope, target, usage_with,
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
        self.first_option_with_agent(true)
    }
    pub(super) fn first_option_with_agent(&self, include_agent: bool) -> Option<&'static str> {
        [
            (include_agent && self.agent.is_some(), "--agent"),
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
    pub(crate) presentation_id: Option<orchbee_workbench_protocol::TerminalPresentationId>,
    pub(crate) source: MetadataSource,
    pub(crate) patch: PanelMetadataPatch,
    pub(crate) sequence: Option<WorkbenchRevision>,
    pub(crate) ttl_milliseconds: Option<u32>,
}
impl PanelMetadataCommand {
    pub(crate) fn action(
        &self,
        panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
        expected_binding: orchbee_workbench_protocol::PresentationBinding,
    ) -> orchbee_workbench_protocol::PanelMetadataAction {
        use orchbee_workbench_protocol::PanelMetadataAction;
        match &self.presentation_id {
            Some(presentation_id) => PanelMetadataAction::ReportPresentation {
                panel_id,
                presentation_id: presentation_id.clone(),
                expected_binding,
                source: self.source.clone(),
                patch: Box::new(self.patch.clone()),
                sequence: self.sequence,
                ttl_milliseconds: self.ttl_milliseconds,
            },
            None => PanelMetadataAction::Report {
                panel_id,
                expected_binding,
                source: self.source.clone(),
                patch: Box::new(self.patch.clone()),
                sequence: self.sequence,
                ttl_milliseconds: self.ttl_milliseconds,
            },
        }
    }
}
pub(super) fn parse(
    value: &str,
    options: &super::Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("panel report-metadata");
    check_scope(options, context, envelope)?;
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
            "error" => MetadataState::Error,
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
    let command = PanelMetadataCommand {
        target: target(value.to_owned(), Resource::Panel, envelope)?,
        presentation_id: super::report_target::parse(options, context, envelope)?,
        source: common.source,
        patch,
        sequence: common.sequence,
        ttl_milliseconds: common.ttl_milliseconds,
    };
    preflight(&command).map_err(|_| invalid("report", "", ExpectedValue::MetadataPatchSize))?;
    Ok(Request::PanelMetadata {
        command,
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

fn preflight(
    command: &PanelMetadataCommand,
) -> Result<(), orchbee_workbench_protocol::WorkbenchWireError> {
    use orchbee_workbench_protocol::{
        PanelMetadataRequest, PresentationBinding, ProtocolVersion, SessionId, TerminalId,
        WorkbenchPanelId, WorkbenchRequest, WorkbenchRequestId, WorkerId, write_workbench_request,
    };
    const ID: &str = "00000000000000000000000000000001";
    write_workbench_request(
        &mut Vec::new(),
        &WorkbenchRequest::PanelMetadata(PanelMetadataRequest {
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
