use super::*;
use orchbee_agent_observation_protocol::*;
use orchbee_workbench_protocol::{TerminalAgentStateSnapshot, WorkbenchRevision};

pub(crate) fn state(n: u64, agent: Option<&str>) -> TerminalAgentStateSnapshot {
    TerminalAgentStateSnapshot {
        binding: machine_agents::tests::physical(n),
        sequence: WorkbenchRevision::new(4),
        agent: agent.map(str::to_owned),
        state: PanelAgentState::Idle,
    }
}
pub(crate) fn summary(id: &str, n: u64, provider: &str) -> AgentSessionSummary {
    serde_json::from_value(json!({"executionLocation":{"executionLocationId":"local"},"lifecycle":"live",
        "observation":{"fidelity":"rich","freshness":"fresh","generation":1,"sequence":2,"source":"native"},
        "parentSessionId":null,"pendingRequestCount":0,"providerId":provider,"sessionId":id,
        "presentationAttachments":[{"generation":format!("{:032x}",1),"navigable":true,
            "presentationId":format!("{n:032x}"),"visibility":"visible"}],"status":{"kind":"working","activity":null,"phase":"working"}})).unwrap()
}
pub(crate) fn rows(
    canonical: Vec<AgentSessionSummary>,
    states: Vec<TerminalAgentStateSnapshot>,
) -> Vec<Row> {
    project(canonical, &machine_agents::tests::state_checkpoint(states)).unwrap()
}
#[test]
fn physical_only_rows_keep_full_binding_without_fabricated_sessions() {
    let rows = rows(Vec::new(), vec![state(12, Some("grok-build"))]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source(), "terminal");
    assert_eq!(rows[0].provider(), "grok");
    assert!(rows[0].canonical().is_none());
    assert_eq!(
        rows[0].target(),
        AgentNameTarget::Terminal {
            binding: machine_agents::tests::physical(12)
        }
    );
    let doc = rows[0].document(1, None);
    assert!(doc["canonical"].is_null());
    assert!(doc["cwd"].is_null());
    assert_eq!(
        doc["terminal"]["binding"]["hostEpoch"],
        format!("{:032x}", 2)
    );
}
#[test]
fn one_to_one_alias_join_prefers_fresh_canonical_and_retains_disagreeing_facet() {
    let mut c = summary("canonical", 12, "grok");
    let fresh = rows(vec![c.clone()], vec![state(12, Some("grok-build"))]);
    assert_eq!(fresh.len(), 1);
    assert_eq!(fresh[0].source(), "both");
    assert_eq!(fresh[0].status(), "working");
    assert_eq!(fresh[0].terminal().unwrap().state, PanelAgentState::Idle);
    c.observation.freshness = AgentObservationFreshness::Stale;
    assert_eq!(
        rows(vec![c.clone()], vec![state(12, Some("grok-build"))])[0].status(),
        "idle"
    );
    c.observation.freshness = AgentObservationFreshness::Fresh;
    c.status = AgentSemanticStatus::Unknown {
        activity_hint: None,
    };
    assert_eq!(
        rows(vec![c], vec![state(12, Some("grok-build"))])[0].status(),
        "idle"
    );
}
#[test]
fn ambiguous_cardinalities_preserve_every_identity() {
    let many_to_one = rows(
        vec![summary("a", 12, "codex"), summary("b", 12, "codex")],
        vec![state(12, Some("codex"))],
    );
    assert_eq!(many_to_one.len(), 3);
    assert!(many_to_one.iter().all(|r| r.source() != "both"));
    let mut c = summary("a", 12, "codex");
    c.presentation_attachments.push(
        summary("unused", 22, "codex")
            .presentation_attachments
            .remove(0),
    );
    let one_to_many = rows(
        vec![c],
        vec![state(12, Some("codex")), state(22, Some("codex"))],
    );
    assert_eq!(one_to_many.len(), 3);
    assert!(one_to_many.iter().all(|r| r.source() != "both"));
}
#[test]
fn unknown_providers_ended_and_replaced_generations_never_merge() {
    let unrecognized = rows(
        vec![summary("a", 12, "custom-agent")],
        vec![state(12, Some("custom-agent"))],
    );
    assert_eq!(unrecognized.len(), 2);
    let mut c = summary("a", 12, "codex");
    c.lifecycle = AgentLifecycle::Ended;
    let ended = rows(vec![c.clone()], vec![state(12, Some("codex"))]);
    assert_eq!(ended.len(), 2);
    assert!(
        ended
            .iter()
            .find(|r| r.canonical().is_some())
            .unwrap()
            .placement
            .is_none()
    );
    c.lifecycle = AgentLifecycle::Live;
    c.presentation_attachments[0].generation =
        AgentAttachmentGeneration::parse(&format!("{:032x}", 9)).unwrap();
    let replaced = rows(vec![c], vec![state(12, Some("codex"))]);
    assert_eq!(replaced.len(), 2);
    assert!(
        replaced
            .iter()
            .find(|r| r.canonical().is_some())
            .unwrap()
            .placement
            .is_none()
    );
}
#[test]
fn canonical_membership_uses_no_agent_incarnation_witness_and_all_current_attachments() {
    let mut c = summary("a", 12, "codex");
    c.presentation_attachments.push(
        summary("unused", 22, "codex")
            .presentation_attachments
            .remove(0),
    );
    let row = rows(vec![c], vec![state(12, Some("codex")), state(22, None)]).remove(0);
    assert_eq!(row.source(), "both");
    assert!(row.in_panel(&format!("{:032x}", 11)));
    assert!(row.in_panel(&format!("{:032x}", 21)));
    assert!(row.in_presentation(&format!("{:032x}", 22)));
    let canonical = rows(vec![summary("a", 13, "codex")], vec![state(13, None)]);
    assert_eq!(canonical[0].source(), "canonical");
    assert!(canonical[0].placement.is_some());
}
#[test]
fn placement_then_physical_then_session_order_is_deterministic() {
    let mut a = summary("z-last", 12, "codex");
    let mut b = summary("a-first", 12, "codex");
    a.presentation_attachments.clear();
    b.presentation_attachments.clear();
    let rows = rows(
        vec![a, summary("placed", 12, "claude-code"), b],
        vec![state(22, Some("grok")), state(12, Some("codex"))],
    );
    assert_eq!(
        rows[0].terminal().unwrap().binding.presentation_id.as_str(),
        format!("{:032x}", 12)
    );
    assert_eq!(
        rows[2].terminal().unwrap().binding.presentation_id.as_str(),
        format!("{:032x}", 22)
    );
    assert_eq!(rows[1].canonical().unwrap().session_id.as_str(), "placed");
    assert_eq!(rows[3].canonical().unwrap().session_id.as_str(), "a-first");
    assert_eq!(rows[4].canonical().unwrap().session_id.as_str(), "z-last");
}
#[test]
fn cursor_requires_matching_sources_filters_and_complete_order() {
    let snapshot = machine_agents::tests::fixture();
    let mut checkpoint = read::Checkpoint {
        canonical_revision: AgentSafeInteger::new(2),
        report_revision: WorkbenchRevision::new(7),
        rows: rows(
            Vec::new(),
            vec![state(12, Some("codex")), state(22, Some("grok"))],
        ),
    };
    let cursor = cursor::Cursor::next(&snapshot, &checkpoint, false, None, 1).unwrap();
    assert_eq!(cursor::Cursor::parse(cursor.as_str()).unwrap(), cursor);
    assert_eq!(cursor.offset(&snapshot, &checkpoint, false, None), Ok(1));
    assert!(cursor.offset(&snapshot, &checkpoint, true, None).is_err());
    checkpoint.rows.swap(0, 1);
    assert!(cursor.offset(&snapshot, &checkpoint, false, None).is_err());
    checkpoint.rows.swap(0, 1);
    checkpoint.report_revision = WorkbenchRevision::new(8);
    assert!(cursor.offset(&snapshot, &checkpoint, false, None).is_err());
    assert!(cursor::Cursor::parse("u1.e30").is_err());
    assert!(cursor::Cursor::parse(&format!("u1.{}", "A".repeat(1024))).is_err());
}
