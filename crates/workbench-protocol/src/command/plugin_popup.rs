//! Popup launch data is bounded display/selector input, never physical authority.
use crate::{
    ClientPresentationSelection, ContractError, LaunchEnvironment, PluginInvocationContext,
    PluginPaneSelector, WorkbenchDirectoryPath, WorkbenchTabId, WorkbenchTerminalContextId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PluginPopupSize {
    Cells(u16),
    Percent(u8),
}
impl PluginPopupSize {
    pub fn validate(self) -> Result<(), ContractError> {
        if matches!(self, Self::Percent(value) if !(1..=100).contains(&value)) {
            Err(ContractError::InvalidDocument)
        } else {
            Ok(())
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        let size = if let Some(percent) = value.strip_suffix('%') {
            Self::Percent(percent.parse().ok()?)
        } else {
            Self::Cells(value.parse().ok()?)
        };
        size.validate().ok().map(|()| size)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientPopupSource {
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub tab_id: WorkbenchTabId,
    pub panel_id: crate::WorkbenchPanelId,
    pub member: ClientPresentationSelection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginPopupSpec {
    pub selector: PluginPaneSelector,
    pub source: ClientPopupSource,
    pub context: Box<PluginInvocationContext>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub cwd: Option<WorkbenchDirectoryPath>,
    pub environment: LaunchEnvironment,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub width: Option<PluginPopupSize>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub height: Option<PluginPopupSize>,
}
impl PluginPopupSpec {
    pub fn validate(&self) -> Result<(), ContractError> {
        PluginPaneSelector::new(
            self.selector.plugin_id.clone(),
            self.selector.entrypoint_id.clone(),
        )
        .map_err(|_| ContractError::InvalidDocument)?;
        self.context.validate()?;
        self.source.member.validate()?;
        for size in [self.width, self.height].into_iter().flatten() {
            size.validate()?;
        }
        if self.context.tab_id.as_deref() != Some(self.source.tab_id.as_str())
            || self.context.focused_pane_id.as_deref() != Some(self.source.panel_id.as_str())
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
