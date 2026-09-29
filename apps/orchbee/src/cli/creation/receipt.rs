use crate::{
    cli::{Failure, FailureKind, Output, client_control::Answer, failure_output, mutation, output},
    invocation::OutputFormat,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientSplitRatio, ClientViewControlStatus, ClientViewOutcome, WorkbenchClientId,
};
use serde_json::{Value, json};

pub(super) struct Receipt {
    pub(super) directory: super::directory::Resolution,
    pub(super) canonical: mutation::Receipt,
    pub(super) client_id: Option<WorkbenchClientId>,
    pub(super) requested_ratio: Option<ClientSplitRatio>,
    pub(super) requested_focus: bool,
    pub(super) ratio: Option<Answer>,
    pub(super) focus: Option<Answer>,
    pub(super) stage: &'static str,
    pub(super) failure: Option<FailureKind>,
}

pub(super) fn applied(answer: &Answer, ratio: bool) -> bool {
    answer.exit_code() == 0
        && matches!(answer.reply.as_ref().map(|reply| &reply.outcome),
        Some(ClientViewOutcome::Operation { operation }) if if ratio {
            matches!(operation.status, ClientViewControlStatus::LayoutAdjusted { .. })
        } else { matches!(operation.status, ClientViewControlStatus::Applied { .. }) })
}

impl Receipt {
    fn complete(&self) -> bool {
        self.failure.is_none()
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
    }
    fn exit_code(&self) -> u8 {
        if let Some(failure) = self.failure {
            return failure.exit_code();
        }
        if self.complete() {
            return 0;
        }
        for answer in [&self.ratio, &self.focus].into_iter().flatten() {
            let status = answer.exit_code();
            if status != 0 {
                return status;
            }
        }
        3
    }
    pub(super) fn document(&self) -> Value {
        let mut document = self.canonical.document();
        document["schema"] = json!("orchbee.creation.result");
        document["version"] = json!(3);
        document["directory"] = self.directory.document();
        document["postCreate"] = json!({
            "clientId":self.client_id,
            "requested":{"ratio":self.requested_ratio,"focus":self.requested_focus},
            "ratio":self.ratio.as_ref().map(Answer::document),
            "focus":self.focus.as_ref().map(Answer::document),
            "stage":self.stage,
            "failure":self.failure.map(FailureKind::as_str),
            "outcome":if !self.complete() { "incomplete" } else if self.requested_ratio.is_none() && !self.requested_focus { "not-requested" } else { "completed" },
        });
        document
    }
    pub(super) fn emit(&self, format: OutputFormat, locale: Locale) -> Output {
        let document = self.document();
        let status = self.exit_code();
        let encoded = match format {
            OutputFormat::Json => output::encode_document(&document),
            OutputFormat::Text => {
                let mut text = locale.cli_creation(&document);
                if let Some(kind) = self.failure {
                    text.push_str(&locale.cli_failure(&Failure::new(kind)));
                }
                output::encode_text(&text)
            }
        };
        match encoded {
            Ok(bytes) if status != 0 && format == OutputFormat::Text => Output {
                status,
                stdout: vec![],
                stderr: String::from_utf8(bytes).expect("encoded text is UTF-8"),
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
