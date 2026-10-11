use super::*;
use crate::{AgentViewSpec, parse_agent_view_spec};
use serde_json::json;
struct Agent {
    scope: &'static str,
    workspace: Option<&'static str>,
    tab: Option<&'static str>,
    token: Option<&'static str>,
    seen: bool,
    seq: u64,
}
impl Entry for Agent {
    fn scope(&self) -> &str {
        self.scope
    }
    fn status(&self) -> &str {
        if self.seen { "idle" } else { "done" }
    }
    fn workspace_id(&self) -> Option<&str> {
        self.workspace
    }
    fn tab_id(&self) -> Option<&str> {
        self.tab
    }
    fn pane_id(&self) -> Option<&str> {
        Some("p")
    }
    fn agent(&self) -> Option<&str> {
        Some("claude")
    }
    fn seen(&self) -> bool {
        self.seen
    }
    fn state_change_seq(&self) -> Option<u64> {
        Some(self.seq)
    }
    fn token(&self, name: &str) -> Option<&str> {
        if name == "phase" { self.token } else { None }
    }
    fn workspace_order(&self) -> Option<u64> {
        self.workspace.map(|_| 0)
    }
    fn tab_order(&self) -> Option<u64> {
        Some(1)
    }
    fn pane_order(&self) -> Option<u64> {
        Some(2)
    }
    fn attention(&self) -> u64 {
        if self.seen { 1 } else { 3 }
    }
}
fn agent(scope: &'static str) -> Agent {
    Agent {
        scope,
        workspace: Some("w"),
        tab: Some("t"),
        token: None,
        seen: false,
        seq: 1,
    }
}
fn make_spec(filter: serde_json::Value, sort: serde_json::Value) -> AgentViewSpec {
    parse_agent_view_spec(
        &serde_json::to_vec(&json!({"source":"x","filter":filter,"sort":sort})).unwrap(),
    )
    .unwrap()
}
#[test]
fn ids_and_current_context_are_endpoint_qualified() {
    let context = Context {
        scope: "local",
        workspace_id: Some("w"),
        tab_id: Some("t"),
    };
    for value in [json!("w"), json!({"context":"current_workspace_id"})] {
        let spec = make_spec(
            json!({"op":"eq","field":"workspace_id","value":value}),
            json!([]),
        );
        assert!(matches_filter(
            &context,
            &agent("local"),
            spec.filter.as_ref().unwrap()
        ));
        assert!(!matches_filter(
            &context,
            &agent("remote"),
            spec.filter.as_ref().unwrap()
        ));
    }
}
#[test]
fn boolean_logic_in_exists_and_reference_missing_equality() {
    let context = Context {
        scope: "local",
        workspace_id: None,
        tab_id: None,
    };
    let spec = make_spec(
        json!({"op":"all","filters":[{"op":"in","field":"status","values":["working","done"]},{"op":"not","filter":{"op":"exists","field":{"token":"phase"}}},{"op":"any","filters":[{"op":"eq","field":"seen","value":false},{"op":"eq","field":"agent","value":"other"}]}]}),
        json!([]),
    );
    assert!(matches_filter(
        &context,
        &agent("local"),
        spec.filter.as_ref().unwrap()
    ));
    let mut entry = agent("local");
    entry.seen = true;
    assert!(!matches_filter(
        &context,
        &entry,
        spec.filter.as_ref().unwrap()
    ));
    let missing = make_spec(
        json!({"op":"eq","field":"workspace_id","value":{"context":"current_workspace_id"}}),
        json!([]),
    );
    entry.workspace = None;
    assert!(matches_filter(
        &context,
        &entry,
        missing.filter.as_ref().unwrap()
    ));
    entry.seq = u64::MAX;
    let maximum = make_spec(
        json!({"op":"eq","field":"state_change_seq","value":u64::MAX}),
        json!([]),
    );
    assert!(matches_filter(
        &context,
        &entry,
        maximum.filter.as_ref().unwrap()
    ));
}
#[test]
fn token_multikey_missing_last_both_directions_and_stable_ties() {
    let mut a = agent("local");
    a.token = Some("10");
    let mut b = agent("remote");
    b.token = Some("2");
    let c = agent("remote");
    for (direction, expected) in [("asc", Ordering::Less), ("desc", Ordering::Greater)] {
        let s = make_spec(
            json!({"op":"exists","field":"agent"}),
            json!([{"field":{"token":"phase"},"order":direction},{"field":"seen"},{"field":"state_change_seq","order":"desc"}]),
        );
        assert_eq!(compare_entries(&a, &b, &s.sort), expected);
        assert_eq!(compare_entries(&a, &c, &s.sort), Ordering::Less);
        assert_eq!(compare_entries(&c, &a, &s.sort), Ordering::Greater);
        assert_eq!(compare_entries(&a, &a, &s.sort), Ordering::Equal);
        b.token = a.token;
        b.seen = true;
        assert_eq!(compare_entries(&a, &b, &s.sort), Ordering::Less);
        b.seen = false;
        b.seq = 2;
        assert_eq!(compare_entries(&a, &b, &s.sort), Ordering::Greater);
        b.seq = 1;
        assert_eq!(compare_entries(&a, &b, &s.sort), Ordering::Equal);
        b.token = Some("2");
        b.seen = false;
    }
}
