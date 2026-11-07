//! Retained per-Presentation directory intent; physical observation admission belongs to the Session.
use super::super::terminal_membership::visit_panel_presentations;
use super::{WorkbenchAuthority, WorkbenchCheckpointError as Error};
use orchbee_workbench_protocol::{
    PresentationBinding, Snapshot, TerminalPresentationId, WorkbenchDirectoryPath, WorkerId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StoredDirectory {
    pub(super) presentation_id: TerminalPresentationId,
    pub(super) path: WorkbenchDirectoryPath,
    /// The kept recent output this Presentation replays after a restart; omitted when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) output: Option<OutputToken>,
}

/// Names one kept terminal output file: a nonzero token written as 16 lowercase hex digits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OutputToken(pub(super) u64);
impl Serialize for OutputToken {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:016x}", self.0))
    }
}
impl<'de> Deserialize<'de> for OutputToken {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() != 16
            || !text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(serde::de::Error::custom("invalid output token"));
        }
        match u64::from_str_radix(&text, 16) {
            Ok(value) if value != 0 => Ok(Self(value)),
            _ => Err(serde::de::Error::custom("invalid output token")),
        }
    }
}
pub(super) fn validate(rows: &[StoredDirectory], snapshot: &Snapshot) -> Result<(), Error> {
    let mut retained = members(snapshot);
    for row in rows {
        if retained.remove(&row.presentation_id).is_none() {
            return Err(Error::InvalidTerminalDirectory);
        }
    }
    Ok(())
}
/// `outputs = None` means replay is off: no row names output, not even a held one's.
pub(super) fn capture(
    authority: &WorkbenchAuthority,
    worker: &WorkerId,
    observed: &[(PresentationBinding, WorkbenchDirectoryPath)],
    outputs: Option<&[(PresentationBinding, u64)]>,
) -> Result<Vec<StoredDirectory>, Error> {
    let retained = members(authority.snapshot());
    let mut paths = BTreeMap::new();
    for (binding, path) in observed {
        if &binding.worker_id != worker
            || !retained.values().any(|b| b.as_ref() == Some(binding))
            || paths.insert(binding.terminal_id.clone(), path).is_some()
        {
            return Err(Error::InvalidTerminalDirectory);
        }
    }
    Ok(retained
        .into_iter()
        .filter_map(|(id, binding)| {
            let path = binding
                .as_ref()
                .and_then(|b| paths.get(&b.terminal_id).copied())
                .or_else(|| authority.restart_directories.get(&id))?
                .clone();
            // A bound terminal names only output captured from it; an unbound one keeps the
            // output it was restored with until it starts.
            let output = outputs.and_then(|outputs| match &binding {
                Some(binding) => outputs
                    .iter()
                    .find(|(observed, _)| observed == binding)
                    .and_then(|(_, token)| (*token != 0).then_some(OutputToken(*token))),
                None => authority.restart_outputs.get(&id).copied().map(OutputToken),
            });
            Some(StoredDirectory {
                presentation_id: id,
                path,
                output,
            })
        })
        .collect())
}
impl WorkbenchAuthority {
    /// Complete checkpoint membership, without the canonical Agent registration read quota.
    pub fn terminal_restart_bindings(&self) -> Vec<PresentationBinding> {
        members(self.snapshot()).into_values().flatten().collect()
    }

    /// Every bound checkpoint member and the Presentation it shows.
    pub fn terminal_restart_presentations(
        &self,
    ) -> Vec<(PresentationBinding, TerminalPresentationId)> {
        members(self.snapshot())
            .into_iter()
            .filter_map(|(id, binding)| Some((binding?, id)))
            .collect()
    }

    /// The kept output token a restored, still unbound Presentation would replay.
    #[must_use]
    pub fn restart_output(&self, presentation_id: &TerminalPresentationId) -> Option<u64> {
        self.restart_outputs.get(presentation_id).copied()
    }
}

/// Every member Presentation and its current binding, in capture order.
pub(super) fn members(
    snapshot: &Snapshot,
) -> BTreeMap<TerminalPresentationId, Option<PresentationBinding>> {
    let mut members = BTreeMap::new();
    for context in &snapshot.terminal_contexts {
        for tab in &context.tabs {
            visit_panel_presentations(&tab.root, &mut |_, p| {
                members.insert(p.presentation_id.clone(), p.binding.clone());
            });
        }
    }
    members
}
