use super::*;

#[derive(Clone)]
struct Viewer {
    owner: usize,
    target: usize,
    id: ViewerId,
    live: bool,
    binding_attach: bool,
    grant: Option<usize>,
    sequence: u64,
    next_frame: u64,
    flight: Option<u64>,
}
#[derive(Default)]
struct Oracle {
    viewers: BTreeMap<usize, Viewer>,
    grants: BTreeMap<usize, ControllerId>,
    disposed: BTreeSet<usize>,
    coverage: Coverage,
    worker_lost: bool,
    last_interleaved_viewer: [Option<usize>; 2],
}
impl Oracle {
    fn viewer_id(&self, slot: usize) -> ViewerId {
        self.viewers
            .get(&slot)
            .map(|v| v.id.clone())
            .unwrap_or_else(|| ViewerId::parse(&format!("{:032x}", 900_000 + slot)).unwrap())
    }
    fn grant_id(&self, slot: usize) -> ControllerId {
        self.grants
            .get(&slot)
            .cloned()
            .unwrap_or_else(|| ControllerId::parse(&format!("{:032x}", 800_000 + slot)).unwrap())
    }
    fn selected(&self, s: &Step) -> Option<Viewer> {
        self.viewers
            .get(&s.viewer)
            .filter(|v| v.live && v.owner == s.session)
            .cloned()
    }
    fn bytes(id: &ViewerId) -> [u8; 16] {
        u128::from_str_radix(id.as_str(), 16).unwrap().to_be_bytes()
    }
    fn controller_bytes(id: &ControllerId) -> [u8; 16] {
        u128::from_str_radix(id.as_str(), 16).unwrap().to_be_bytes()
    }
    fn cleanup(&self, v: &Viewer) -> Vec<Effect> {
        if self.worker_lost || self.disposed.contains(&v.target) {
            return Vec::new();
        }
        let mut effects = Vec::new();
        let id = Self::bytes(&v.id);
        if let Some(grant) = v.grant {
            effects.push(Effect::Release(
                id,
                Self::controller_bytes(&self.grants[&grant]),
            ));
        }
        effects.push(Effect::Detach(id));
        effects
    }
}
fn tag(message: &ServerMessage) -> String {
    match message {
        ServerMessage::TerminalViewerResult(result) => match result.outcome() {
            Outcome::Attached { .. } => "attached".into(),
            Outcome::Empty {} => "empty".into(),
            Outcome::Record(_) => "record".into(),
            Outcome::FeedbackAccepted {} => "feedback".into(),
            Outcome::Detached {} => "detached".into(),
            Outcome::Rejected { reason } => format!("viewer:{reason:?}"),
            Outcome::RowsRejected { reason } => format!("rows:{reason:?}"),
            Outcome::Unavailable { reason } => format!("unavailable:{reason:?}"),
        },
        ServerMessage::TerminalControlResult(result) => match result.outcome() {
            ControlOutcome::Acquired { .. } => "acquired".into(),
            ControlOutcome::InputAccepted {} => "input".into(),
            ControlOutcome::Resized { .. } => "resize".into(),
            ControlOutcome::Pasted { .. } => "pasted".into(),
            ControlOutcome::Rejected { reason } => format!("control:{reason:?}"),
            ControlOutcome::Unavailable { reason } => format!("unavailable:{reason:?}"),
            ControlOutcome::OutcomeUnknown {} => "unknown".into(),
            ControlOutcome::Cleared { .. } => "clear".into(),
        },
        _ => "wrong-message".into(),
    }
}
fn failure(kind: Kind, category: &str, detail: String) -> Failure {
    Failure {
        fingerprint: (
            match kind {
                Kind::Attach => "attach",
                Kind::Acquire => "acquire",
                Kind::Control => "control",
                Kind::Poll => "poll",
                Kind::Feedback => "feedback",
                Kind::Detach => "detach",
                Kind::Drop => "drop",
                Kind::Dispose => "dispose",
                Kind::ReplaceWorker => "worker-loss",
            },
            category.to_owned(),
        ),
        detail,
    }
}

