//! Compact title reads join only the already-observed exact physical Agent bindings.
use super::{
    AtomicU8, Instant, LocalClient, MAXIMUM_BYTES, MAXIMUM_TERMINAL_TITLE_ROWS, PanelAgentAction,
    PanelAgentOutcome, PanelAgentRequest, ProtocolVersion, Snapshot, TerminalAgentStateSnapshot,
    TerminalPresentationId, TerminalTitleObservation, check, request_id,
};
use std::collections::BTreeMap;
pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    states: &BTreeMap<TerminalPresentationId, Option<TerminalAgentStateSnapshot>>,
    deadline: Instant,
    cancel: &AtomicU8,
    bytes: &mut usize,
) -> Result<BTreeMap<TerminalPresentationId, TerminalTitleObservation>, ()> {
    let bindings: Vec<_> = states
        .values()
        .flatten()
        .filter(|s| s.agent.is_some())
        .map(|s| s.binding.clone())
        .collect();
    let mut titles = BTreeMap::new();
    for chunk in bindings.chunks(MAXIMUM_TERMINAL_TITLE_ROWS) {
        check(deadline, cancel)?;
        let result = client
            .panel_agent(
                PanelAgentRequest {
                    protocol_version: ProtocolVersion,
                    request_id: request_id()?,
                    site_id: snapshot.site_id.clone(),
                    action: PanelAgentAction::Titles {
                        bindings: chunk.to_vec(),
                    },
                },
                deadline,
                Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
            )
            .map_err(|_| ())?;
        let PanelAgentOutcome::Titles {
            workbench_revision,
            presentations,
        } = result.outcome
        else {
            return Err(());
        };
        if workbench_revision != snapshot.revision || presentations.len() != chunk.len() {
            return Err(());
        }
        for (expected, row) in chunk.iter().zip(presentations) {
            *bytes = bytes
                .checked_add(serde_json::to_vec(&row).map_err(|_| ())?.len())
                .ok_or(())?;
            if &row.binding != expected || *bytes > MAXIMUM_BYTES {
                return Err(());
            }
            let id = TerminalPresentationId::parse(row.binding.presentation_id.as_str())
                .map_err(|_| ())?;
            if titles.insert(id, row.observation).is_some() {
                return Err(());
            }
        }
    }
    check(deadline, cancel)?;
    Ok(titles)
}
