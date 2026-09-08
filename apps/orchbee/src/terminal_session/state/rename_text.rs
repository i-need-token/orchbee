use orchbee_workbench_protocol::{ContractError, MAXIMUM_LABEL_LENGTH, WorkbenchLabel};

/// Exact editable text only; input decoding and command admission belong to its consumer.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct RenameText(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct RenameTextLimit;

impl RenameText {
    pub(in crate::terminal_session) fn from_label(label: &WorkbenchLabel) -> Self {
        Self(label.as_str().to_owned())
    }

    pub(in crate::terminal_session) fn as_str(&self) -> &str {
        &self.0
    }

    pub(in crate::terminal_session) fn append(
        &mut self,
        text: &str,
    ) -> Result<(), RenameTextLimit> {
        let remaining = MAXIMUM_LABEL_LENGTH - self.0.chars().count();
        if text.chars().take(remaining + 1).count() > remaining {
            return Err(RenameTextLimit);
        }
        self.0.push_str(text);
        Ok(())
    }

    pub(in crate::terminal_session) fn remove_last_scalar(&mut self) -> bool {
        self.0.pop().is_some()
    }

    pub(in crate::terminal_session) fn clear(&mut self) {
        self.0.clear();
    }

    pub(in crate::terminal_session) fn label(&self) -> Result<WorkbenchLabel, ContractError> {
        WorkbenchLabel::parse(&self.0)
    }
}
