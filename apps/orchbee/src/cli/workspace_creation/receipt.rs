use crate::{
    cli::{Failure, FailureKind, Output, client_control::Answer, failure_output, mutation, output},
    invocation::OutputFormat,
    localization::Locale,
};
use orchbee_workbench_protocol::{ClientViewControlStatus, ClientViewOutcome, WorkbenchClientId};
use serde_json::{Value, json};

pub(super) struct Receipt {
    pub(super) project: Option<mutation::Receipt>,
    pub(super) workspace: Option<mutation::Receipt>,
    pub(super) directory: super::source::Directory,
    pub(super) client_id: Option<WorkbenchClientId>,
    pub(super) requested_focus: bool,
    pub(super) focus: Option<Answer>,
    pub(super) stage: &'static str,
    pub(super) failure: Option<Failure>,
}
impl Receipt {
    pub(super) fn complete(&self) -> bool {
        self.failure.is_none() && self.workspace.as_ref().is_some_and(|receipt| receipt.launch_failure().is_none()) && (!self.requested_focus || self.focus.as_ref().is_some_and(|answer| answer.exit_code()==0 && matches!(answer.reply.as_ref().map(|reply| &reply.outcome),Some(ClientViewOutcome::Operation {operation}) if matches!(operation.status,ClientViewControlStatus::Applied {..}))))
    }
    fn status(&self) -> u8 {
        if let Some(error) = &self.failure {
            error.kind.exit_code()
        } else if self.complete() {
            0
        } else {
            self.focus.as_ref().map_or(3, |answer| {
                let code = answer.exit_code();
                if code == 0 { 3 } else { code }
            })
        }
    }
    pub(super) fn document(&self) -> Value {
        json!({"schema":"orchbee.workspace.creation","version":1,"project":self.project.as_ref().map(mutation::Receipt::document),"workspace":self.workspace.as_ref().map(mutation::Receipt::document),"directory":self.directory.document(),"clientId":self.client_id,"requestedFocus":self.requested_focus,"focus":self.focus.as_ref().map(Answer::document),"stage":self.stage,"failure":self.failure.as_ref().map(Failure::observation),"outcome":if self.complete() {"completed"} else {"incomplete"}})
    }
    pub(super) fn emit(&self, format: OutputFormat, locale: Locale) -> Output {
        let document = self.document();
        let status = self.status();
        let bytes = match format {
            OutputFormat::Json => output::encode_document(&document),
            OutputFormat::Text => {
                let mut text = locale.cli_workspace_creation(&document);
                if let Some(failure) = &self.failure {
                    text.push_str(&locale.cli_failure(failure));
                }
                output::encode_text(&text)
            }
        };
        match bytes {
            Ok(bytes) if status != 0 && format == OutputFormat::Text => Output {
                status,
                stdout: vec![],
                stderr: String::from_utf8(bytes).expect("encoded text"),
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
