//! Projects canonical Panel-swap and subsequent Client-focus outcomes into one receipt and
//! exit status while preserving partial success. It executes no swap or focus request and
//! never treats logical commitment as visible presentation.

use super::{
    Answer, ClientPanelDirection, ClientViewControlStatus, ClientViewIntent, ClientViewOutcome,
    Failure, FailureKind, Locale, Output, OutputFormat, SessionId, WorkbenchClientId,
    WorkbenchCommandOutcome, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTerminalContextId, failure_output,
};
use crate::cli_text::CliText;
use serde_json::{Value, json};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(super) struct Panel {
    pub(super) terminal_context_id: WorkbenchTerminalContextId,
    pub(super) tab_id: WorkbenchTabId,
    pub(super) panel_id: WorkbenchPanelId,
}
impl Panel {
    fn document(&self) -> Value {
        json!({"terminalContextId":self.terminal_context_id,"tabId":self.tab_id,"panelId":self.panel_id})
    }

    pub(super) fn intent(&self) -> ClientViewIntent {
        ClientViewIntent::Panel {
            terminal_context_id: self.terminal_context_id.clone(),
            tab_id: self.tab_id.clone(),
            panel_id: self.panel_id.clone(),
        }
    }
    pub(super) fn same_tab(&self, other: &Self) -> bool {
        self.terminal_context_id == other.terminal_context_id && self.tab_id == other.tab_id
    }
}

pub(super) enum Canonical {
    NotSubmitted,
    Unchanged {
        revision: WorkbenchRevision,
    },
    Refused {
        outcome: Box<WorkbenchCommandOutcome>,
    },
    Committed {
        revision: WorkbenchRevision,
    },
    Unknown,
}

impl Canonical {
    fn document(&self) -> Value {
        match self {
            Self::NotSubmitted => json!({"kind":"not-submitted"}),
            Self::Unchanged { revision } => json!({"kind":"unchanged","revision":revision}),
            Self::Committed { revision } => json!({"kind":"committed","revision":revision}),
            Self::Refused { outcome } => json!({"kind":"refused","outcome":outcome}),
            Self::Unknown => json!({"kind":"unknown"}),
        }
    }
}
fn focus_document(answer: &Answer) -> Value {
    let document = answer.document();
    let observation = if answer.unknown {
        json!({"kind":"unknown"})
    } else if let Some(reply) = &answer.reply {
        let result = match &reply.outcome {
            ClientViewOutcome::Operation { operation } => json!(operation.status),
            outcome => json!(outcome),
        };
        json!({"kind":"server","result":result})
    } else {
        json!({"kind":"not-submitted"})
    };
    json!({"operationId":answer.operation_id,"observation":observation,"wait":document["result"]["wait"]})
}

pub(super) struct Receipt {
    pub(super) session_id: SessionId,
    pub(super) client_id: Option<WorkbenchClientId>,
    pub(super) base_revision: WorkbenchRevision,
    pub(super) source: Panel,
    pub(super) target: Option<Panel>,
    pub(super) direction: Option<ClientPanelDirection>,
    pub(super) layout_operation_id: Option<WorkbenchRequestId>,
    pub(super) layout_revision: Option<WorkbenchRevision>,
    pub(super) request_id: Option<WorkbenchRequestId>,
    pub(super) canonical: Canonical,
    pub(super) focus: Option<Answer>,
    pub(super) stage: &'static str,
    pub(super) reason: Option<&'static str>,
    pub(super) failure: Option<FailureKind>,
}
impl Receipt {
    pub(super) fn focused(&self) -> bool {
        self.focus.as_ref().is_some_and(|answer| {
            answer.exit_code() == 0
                && matches!(answer.reply.as_ref().map(|reply| &reply.outcome),
                Some(ClientViewOutcome::Operation { operation })
                if matches!(operation.status, ClientViewControlStatus::Applied { .. }))
        })
    }
    pub(super) fn outcome(&self) -> &'static str {
        match &self.canonical {
            Canonical::Committed { .. }
                if (self.client_id.is_none() || self.focused()) && self.failure.is_none() =>
            {
                "swapped"
            }
            Canonical::Committed { .. } => "incomplete",
            Canonical::Unchanged { .. } => "unchanged",
            Canonical::Unknown => "unknown",
            Canonical::Refused { .. } | Canonical::NotSubmitted => "not-swapped",
        }
    }
    pub(super) fn exit_code(&self) -> u8 {
        if let Some(kind) = self.failure {
            return kind.exit_code();
        }
        match &self.canonical {
            Canonical::Unchanged { .. } => 0,
            Canonical::Unknown => 8,
            Canonical::Committed { .. } if self.client_id.is_none() || self.focused() => 0,
            Canonical::Committed { .. } => self.focus.as_ref().map_or(3, |answer| {
                let status = answer.exit_code();
                if status == 0 { 3 } else { status }
            }),
            Canonical::Refused { .. } => 3,
            Canonical::NotSubmitted => 1,
        }
    }
    pub(super) fn document(&self) -> Value {
        let document = json!({"schema":"orchbee.panel.swap","version":1,"result":{"sessionId":self.session_id,
            "clientId":self.client_id,"baseRevision":self.base_revision,
            "scope":if self.client_id.is_some() {"client"} else {"shared"},
            "source":self.source.document(),"target":self.target.as_ref().map(Panel::document),"direction":self.direction,
            "layoutOperationId":self.layout_operation_id,"layoutRevision":self.layout_revision,
            "requestId":self.request_id,"canonical":self.canonical.document(),
            "focus":self.focus.as_ref().map(focus_document),"stage":self.stage,
            "reason":self.reason,"failure":self.failure.map(FailureKind::as_str),
            "outcome":self.outcome()}});
        document
    }
    pub(super) fn emit(&self, format: OutputFormat, locale: Locale) -> Output {
        let status = self.exit_code();
        let document = self.document();
        let encoded = match format {
            OutputFormat::Json => crate::cli::output::encode_document(&document),
            OutputFormat::Text => {
                let mut text = CliText::new(locale).cli_panel_swap(&document);
                if let Some(kind) = self.failure {
                    text.push_str(&CliText::new(locale).cli_failure(&Failure::new(kind)));
                }
                crate::cli::output::encode_text(&text)
            }
        };
        match encoded {
            Ok(bytes) => Output {
                status,
                stdout: if format == OutputFormat::Text && status != 0 {
                    Vec::new()
                } else {
                    bytes.clone()
                },
                stderr: if format == OutputFormat::Text && status != 0 {
                    String::from_utf8(bytes).expect("localized output is UTF-8")
                } else {
                    String::new()
                },
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