pub(super) fn run(steps: &[Step]) -> Result<Coverage, Failure> {
    // Preserve seed/replay diagnostics even when fixture setup, peer lifecycle or a
    // runtime assertion panics. Infrastructure failures cannot replace a semantic
    // counterexample during shrinking, and are reported with the original trace.
    std::panic::catch_unwind(|| run_inner(steps)).unwrap_or_else(|payload| {
        let detail = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_owned())
            })
            .unwrap_or_else(|| "non-text panic payload".to_owned());
        Err(Failure {
            fingerprint: ("fixture", "panic".to_owned()),
            detail,
        })
    })
}
fn run_inner(steps: &[Step]) -> Result<Coverage, Failure> {
    // Allocation symbols are stable replay identities, never mutable current-viewer aliases.
    // A shrink that aliases two declarations is invalid independently of runtime outcomes.
    let mut viewer_symbols = BTreeSet::new();
    let mut grant_symbols = BTreeSet::new();
    for step in steps {
        let invalid = match step.kind {
            Kind::Attach if !viewer_symbols.insert(step.viewer) => Some("duplicate-viewer-symbol"),
            Kind::Acquire if !grant_symbols.insert(step.grant) => Some("duplicate-grant-symbol"),
            _ => None,
        };
        if let Some(reason) = invalid {
            return Err(Failure {
                fingerprint: ("replay", reason.to_owned()),
                detail: "allocation symbols must be single-assignment".to_owned(),
            });
        }
    }
    let fixture = Fixture::new();
    let mut sessions = [
        Some(fixture.service.session()),
        Some(fixture.service.session()),
    ];
    let mut oracle = Oracle::default();
    let mut consumed = 0;
    for (index, s) in steps.iter().enumerate() {
        let selected = oracle.selected(s);
        let viewer_id = oracle.viewer_id(s.viewer);
        let grant_id = oracle.grant_id(s.grant);
        let mut effects = Vec::new();
        let mut action = None;
        let mut expected = String::new();
        let two = !oracle.worker_lost
            && oracle
                .viewers
                .values()
                .filter(|v| v.live && v.owner == s.session && !oracle.disposed.contains(&v.target))
                .count()
                >= 2;
        if two {
            oracle.coverage.insert("two-live");
        }
        if selected.is_none()
            && !matches!(
                s.kind,
                Kind::Attach | Kind::Drop | Kind::Dispose | Kind::ReplaceWorker
            )
        {
            if oracle
                .viewers
                .get(&s.viewer)
                .is_some_and(|v| v.live && v.owner != s.session)
            {
                oracle.coverage.insert("wrong-scope");
            } else if oracle
                .viewers
                .get(&s.viewer)
                .is_some_and(|viewer| !viewer.live && viewer.owner == s.session)
            {
                oracle.coverage.insert("stale-viewer");
            }
        }
        match s.kind {
            Kind::Attach => {
                for viewer in oracle.viewers.values_mut() {
                    if viewer.owner == s.session && oracle.disposed.contains(&viewer.target) {
                        viewer.live = false;
                    }
                }
                let count = oracle
                    .viewers
                    .values()
                    .filter(|v| v.live && v.owner == s.session)
                    .count();
                let duplicate = oracle
                    .viewers
                    .values()
                    .any(|v| v.live && v.owner == s.session && v.target == s.target);
                expected = if count == 8 {
                    oracle.coverage.insert("local-capacity");
                    if duplicate {
                        oracle.coverage.insert("full-duplicate");
                    }
                    "unavailable:ServerBusy"
                } else if oracle.worker_lost {
                    "unavailable:TerminalRuntimeUnavailable"
                } else if oracle.disposed.contains(&s.target) {
                    "viewer:TerminalUnavailable"
                } else if duplicate {
                    if oracle.viewers.values().any(|viewer| {
                        viewer.live
                            && viewer.owner == s.session
                            && viewer.target == s.target
                            && viewer.binding_attach != s.alternate
                    }) {
                        oracle.coverage.insert("duplicate");
                    }
                    "viewer:AlreadyAttached"
                } else if oracle
                    .viewers
                    .values()
                    .filter(|v| v.live && !oracle.disposed.contains(&v.target))
                    .count()
                    == 8
                {
                    effects.push(Effect::Capacity);
                    oracle.coverage.insert("worker-capacity");
                    oracle.coverage.insert("rejected-candidate");
                    "unavailable:ServerBusy"
                } else {
                    "attached"
                }
                .into();
                action = Some(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
                    client_id(100_000 + index as u128),
                    fixture.attach(s.target, s.alternate),
                )));
            }
            Kind::Acquire => {
                expected = if let Some(v) = &selected {
                    if oracle.worker_lost {
                        "unavailable:TerminalRuntimeUnavailable"
                    } else if v.grant.is_some() {
                        "control:ControllerBusy"
                    } else if oracle.disposed.contains(&v.target) {
                        "control:ViewerUnavailable"
                    } else if oracle.viewers.values().any(|other| {
                        other.live && other.target == v.target && other.grant.is_some()
                    }) {
                        "control:ControllerBusy"
                    } else {
                        "acquired"
                    }
                } else {
                    "control:ViewerUnavailable"
                }
                .into();
                action = Some(ClientMessage::TerminalControl(TerminalControlRequest::new(
                    client_id(100_000 + index as u128),
                    Control::Acquire {
                        attachment_id: viewer_id.clone(),
                    },
                )));
            }
            Kind::Control => {
                expected = if let Some(v) = &selected {
                    if oracle.worker_lost {
                        "unavailable:TerminalRuntimeUnavailable"
                    } else if v.grant != Some(s.grant) {
                        if v.grant.is_some() {
                            oracle.coverage.insert("wrong-grant");
                        }
                        if v.grant.is_some()
                            && oracle.viewers.values().any(|old| {
                                !old.live
                                    && old.owner == v.owner
                                    && old.target == v.target
                                    && old.grant == Some(s.grant)
                            })
                        {
                            oracle.coverage.insert("stale-grant");
                        }
                        "control:NotController"
                    } else if s.value <= v.sequence {
                        "control:SequenceMismatch"
                    } else if oracle.disposed.contains(&v.target) {
                        "control:ViewerUnavailable"
                    } else {
                        if s.value > v.sequence + 1 {
                            oracle.coverage.insert("gap");
                        }
                        if oracle.viewers.values().any(|other| {
                            other.live
                                && other.owner == v.owner
                                && !oracle.disposed.contains(&other.target)
                                && other.grant.is_some()
                                && other.id != v.id
                                && other.sequence == s.value
                        }) {
                            oracle.coverage.insert("equal-sequence");
                        }
                        let id = Oracle::bytes(&v.id);
                        let grant = Oracle::controller_bytes(&grant_id);
                        effects.push(if s.alternate {
                            Effect::Resize(id, grant, s.value)
                        } else {
                            Effect::Input(id, grant, s.value)
                        });
                        oracle.viewers.get_mut(&s.viewer).unwrap().sequence = s.value;
                        if s.alternate { "resize" } else { "input" }
                    }
                } else {
                    "control:ViewerUnavailable"
                }
                .into();
                let sequence = NonZeroU64::new(s.value).unwrap();
                let control = if s.alternate {
                    Control::Resize {
                        attachment_id: viewer_id.clone(),
                        controller_id: grant_id,
                        sequence,
                        geometry: Geometry::new(70 + s.target as u16, 20, 8, 16).unwrap(),
                    }
                } else {
                    Control::Input {
                        attachment_id: viewer_id.clone(),
                        controller_id: grant_id,
                        sequence,
                        bytes: ControlInput::new(b"routing".to_vec()).unwrap(),
                    }
                };
                action = Some(ClientMessage::TerminalControl(TerminalControlRequest::new(
                    client_id(100_000 + index as u128),
                    control,
                )));
            }
            Kind::Poll | Kind::Feedback | Kind::Detach => {
                expected = if let Some(v) = &selected {
                    if oracle.disposed.contains(&v.target) {
                        oracle.viewers.get_mut(&s.viewer).unwrap().live = false;
                        if s.kind == Kind::Detach {
                            "detached"
                        } else {
                            "unavailable:TerminalRuntimeUnavailable"
                        }
                    } else if oracle.worker_lost {
                        oracle.viewers.get_mut(&s.viewer).unwrap().live = false;
                        "unavailable:TerminalRuntimeUnavailable"
                    } else if s.kind == Kind::Detach {
                        effects.extend(oracle.cleanup(v));
                        oracle.viewers.get_mut(&s.viewer).unwrap().live = false;
                        if two {
                            oracle.coverage.insert("detach-sibling");
                        }
                        "detached"
                    } else if s.kind == Kind::Poll {
                        // Poll reaches the Worker even while a frame flight is
                        // pending; the Worker then decides empty vs. record.
                        effects.push(Effect::Poll(Oracle::bytes(&v.id)));
                        if v.flight.is_some() {
                            "empty"
                        } else {
                            let current = oracle.viewers.get_mut(&s.viewer).unwrap();
                            current.next_frame += 1;
                            current.flight = Some(current.next_frame);
                            "record"
                        }
                    } else if v.flight == Some(s.value) {
                        if oracle.viewers.values().any(|other| {
                            other.live
                                && other.owner == v.owner
                                && !oracle.disposed.contains(&other.target)
                                && other.id != v.id
                                && other.flight == v.flight
                        }) {
                            oracle.coverage.insert("equal-frame");
                        }
                        effects.push(Effect::Feedback(Oracle::bytes(&v.id)));
                        oracle.viewers.get_mut(&s.viewer).unwrap().flight = None;
                        "feedback"
                    } else {
                        if v.flight.is_some() {
                            oracle.coverage.insert("bad-feedback");
                        }
                        "viewer:FeedbackMismatch"
                    }
                } else if s.kind == Kind::Detach {
                    oracle.coverage.insert("absent-detach");
                    "detached"
                } else {
                    "viewer:TerminalUnavailable"
                }
                .into();
                let action_value = match s.kind {
                    Kind::Poll => Action::Poll {
                        attachment_id: viewer_id,
                    },
                    Kind::Detach => Action::Detach {
                        attachment_id: viewer_id,
                    },
                    _ => {
                        if s.alternate {
                            Action::NeedSnapshot {
                                attachment_id: viewer_id,
                                sequence: NonZeroU64::new(s.value).unwrap(),
                                reason:
                                    orchbee_client_server_protocol::ViewerSnapshotReason::StoreReset,
                            }
                        } else {
                            Action::FrameApplied {
                                attachment_id: viewer_id,
                                sequence: NonZeroU64::new(s.value).unwrap(),
                                generation: NonZeroU64::MIN,
                                revision: s.value,
                            }
                        }
                    }
                };
                action = Some(ClientMessage::TerminalViewer(TerminalViewerRequest::new(
                    client_id(100_000 + index as u128),
                    action_value,
                )));
            }
            Kind::Drop => {
                let owned: Vec<_> = oracle
                    .viewers
                    .values()
                    .filter(|v| v.live && v.owner == s.session)
                    .cloned()
                    .collect();
                if !oracle.worker_lost
                    && owned
                        .iter()
                        .filter(|viewer| !oracle.disposed.contains(&viewer.target))
                        .count()
                        >= 2
                {
                    oracle.coverage.insert("drop-multiple");
                }
                for viewer in &owned {
                    effects.extend(oracle.cleanup(viewer));
                }
                for viewer in oracle.viewers.values_mut() {
                    if viewer.owner == s.session {
                        viewer.live = false;
                    }
                }
                drop(sessions[s.session].take());
                sessions[s.session] = Some(fixture.service.session());
            }
            Kind::Dispose => {
                if oracle.worker_lost || oracle.disposed.contains(&s.target) {
                    continue;
                }
                if oracle.viewers.values().any(|v| {
                    v.live
                        && v.target == s.target
                        && oracle.viewers.values().any(|other| {
                            other.live
                                && other.owner == v.owner
                                && other.target != s.target
                                && !oracle.disposed.contains(&other.target)
                        })
                }) {
                    oracle.coverage.insert("disposed-sibling");
                }
                fixture.dispose(s.target);
                oracle.disposed.insert(s.target);
                effects.push(Effect::Dispose(s.target));
            }
            Kind::ReplaceWorker => {
                fixture.replace_worker();
                oracle.worker_lost = true;
            }
        }
        if let Some(action) = action {
            let reply = sessions[s.session].as_mut().unwrap().execute(action);
            let actual = tag(&reply);
            if actual != expected {
                return Err(failure(
                    s.kind,
                    &format!("outcome:{expected}->{actual}"),
                    format!("step={index} {s:?}: expected {expected}, actual {reply:?}"),
                ));
            }
            match reply {
                ServerMessage::TerminalViewerResult(result) => {
                    if let Outcome::Attached { attachment_id, .. } = result.into_outcome() {
                        effects.push(Effect::Attach(Oracle::bytes(&attachment_id), s.target));
                        oracle.viewers.insert(
                            s.viewer,
                            Viewer {
                                owner: s.session,
                                target: s.target,
                                id: attachment_id,
                                live: true,
                                binding_attach: s.alternate,
                                grant: None,
                                sequence: 0,
                                next_frame: 0,
                                flight: None,
                            },
                        );
                    }
                }
                ServerMessage::TerminalControlResult(result) => {
                    if let ControlOutcome::Acquired { controller_id } = result.into_outcome() {
                        effects.push(Effect::Acquire(
                            Oracle::bytes(&viewer_id_for(&oracle, s.viewer)),
                            Oracle::controller_bytes(&controller_id),
                        ));
                        oracle.grants.insert(s.grant, controller_id);
                        oracle.viewers.get_mut(&s.viewer).unwrap().grant = Some(s.grant);
                    }
                }
                _ => unreachable!(),
            }
        }
        let all = fixture.barrier().map_err(|detail| {
            failure(
                s.kind,
                "fixture:barrier",
                format!("step={index} {s:?}: {detail}"),
            )
        })?;
        let actual = &all[consumed..];
        let sorted = |items: &[Effect]| {
            let mut text: Vec<_> = items.iter().map(|e| format!("{e:?}")).collect();
            text.sort();
            text
        };
        if sorted(actual) != sorted(&effects) {
            return Err(failure(
                s.kind,
                &effect_mismatch(&effects, actual),
                format!("step={index} {s:?}: expected {effects:?}, actual {actual:?}"),
            ));
        }
        for (at, effect) in actual.iter().enumerate() {
            if s.kind == Kind::Drop
                && let Effect::Release(viewer, _) = effect
                && actual[..at]
                    .iter()
                    .any(|e| matches!(e,Effect::Detach(id) if id==viewer))
            {
                return Err(failure(
                    s.kind,
                    "cleanup-order",
                    format!("step={index} {actual:?}"),
                ));
            }
        }
        if oracle.worker_lost
            && selected
                .as_ref()
                .is_some_and(|viewer| !oracle.disposed.contains(&viewer.target))
            && expected == "unavailable:TerminalRuntimeUnavailable"
            && matches!(s.kind, Kind::Poll | Kind::Control)
        {
            // Count only a reached stale-generation route with the expected typed refusal
            // and no observed request to the live replacement carrier (checked above).
            oracle.coverage.insert("worker-loss");
            oracle.coverage.insert(if s.kind == Kind::Control {
                "worker-generation-control"
            } else {
                "worker-generation-poll"
            });
        }
        // A window belongs to one Session and requires two distinct successfully
        // addressed live viewers. Repeated work on one viewer is not interleaving.
        if two
            && selected
                .as_ref()
                .is_some_and(|viewer| !oracle.disposed.contains(&viewer.target))
            && matches!(s.kind, Kind::Poll | Kind::Feedback | Kind::Control)
            && matches!(
                expected.as_str(),
                "record" | "empty" | "feedback" | "input" | "resize"
            )
        {
            if oracle.last_interleaved_viewer[s.session]
                .is_some_and(|previous| previous != s.viewer)
            {
                oracle.coverage.insert("interleaved");
            }
            oracle.last_interleaved_viewer[s.session] = Some(s.viewer);
        }
        // Observe the post-state so Detach/Dispose cannot bridge two separate
        // two-live windows even when the very next action installs another viewer.
        for session in 0..2 {
            if oracle.worker_lost
                || (s.kind == Kind::Drop && s.session == session)
                || oracle
                    .viewers
                    .values()
                    .filter(|viewer| {
                        viewer.live
                            && viewer.owner == session
                            && !oracle.disposed.contains(&viewer.target)
                    })
                    .count()
                    < 2
            {
                oracle.last_interleaved_viewer[session] = None;
            }
        }
        consumed = all.len();
    }
    drop(sessions);
    Ok(oracle.coverage)
}
fn viewer_id_for(oracle: &Oracle, slot: usize) -> ViewerId {
    oracle.viewer_id(slot)
}

