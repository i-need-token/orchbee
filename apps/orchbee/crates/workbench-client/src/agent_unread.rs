//! Which Agent state changes give the person something new to look at and so mark the Agent
//! unread. The local physical acknowledgement owner and the machine Agent rows share this rule.

// module-layer: values

use orchbee_workbench_protocol::{AgentCompletion, PanelAgentState};

/// The facts of one Agent version that decide whether it hands the person something new.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UnreadVersion {
    pub(crate) state: PanelAgentState,
    pub(crate) completion: AgentCompletion,
}

/// Whether a terminal's new Agent version starts seen. A finished turn, a request or a failure
/// hands the person something new, so it starts unseen. Entering work, starting, or a screen no
/// rule recognises shows nothing new, so the version keeps `previous`'s acknowledgement: a finished
/// turn nobody has read stays unread while the Agent works again. `previous` is the facts and
/// acknowledgement of the same Agent's older version in that terminal, absent when this Client has
/// none. An idle version finishes a turn when it newly awaits input, even after a screen no rule
/// recognised, or when it follows a working or an idle one: an idle version after an idle one means
/// versions went unread in between, which may hold a whole turn.
pub(crate) fn version_starts_seen(
    previous: Option<(UnreadVersion, bool)>,
    current: UnreadVersion,
) -> bool {
    use PanelAgentState as State;
    let awaits = |version: UnreadVersion| version.completion == AgentCompletion::AwaitingInput;
    let handed_over = match current.state {
        State::Done | State::Error | State::Blocked => true,
        State::Idle => {
            (awaits(current) && previous.is_none_or(|(before, _)| !awaits(before)))
                || previous
                    .is_some_and(|(before, _)| matches!(before.state, State::Working | State::Idle))
        }
        State::Working | State::Opening | State::Unknown => false,
    };
    !handed_over && previous.is_none_or(|(_, seen)| seen)
}
