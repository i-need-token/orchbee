use super::*;

struct Trace {
    events: Vec<String>,
    fail: Option<usize>,
    kind: FailureKind,
}
impl Trace {
    fn event(&mut self, event: String) -> Result<(), Failure> {
        self.events.push(event);
        if self.fail == Some(self.events.len() - 1) {
            Err(Failure::new(self.kind))
        } else {
            Ok(())
        }
    }
}
impl Port for Trace {
    fn create(&mut self) -> Result<(), Failure> {
        self.event("create".into())
    }
    fn split(&mut self, index: usize) -> Result<(), Failure> {
        self.event(format!("split{index}"))
    }
    fn settle(&mut self, index: usize) -> Result<(), Failure> {
        self.event(format!("started{index}"))
    }
    fn label(&mut self, index: usize) -> Result<(), Failure> {
        self.event(format!("label{index}"))
    }
    fn ratio(&mut self, index: usize) -> Result<(), Failure> {
        self.event(format!("ratio{index}"))
    }
    fn ready(&mut self) -> Result<(), Failure> {
        self.event("ready".into())
    }
    fn focus(&mut self) -> Result<(), Failure> {
        self.event("focus".into())
    }
    fn replace(&mut self) -> Result<(), Failure> {
        self.event("closed".into())
    }
    fn observe(&mut self) -> Result<(), Failure> {
        self.event("observe".into())
    }
    fn rollback(&mut self) {
        self.events.push("rollback".into());
    }
}
fn plan() -> Plan {
    super::super::template::parse(br#"{"type":"split","direction":"right","ratio":0.3,"first":{"type":"panel"},"second":{"type":"panel"}}"#).expect("fixture").plan()
}

#[test]
fn replacement_requires_all_started_leaves_labels_ratios_and_a_ready_tree() {
    let mut trace = Trace {
        events: vec![],
        fail: None,
        kind: FailureKind::Rejected,
    };
    execute(&plan(), &mut trace).expect("complete");
    assert_eq!(
        trace.events,
        [
            "create", "split0", "started0", "started1", "label0", "label1", "ratio0", "ready",
            "focus", "closed", "observe"
        ]
    );
}

#[test]
fn every_phase_failure_stops_later_effects_and_only_definite_pre_ready_failure_compensates() {
    let expected = [
        "create", "split0", "started0", "started1", "label0", "label1", "ratio0", "ready", "focus",
        "closed", "observe",
    ];
    for index in 0..expected.len() {
        for kind in [
            FailureKind::Rejected,
            FailureKind::Cancelled,
            FailureKind::Timeout,
            FailureKind::OutcomeUnknown,
        ] {
            let mut trace = Trace {
                events: vec![],
                fail: Some(index),
                kind,
            };
            assert_eq!(execute(&plan(), &mut trace).expect_err("fault").kind, kind);
            let mut prefix = expected[..=index]
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>();
            if index <= 7 && kind != FailureKind::OutcomeUnknown {
                prefix.push("rollback".into());
            }
            assert_eq!(trace.events, prefix, "{kind:?} at {index}");
        }
    }
}
