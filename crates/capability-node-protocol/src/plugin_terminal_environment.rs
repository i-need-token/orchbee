//! Bounded plugin display metadata, separate from caller overrides and runtime authority.
use crate::ProtocolError;
use std::fmt;
pub const MAXIMUM_PLUGIN_TERMINAL_ENVIRONMENT_BYTES: usize = 24 * 1024;
pub const PLUGIN_TERMINAL_ENVIRONMENT_NAMES: [&str; 10] = [
    "ORCHBEE_PLUGIN_ID",
    "ORCHBEE_PLUGIN_ROOT",
    "ORCHBEE_PLUGIN_ENTRYPOINT_ID",
    "ORCHBEE_PLUGIN_CONFIG_DIR",
    "ORCHBEE_PLUGIN_STATE_DIR",
    "ORCHBEE_PLUGIN_CONTEXT_JSON",
    "ORCHBEE_BIN_PATH",
    "ORCHBEE_WORKSPACE_ID",
    "ORCHBEE_TAB_ID",
    "ORCHBEE_PANE_ID",
];
#[derive(Clone, Eq, PartialEq)]
pub struct PluginTerminalEnvironment(Vec<(String, String)>);
impl PluginTerminalEnvironment {
    pub fn new(entries: Vec<(String, String)>) -> Result<Self, ProtocolError> {
        let mut present = [false; 10];
        let mut bytes = 0usize;
        for (name, value) in &entries {
            let Some(index) = PLUGIN_TERMINAL_ENVIRONMENT_NAMES
                .iter()
                .position(|key| *key == name)
            else {
                return Err(ProtocolError::InvalidPayload);
            };
            if present[index] || value.is_empty() || value.contains('\0') {
                return Err(ProtocolError::InvalidPayload);
            }
            present[index] = true;
            bytes = bytes
                .checked_add(name.len())
                .and_then(|n| n.checked_add(value.len()))
                .ok_or(ProtocolError::InvalidPayload)?;
            if bytes > MAXIMUM_PLUGIN_TERMINAL_ENVIRONMENT_BYTES {
                return Err(ProtocolError::InvalidPayload);
            }
        }
        if !present[..7].iter().all(|value| *value) {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(entries))
    }
    pub fn entries(&self) -> &[(String, String)] {
        &self.0
    }
}
impl fmt::Debug for PluginTerminalEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PluginTerminalEnvironment")
            .field(
                "names",
                &self.0.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
