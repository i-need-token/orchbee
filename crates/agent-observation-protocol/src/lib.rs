//! Independent Rust binding for the version-1 Agent observation read contract:
//! bounded typed wire DTOs, strict semantic validation, and the OBAP frame
//! codec governed by `schemas/agent-observation/v1`. This crate owns no
//! canonical Agent state, I/O or socket policy, permissions, cursor secrets,
//! deadlines, or rendering — those stay with the Server and the CLI.

mod changes;
mod contract;
mod cursor;
mod frame;
mod identity;
mod launch;
mod model;
mod names;
mod read;
mod report_values;

pub use changes::{
    AgentChange, AgentChangeSession, AgentChangeTransition, AgentChangesCursor,
    AgentChangesOutcome, AgentChangesOutcomeEnvelope, AgentChangesRejectionReason,
    AgentChangesRequest, AgentChangesResult, MAXIMUM_CHANGE_ITEMS,
};
pub use contract::{
    AgentContractError, parse_agent_request, parse_agent_result, serialize_agent_request,
    serialize_agent_result,
};
pub use cursor::{
    AGENT_CURSOR_PAYLOAD_BYTES, AGENT_CURSOR_TAG_BYTES, AGENT_LIST_CURSOR_CHARACTERS,
    AGENT_LIST_CURSOR_MAXIMUM_CHARACTERS, AgentCursorError, AgentCursorPayload, AgentListCursor,
};
pub use frame::{
    AGENT_MAGIC, AgentWireError, MAXIMUM_AGENT_CHANGES_RESULT_BYTES,
    MAXIMUM_AGENT_LIST_RESULT_BYTES, MAXIMUM_AGENT_NAMES_RESULT_BYTES, MAXIMUM_AGENT_REQUEST_BYTES,
    MAXIMUM_AGENT_SESSION_RESULT_BYTES, MAXIMUM_AGENT_SMALL_OUTCOME_BYTES, read_agent_request,
    read_agent_result, write_agent_request, write_agent_result,
};
pub use identity::{
    AgentAttachmentGeneration, AgentAttentionRequestId, AgentEventId, AgentExecutionLocationId,
    AgentHostEpoch, AgentHostTerminalId, AgentIdentityError, AgentName, AgentPresentationId,
    AgentProviderId, AgentSafeInteger, AgentSessionId, AgentTurnId, MAXIMUM_SAFE_INTEGER,
    ObapRequestId,
};
pub use launch::{
    AgentLaunchBinding, AgentLaunchOutcome, AgentLaunchOutcomeEnvelope, AgentLaunchRejectionReason,
    AgentLaunchRequest, AgentLaunchReservation, AgentLaunchResult, AgentManagement,
    LAUNCH_SETTLE_MS, MAXIMUM_LAUNCH_TTL_MS,
};
pub use model::{
    AgentActivity, AgentAttentionKind, AgentAttentionRequest, AgentCapability,
    AgentExecutionLocation, AgentFreshnessMeaning, AgentLifecycle, AgentObservation,
    AgentObservationFidelity, AgentObservationFreshness, AgentObservationSource,
    AgentPositiveInteger, AgentPresentationAttachment, AgentPresentationVisibility,
    AgentSemanticStatus, AgentSessionDetail, AgentSessionExplanation, AgentSessionSnapshot,
    AgentSessionSummary, AgentStatusBasis, AgentTurnOutcome, AgentTurnOutcomeKind, AgentTurnPhase,
    AgentTurnState, ContentUnavailable, NavigateOnly,
};
pub use names::{
    AgentNameAssignOutcome, AgentNameAssignOutcomeEnvelope, AgentNameAssignRejectionReason,
    AgentNameBinding, AgentNameReleaseOutcome, AgentNameReleaseOutcomeEnvelope,
    AgentNameReleaseRejectionReason, AgentNameTarget, AgentNamedTarget, AgentNamesListOutcome,
    AgentNamesListOutcomeEnvelope, AgentNamesListRejectionReason, AgentNamesRequest,
    AgentNamesResult, AgentResolveOutcome, AgentResolveOutcomeEnvelope,
    AgentResolveRejectionReason, AgentResolvedBy, AgentSessionSelector, MAXIMUM_NAME_BINDINGS,
};
pub use read::{
    AgentExplainOutcome, AgentExplainOutcomeEnvelope, AgentGetOutcome, AgentGetOutcomeEnvelope,
    AgentListOutcome, AgentListOutcomeEnvelope, AgentListRejectionReason, AgentReadRequest,
    AgentReadResult, AgentRequest, AgentResult, AgentSessionRejectionReason,
    AgentUnavailableReason, DEFAULT_LIST_LIMIT, MAXIMUM_CAPABILITIES, MAXIMUM_LIST_ITEMS,
    MAXIMUM_LIST_LIMIT, MAXIMUM_PENDING_REQUESTS, MAXIMUM_PRESENTATION_ATTACHMENTS,
};

#[cfg(test)]
mod tests;

pub use report_values::{
    AgentProviderSessionReference, AgentProviderSessionReferenceKind, AgentReportProvider,
    AgentReportSequence, AgentReportSourcePolicy, AgentSessionStartSource,
    MAXIMUM_PROVIDER_SESSION_ID_BYTES, MAXIMUM_PROVIDER_SESSION_PATH_BYTES,
    agent_report_allows_session_replacement, agent_report_reserves_native_state,
    agent_report_source_policy, normalize_agent_report_label, windows_agent_resume_argv,
};

mod process_identity;
pub use process_identity::{
    AgentProcessCandidate, AgentProcessSelection, identify_agent_process, native_agent_basename,
    native_agent_provider, select_agent_process,
};