fn effect_kind(effect: &Effect) -> &'static str {
    match effect {
        Effect::Attach(..) => "attach",
        Effect::Capacity => "capacity",
        Effect::Poll(..) => "poll",
        Effect::Feedback(..) => "feedback",
        Effect::Detach(..) => "detach",
        Effect::Acquire(..) => "acquire",
        Effect::Input(..) => "input",
        Effect::Resize(..) => "resize",
        Effect::Paste(..) => "paste",
        Effect::Release(..) => "release",
        Effect::Clear(..) => "clear",
        Effect::Dispose(..) => "dispose",
        Effect::ProcessInfo(..) => "process-info",
        Effect::GraphicsDecode(..) => "graphics-decode",
        Effect::DetachRefused(..) => "detach-refused",
        Effect::Lifecycle(..) => "lifecycle",
        Effect::PopupLaunch(..) => "popup-launch",
        Effect::PopupForce(..) => "popup-force",
        Effect::PopupInventory(..) => "popup-inventory",
        Effect::EditorCommand => "editor-command",
        Effect::EditorCapture(..) => "editor-capture",
        Effect::EditorStart => "editor-start",
    }
}
fn effect_mismatch(expected: &[Effect], actual: &[Effect]) -> String {
    let mut delta = BTreeMap::new();
    for effect in expected {
        *delta.entry(effect_kind(effect)).or_insert(0_i32) += 1;
    }
    for effect in actual {
        *delta.entry(effect_kind(effect)).or_insert(0_i32) -= 1;
    }
    delta.retain(|_, count| *count != 0);
    if !delta.is_empty() {
        return format!("effects:multiplicity:{delta:?}");
    }
    let sorted = |effects: &[Effect], axis: u8| {
        let mut values: Vec<_> = effects
            .iter()
            .filter_map(|effect| {
                let value = match (axis, effect) {
                    (
                        0,
                        Effect::Attach(viewer, _)
                        | Effect::Poll(viewer)
                        | Effect::Feedback(viewer)
                        | Effect::Detach(viewer)
                        | Effect::Acquire(viewer, _)
                        | Effect::Input(viewer, _, _)
                        | Effect::Resize(viewer, _, _)
                        | Effect::Release(viewer, _)
                        | Effect::Clear(viewer, _),
                    ) => u128::from_be_bytes(*viewer),
                    (
                        1,
                        Effect::Acquire(_, grant)
                        | Effect::Input(_, grant, _)
                        | Effect::Resize(_, grant, _)
                        | Effect::Release(_, grant)
                        | Effect::Clear(_, grant),
                    ) => u128::from_be_bytes(*grant),
                    (2, Effect::Attach(_, target) | Effect::Dispose(target)) => *target as u128,
                    (3, Effect::Input(_, _, sequence) | Effect::Resize(_, _, sequence)) => {
                        u128::from(*sequence)
                    }
                    _ => return None,
                };
                Some((effect_kind(effect), value))
            })
            .collect();
        values.sort();
        values
    };
    for (axis, name) in [
        (0, "viewer-route"),
        (1, "grant-route"),
        (2, "target"),
        (3, "sequence"),
    ] {
        if sorted(expected, axis) != sorted(actual, axis) {
            return format!("effects:{name}");
        }
    }
    // All individual axes match, but their binding tuple differs (for example grants
    // swapped between viewers). This is distinct from a missing or extra message.
    "effects:tuple-association".to_owned()
}

