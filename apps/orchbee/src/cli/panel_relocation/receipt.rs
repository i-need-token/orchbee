use super::targeting::Panel;
use crate::{
    cli::{Failure, FailureKind, Output, client_control::Answer, failure_output, mutation, output},
    invocation::OutputFormat,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientPanelLayout, ClientSplitRatio, ClientViewControlStatus, ClientViewOutcome, SiteId,
    WorkbenchClientId, WorkbenchDirectoryPath, WorkbenchRevision,
};
use serde_json::{Value, json};

pub(super) struct Receipt {
    pub(super) site_id: SiteId,
    pub(super) base_revision: WorkbenchRevision,
    pub(super) source: Panel,
    pub(super) client_id: Option<WorkbenchClientId>,
    pub(super) requested_focus: bool,
    pub(super) requested_ratio: Option<ClientSplitRatio>,
    pub(super) directory_path: Option<WorkbenchDirectoryPath>,
    pub(super) directory_source: Option<&'static str>,
    pub(super) movement: Option<mutation::Receipt>,
    pub(super) directory: Option<orchbee_workbench_protocol::Directory>,
    pub(super) ratio: Option<Answer>,
    pub(super) focus: Option<Answer>,
    pub(super) source_layout: Option<ClientPanelLayout>,
    pub(super) target_layout: Option<ClientPanelLayout>,
    pub(super) stage: &'static str,
    pub(super) reason: Option<&'static str>,
    pub(super) failure: Option<Failure>,
}
pub(super) fn applied(answer: &Answer, ratio: bool) -> bool {
    answer.exit_code() == 0
        && matches!(answer.reply.as_ref().map(|reply| &reply.outcome),
        Some(ClientViewOutcome::Operation { operation }) if if ratio {
            matches!(operation.status, ClientViewControlStatus::LayoutAdjusted { .. })
        } else { matches!(operation.status, ClientViewControlStatus::Applied { .. }) })
}
impl Receipt {
    fn status(&self) -> u8 {
        if self
            .failure
            .as_ref()
            .is_some_and(|failure| failure.kind == FailureKind::OutcomeUnknown)
            || [&self.ratio, &self.focus]
                .into_iter()
                .flatten()
                .any(|answer| answer.exit_code() == 8)
        {
            return 8;
        }
        if let Some(failure) = &self.failure {
            return failure.kind.exit_code();
        }
        if self.stage == "complete" {
            if self.reason.is_some() && self.movement.is_none() {
                return 0;
            }
            if self.movement.is_some()
                && self.reason.is_none()
                && (self.directory_path.is_none() || self.directory.is_some())
                && (self.requested_ratio.is_none()
                    || self
                        .ratio
                        .as_ref()
                        .is_some_and(|answer| applied(answer, true)))
                && (!self.requested_focus
                    || self
                        .focus
                        .as_ref()
                        .is_some_and(|answer| applied(answer, false)))
            {
                return 0;
            }
        }
        for answer in [&self.ratio, &self.focus].into_iter().flatten() {
            if answer.exit_code() != 0 {
                return answer.exit_code();
            }
        }
        FailureKind::Rejected.exit_code()
    }
    pub(super) fn document(&self) -> Value {
        let outcome = if self.status() == 8 {
            "outcome-unknown"
        } else if self.status() != 0 {
            "incomplete"
        } else if self.movement.is_none() {
            "unchanged"
        } else {
            "completed"
        };
        json!({"schema":"orchbee.panel.move","version":1,"siteId":self.site_id,"baseRevision":self.base_revision,
            "source":{"terminalContextId":self.source.terminal_context_id,"tabId":self.source.tab_id,"panelId":self.source.panel_id},"clientId":self.client_id,"requestedFocus":self.requested_focus,"requestedRatio":self.requested_ratio,
            "directoryPath":self.directory_path,"directorySource":self.directory_source,
            "movement":self.movement.as_ref().map(mutation::Receipt::document),
            "directory":self.directory,
            "ratio":self.ratio.as_ref().map(Answer::document),"focus":self.focus.as_ref().map(Answer::document),
            "sourceLayout":self.source_layout,"targetLayout":self.target_layout,"stage":self.stage,"reason":self.reason,
            "failure":self.failure.as_ref().map(Failure::observation),"outcome":outcome})
    }
    pub(super) fn emit(&self, format: OutputFormat, locale: Locale) -> Output {
        let document = self.document();
        let status = self.status();
        let encoded = match format {
            OutputFormat::Json => output::encode_document(&document),
            OutputFormat::Text => {
                let mut text = locale.cli_panel_relocation(&document);
                if let Some(failure) = &self.failure {
                    text.push_str(&locale.cli_failure(failure));
                }
                output::encode_text(&text)
            }
        };
        match encoded {
            Ok(bytes) if status != 0 && format == OutputFormat::Text => Output {
                status,
                stdout: vec![],
                stderr: String::from_utf8(bytes).expect("text"),
                finalization: None,
            },
            Ok(stdout) => Output {
                status,
                stdout,
                stderr: String::new(),
                finalization: None,
            },
            Err(_) => failure_output(
                Failure::new(FailureKind::OutputTooLarge),
                Some(format),
                locale,
            ),
        }
    }
}
