//! Agent session resume candidates: the canonical side of the capture rule. One state access joins
//! root hook sessions to their canonical lifecycle and to a navigable attachment of an exact
//! Terminal generation. Command and process evidence, and the final choice, belong to the Session.

use orchbee_agent_observation_protocol::AgentRunToken;
use orchbee_server::AgentResumeProvider;
use orchbee_workbench_protocol::TerminalPresentationId;

/// One root hook session of a resumable provider, live and attached to `presentation`'s current
/// Terminal generation. The native key is unvalidated here; a reference admits it only after the
/// Session's selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentResumeCandidate {
    pub(crate) presentation: TerminalPresentationId,
    pub(crate) provider: AgentResumeProvider,
    pub(crate) native_key: String,
    pub(crate) run_token: Option<AgentRunToken>,
}

#[cfg(windows)]
impl super::AgentService {
    /// Candidates for each supplied Presentation and its current Host Terminal identity: root
    /// sessions whose provider has a resume row, whose canonical session is live with no parent,
    /// and which hold a navigable attachment of that Presentation at that exact creation
    /// generation. A disposed runtime has none.
    pub(crate) fn resume_candidates(
        &self,
        physical: &[(
            TerminalPresentationId,
            orchbee_capability_node_protocol::HostTerminalIdentity,
        )],
    ) -> Result<Vec<AgentResumeCandidate>, super::AgentFoldUnavailable> {
        let generations = physical
            .iter()
            .map(|(presentation, identity)| {
                (
                    presentation,
                    super::canonical_hex32(identity.creation_id().as_bytes()),
                )
            })
            .collect::<Vec<_>>();
        self.with_state(|state| {
            if state.runtime.is_disposed() {
                return Vec::new();
            }
            let sessions = &state.runtime.snapshot().sessions;
            let mut candidates = Vec::new();
            for native in state.ingress.native_session_refs() {
                let Some(provider) = AgentResumeProvider::from_provider_id(native.provider_id)
                else {
                    continue;
                };
                let Some(session) = sessions.iter().find(|session| {
                    session.session_id.as_str() == native.session_id
                        && session.provider_id.as_str() == native.provider_id
                        && session.lifecycle == crate::agent::AgentLifecycle::Live
                        && session.parent_session_id.is_none()
                }) else {
                    continue;
                };
                for (presentation, generation) in &generations {
                    if session.presentation_attachments.iter().any(|attachment| {
                        attachment.navigable
                            && attachment.presentation_id.as_str() == presentation.as_str()
                            && attachment.generation.as_str() == generation
                    }) {
                        candidates.push(AgentResumeCandidate {
                            presentation: (*presentation).clone(),
                            provider,
                            native_key: native.native_key.to_owned(),
                            run_token: native.run_token.cloned(),
                        });
                    }
                }
            }
            candidates
        })
    }
}
