//! Application receipts retain canonical and Client acknowledgements without launch secrets.
use crate::{
    cli::{Failure, FailureKind, Output, failure_output, output},
    invocation::OutputFormat,
    localization::Locale,
};
use orchbee_workbench_protocol::WorkbenchTabId;
use serde_json::{Value, json};

pub(super) struct Receipt {
    pub(super) stage: &'static str,
    pub(super) source: Option<Value>,
    pub(super) old_tab: Option<WorkbenchTabId>,
    pub(super) new_tab: Option<WorkbenchTabId>,
    pub(super) effects: Vec<Value>,
    pub(super) recovery: Value,
    pub(super) layout: Option<Value>,
}
impl Receipt {
    pub(super) fn new() -> Self {
        Self {
            stage: "input",
            source: None,
            old_tab: None,
            new_tab: None,
            effects: vec![],
            recovery: json!({"outcome":"not-needed"}),
            layout: None,
        }
    }
    pub(super) fn record(&mut self, phase: &str, document: Value) {
        let entry = json!({"phase":phase,"document":document});
        if phase == "replace-status"
            && self
                .effects
                .last()
                .is_some_and(|last| last["phase"] == phase)
        {
            *self.effects.last_mut().expect("last status") = entry;
        } else {
            self.effects.push(entry);
        }
    }
    pub(super) fn emit(
        &self,
        failure: Option<Failure>,
        format: OutputFormat,
        locale: Locale,
    ) -> Output {
        let status = failure.as_ref().map_or(0, |value| value.kind.exit_code());
        let document = json!({"schema":"orchbee.layout.apply","version":2,"source":self.source,
            "result":{"outcome":if failure.is_some(){"incomplete"}else{"completed"},"stage":self.stage,
                "oldTabId":self.old_tab,"newTabId":self.new_tab,"effects":self.effects,
                "recovery":self.recovery,"layout":self.layout,"failure":failure.as_ref().map(Failure::observation)}});
        let encoded = match format {
            OutputFormat::Json => output::encode_document(&document),
            OutputFormat::Text => {
                let mut text = locale.cli_layout_apply(&document);
                if let Some(failure) = &failure {
                    // Preserve source-specific diagnostics, including escaped file errors.
                    text.push_str(&locale.cli_failure(failure));
                }
                output::encode_text(&text)
            }
        };
        match encoded {
            Ok(bytes) => {
                if status != 0 && format == OutputFormat::Text {
                    Output {
                        status,
                        stdout: vec![],
                        stderr: String::from_utf8(bytes).expect("UTF-8 text"),
                        finalization: None,
                    }
                } else {
                    Output {
                        status,
                        stdout: bytes,
                        stderr: String::new(),
                        finalization: None,
                    }
                }
            }
            _ => failure_output(
                Failure::new(FailureKind::OutputTooLarge),
                Some(format),
                locale,
            ),
        }
    }
}

#[cfg(test)]
mod tests;
