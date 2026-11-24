//! Which Agent state changes give the person something new to look at and so mark the Agent
//! unread. The local physical acknowledgement owner and the machine Agent rows share this rule.

// module-layer: values

use orchbee_workbench_protocol::PanelAgentState;

/// Whether a terminal's new Agent version starts seen. A finished turn, a request or a failure
/// hands the person something new, so it starts unseen. Entering work, starting, or a screen no
/// rule recognises shows nothing new, so the version keeps `previous`'s acknowledgement: a finished
/// turn nobody has read stays unread while the Agent works again. `previous` is the state and
/// acknowledgement of the same Agent's older version in that terminal, absent when this Client has
/// none. An idle version after an idle one means versions went unread in between, which may hold a
/// whole turn, so it counts as finished.
pub(crate) fn version_starts_seen(
    previous: Option<(PanelAgentState, bool)>,
    current: PanelAgentState,
) -> bool {
    use PanelAgentState as State;
    let handed_over = match current {
        State::Done | State::Error | State::Blocked => true,
        State::Idle => matches!(previous, Some((State::Working | State::Idle, _))),
        State::Working | State::Opening | State::Unknown => false,
    };
    !handed_over && previous.is_none_or(|(_, seen)| seen)
}