#[test]
fn effect_failure_fingerprint_distinguishes_route_sequence_and_multiplicity() {
    let expected = [Effect::Input([1; 16], [2; 16], 3)];
    assert_eq!(
        effect_mismatch(&expected, &[Effect::Input([9; 16], [2; 16], 3)]),
        "effects:viewer-route"
    );
    assert_eq!(
        effect_mismatch(&expected, &[Effect::Input([1; 16], [9; 16], 3)]),
        "effects:grant-route"
    );
    assert_eq!(
        effect_mismatch(&expected, &[Effect::Input([1; 16], [2; 16], 9)]),
        "effects:sequence"
    );
    assert_ne!(effect_mismatch(&expected, &[]), "effects:viewer-route");
    let renamed = [Effect::Input([4; 16], [5; 16], 3)];
    assert_eq!(
        effect_mismatch(&renamed, &[Effect::Input([6; 16], [5; 16], 3)]),
        "effects:viewer-route"
    );
}
#[test]
fn replay_allocation_symbols_cannot_overwrite_an_earlier_owner() {
    let attach = Step::new(Kind::Attach);
    let mut another = attach.clone();
    another.target = 1;
    let failure = run(&[attach, another]).unwrap_err();
    assert_eq!(
        failure.fingerprint,
        ("replay", "duplicate-viewer-symbol".to_owned())
    );
    let acquire = Step::new(Kind::Acquire);
    let mut another = acquire.clone();
    another.viewer = 1;
    let failure = run(&[acquire, another]).unwrap_err();
    assert_eq!(
        failure.fingerprint,
        ("replay", "duplicate-grant-symbol".to_owned())
    );
}
