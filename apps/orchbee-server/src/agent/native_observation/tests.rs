//! Pure-leaf coverage of the native observation reducer:
//! carrier admit/retire fencing, scalar-W sequence and gap semantics,
//! typed duplicate/conflict, expiry sweeps, held caps, the typed
//! payload bound, and complete-snapshot reconcile atomicity. The
//! same-runtime bridge coverage (held -> bound -> replay -> dispatch ->
//! canonical read, and the shared poison boundary) lives in
//! `agent_service/tests.rs` where the real `AgentService` exists.

use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, WorkerGeneration, WorkerId,
};
use orchbee_workbench_protocol::TerminalPresentationId;

use super::{
    Binding, CarrierIdentity, NativeClass, NativeEvent, NativeObservation, NativeObservationEffect,
};

fn leaf() -> NativeObservation {
    NativeObservation::new()
}

fn carrier_id(worker_seed: u8, generation: u64) -> CarrierIdentity {
    CarrierIdentity {
        worker_id: WorkerId::from_bytes([worker_seed; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
    }
}

fn host(seed: u8) -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([seed; 16]).unwrap(),
        HostEpoch::from_bytes([seed.wrapping_add(16); 16]).unwrap(),
        HostTerminalId::from_bytes([seed.wrapping_add(32); 16]).unwrap(),
    )
}

fn presentation(seed: u8) -> TerminalPresentationId {
    TerminalPresentationId::parse(&format!("{seed:032x}")).unwrap()
}

fn binding(seed: u8, carrier: CarrierIdentity) -> Binding {
    Binding {
        presentation: presentation(seed),
        carrier,
        host: host(seed),
    }
}

fn hint_json(correlation: u64, event: u64) -> String {
    format!(
        "{{\"correlationId\":\"correlation-{correlation:016}\",\
         \"eventId\":\"event-{event:016}\",\
         \"kind\":\"correlation\",\"version\":3}}"
    )
}

fn event(
    carrier: CarrierIdentity,
    sequence: u64,
    host: HostTerminalIdentity,
    raw: impl Into<Box<str>>,
) -> NativeEvent {
    NativeEvent::new(carrier, NonZeroU64::new(sequence).unwrap(), host, raw).unwrap()
}

fn class_of(transition: &ObservedTransition) -> (NativeClass, Option<u64>) {
    (transition.class, transition.ack_w)
}

fn effect_kinds(effects: &[NativeObservationEffect]) -> Vec<&'static str> {
    effects
        .iter()
        .map(|effect| match effect {
            NativeObservationEffect::RouteHint { .. } => "route",
            NativeObservationEffect::RegisterPresentation { .. } => "register",
            NativeObservationEffect::RetirePresentation { .. } => "retire",
        })
        .collect()
}

#[test]
fn carrier_admit_retire_and_stale_fencing() {
    let mut leaf = leaf();
    let t0 = Instant::now();
    let old = carrier_id(1, 1);
    let new = carrier_id(1, 2);

    // Events with no current carrier are discarded without an ACK.
    let outcome = leaf.apply_event_observed(event(old, 1, host(1), hint_json(1, 1)), t0);
    assert_eq!(class_of(&outcome), (NativeClass::Discard, None));

    leaf.admit_carrier(old, t0).unwrap();
    // Double admission is a contract refusal, never a silent overwrite.
    assert!(leaf.admit_carrier(new, t0).is_err());

    leaf.reconcile_complete_bindings(&[binding(1, old)], t0)
        .unwrap();
    let held = leaf.apply_event_observed(event(old, 1, host(2), hint_json(2, 2)), t0);
    assert_eq!(held.class, NativeClass::Held);
    assert_eq!(leaf.held_count(), 1);

    // Retire: held purged atomically and one RetirePresentation per
    // binding of the exact carrier is returned for the caller's fold.
    let effects = leaf.retire_carrier(&old, t0);
    assert_eq!(effect_kinds(&effects), vec!["retire"]);
    match &effects[0] {
        NativeObservationEffect::RetirePresentation {
            presentation: retired,
        } => {
            assert_eq!(*retired, presentation(1));
        }
        _ => panic!("expected a retire effect"),
    }
    assert_eq!(leaf.held_count(), 0);

    // A stale retire fact on the already-dead carrier is a no-op.
    assert!(leaf.retire_carrier(&old, t0).is_empty());

    // Replacement carrier: fresh scalar W; a stale-generation event is
    // discarded without minting an ACK.
    leaf.admit_carrier(new, t0).unwrap();
    assert!(leaf.retire_carrier(&old, t0).is_empty());
    let stale = leaf.apply_event_observed(event(old, 2, host(1), hint_json(3, 3)), t0);
    assert_eq!(class_of(&stale), (NativeClass::Discard, None));
    leaf.reconcile_complete_bindings(&[binding(1, new)], t0)
        .unwrap();
    let routed = leaf.apply_event_observed(event(new, 1, host(1), hint_json(4, 4)), t0);
    assert_eq!(class_of(&routed), (NativeClass::Routed, Some(1)));
    assert_eq!(effect_kinds(&routed.effects), vec!["route"]);
}

#[test]
fn typed_duplicate_conflict_and_invalid_consume_sequence() {
    let mut leaf = leaf();
    let t0 = Instant::now();
    let carrier = carrier_id(1, 1);
    let host = host(1);
    leaf.admit_carrier(carrier, t0).unwrap();

    let first = leaf.apply_event_observed(event(carrier, 1, host, hint_json(1, 1)), t0);
    assert_eq!(first.class, NativeClass::Held);

    // Same semantic hint, reordered members -> Duplicate, never
    // Conflict: equality is typed, not bytewise.
    let reordered = "{\"version\":3,\"kind\":\"correlation\",\
        \"eventId\":\"event-0000000000000001\",\
        \"correlationId\":\"correlation-0000000000000001\"}";
    let duplicate = leaf.apply_event_observed(event(carrier, 2, host, reordered), t0);
    assert_eq!(class_of(&duplicate), (NativeClass::Duplicate, Some(2)));
    assert_eq!(leaf.held_count(), 1);

    // Same correlation identity, different event identity -> Conflict.
    let conflict = leaf.apply_event_observed(event(carrier, 3, host, hint_json(1, 9)), t0);
    assert_eq!(class_of(&conflict), (NativeClass::Conflict, Some(3)));
    assert_eq!(leaf.held_count(), 1);

    // Invalid JSON still consumes the sequence and commits W.
    let invalid = leaf.apply_event_observed(event(carrier, 4, host, "not json"), t0);
    assert_eq!(class_of(&invalid), (NativeClass::Invalid, Some(4)));
    assert_eq!(leaf.held_count(), 1);

    // A schema-invalid hint (wrong kind) is likewise consumed.
    let wrong_kind = "{\"correlationId\":\"correlation-0000000000000005\",\"eventId\":\"event-0000000000000005\",\
         \"kind\":\"other\",\"version\":3}";
    let schema_invalid = leaf.apply_event_observed(event(carrier, 5, host, wrong_kind), t0);
    assert_eq!(class_of(&schema_invalid), (NativeClass::Invalid, Some(5)));
}

#[test]
fn expiry_sweeps_at_every_entry_head() {
    let mut leaf = leaf();
    let t0 = Instant::now();
    let carrier = carrier_id(1, 1);
    leaf.admit_carrier(carrier, t0).unwrap();
    let held = leaf.apply_event_observed(event(carrier, 1, host(1), hint_json(1, 1)), t0);
    assert_eq!(held.class, NativeClass::Held);
    assert_eq!(leaf.held_count(), 1);

    // Past the TTL the next entry sweeps first — here reconcile.
    let late = t0 + Duration::from_secs(11);
    let effects = leaf
        .reconcile_complete_bindings(&[binding(1, carrier)], late)
        .unwrap();
    // The register effect fires but the expired record is already gone:
    // no RouteHint can be minted for it.
    assert_eq!(effect_kinds(&effects), vec!["register"]);
    assert_eq!(leaf.held_count(), 0);

    // Explicit sweep is idempotent.
    leaf.sweep_expired(late + Duration::from_secs(1));
    assert_eq!(leaf.held_count(), 0);

    // An earlier-than-origin timestamp saturates instead of expiring
    // everything: a record stamped at t0 is NOT expired when swept at t0.
    let fresh = leaf.apply_event_observed(event(carrier, 2, host(2), hint_json(2, 2)), t0);
    assert_eq!(fresh.class, NativeClass::Held);
    leaf.sweep_expired(t0);
    assert_eq!(leaf.held_count(), 1);
}

#[test]
fn held_cap_and_typed_payload_bound() {
    let mut leaf = leaf();
    let t0 = Instant::now();
    let carrier = carrier_id(1, 1);
    let host = host(1);
    leaf.admit_carrier(carrier, t0).unwrap();

    for sequence in 1..=1024u64 {
        let outcome = leaf.apply_event_observed(
            event(carrier, sequence, host, hint_json(sequence + 100, sequence)),
            t0,
        );
        assert_eq!(outcome.class, NativeClass::Held);
    }
    assert_eq!(leaf.held_count(), 1024);

    // The cap rejects the NEWEST record and still consumes the
    // sequence: W commits, no record is evicted to make room.
    let overflow = leaf.apply_event_observed(event(carrier, 1025, host, hint_json(2000, 2000)), t0);
    assert_eq!(class_of(&overflow), (NativeClass::Full, Some(1025)));
    assert_eq!(leaf.held_count(), 1024);

    // The typed boundary: >4096 raw bytes is unrepresentable input,
    // rejected by the constructor rather than stored or parsed.
    let oversized = "x".repeat(4097);
    assert!(
        NativeEvent::new(
            carrier,
            NonZeroU64::new(1026).unwrap(),
            host,
            oversized.into_boxed_str(),
        )
        .is_err()
    );
    assert!(
        NativeEvent::new(
            carrier,
            NonZeroU64::new(1026).unwrap(),
            host,
            "x".repeat(4096).into_boxed_str(),
        )
        .is_ok()
    );
}

#[test]
fn scalar_w_sequence_and_gap_semantics() {
    let mut leaf = leaf();
    let t0 = Instant::now();
    let carrier = carrier_id(1, 1);
    let stale = carrier_id(2, 9);
    let host = host(1);
    leaf.admit_carrier(carrier, t0).unwrap();

    // A hole does not move W and ACKs the current W.
    let hole = leaf.apply_event_observed(event(carrier, 5, host, hint_json(10, 10)), t0);
    assert_eq!(class_of(&hole), (NativeClass::Hole, Some(0)));

    // The contiguous prefix commits in order.
    let first = leaf.apply_event_observed(event(carrier, 1, host, hint_json(11, 11)), t0);
    assert_eq!(class_of(&first), (NativeClass::Held, Some(1)));
    let second = leaf.apply_event_observed(event(carrier, 2, host, hint_json(12, 12)), t0);
    assert_eq!(class_of(&second), (NativeClass::Held, Some(2)));

    // Replay of a committed sequence dedups against W.
    let dedup = leaf.apply_event_observed(event(carrier, 1, host, hint_json(11, 11)), t0);
    assert_eq!(class_of(&dedup), (NativeClass::Dedup, Some(2)));

    // Gap [3,5] fills the pending hole contiguously and ACKs hi.
    let covered = leaf.apply_gap_observed(&carrier, 3, 5, t0);
    assert_eq!(class_of(&covered), (NativeClass::Gap, Some(5)));

    // A fully-covered gap is a dedup-equivalent noop ACK, never Hole.
    let noop = leaf.apply_gap_observed(&carrier, 1, 5, t0);
    assert_eq!(class_of(&noop), (NativeClass::Dedup, Some(5)));

    // A gap that cannot clip to W+1 is a hole ACKing the current W.
    let clipped_hole = leaf.apply_gap_observed(&carrier, 7, 9, t0);
    assert_eq!(class_of(&clipped_hole), (NativeClass::Hole, Some(5)));

    // Stale carrier: discard without minting an ACK.
    let stale_gap = leaf.apply_gap_observed(&stale, 1, 3, t0);
    assert_eq!(class_of(&stale_gap), (NativeClass::Discard, None));

    // Malformed gap shapes (lo>hi, lo==0) are discarded with no ACK.
    let malformed = leaf.apply_gap_observed(&carrier, 9, 6, t0);
    assert_eq!(class_of(&malformed), (NativeClass::Discard, None));
    let zero_lo = leaf.apply_gap_observed(&carrier, 0, 9, t0);
    assert_eq!(class_of(&zero_lo), (NativeClass::Discard, None));

    // Cover through u64::MAX: contiguous, overflow-safe.
    let max_cover = leaf.apply_gap_observed(&carrier, 6, u64::MAX, t0);
    assert_eq!(class_of(&max_cover), (NativeClass::Gap, Some(u64::MAX)));

    // Beyond the maximum sequence there is nothing left to deliver.
    let after_max = leaf.apply_gap_observed(&carrier, 1, u64::MAX, t0);
    assert_eq!(class_of(&after_max), (NativeClass::Dedup, Some(u64::MAX)));
}

#[test]
fn reconcile_snapshot_atomicity_removal_and_rebind() {
    let mut leaf = leaf();
    let t0 = Instant::now();
    let carrier = carrier_id(1, 1);
    leaf.admit_carrier(carrier, t0).unwrap();

    // Ambiguous snapshot (same presentation twice) -> whole snapshot
    // rejected; nothing mutates.
    let ambiguous = [binding(1, carrier), binding(1, carrier)];
    assert!(leaf.reconcile_complete_bindings(&ambiguous, t0).is_err());

    // Same full identity under two presentations is equally ambiguous.
    let first = binding(1, carrier);
    let mut second = binding(2, carrier);
    second.host = first.host;
    let colliding = [first, second];
    assert!(leaf.reconcile_complete_bindings(&colliding, t0).is_err());

    // Commit a binding; the event routes with a RouteHint effect
    // carrying the record's own host identity and the bound
    // presentation.
    let effects = leaf
        .reconcile_complete_bindings(&[binding(1, carrier)], t0)
        .unwrap();
    assert_eq!(effect_kinds(&effects), vec!["register"]);
    match &effects[0] {
        NativeObservationEffect::RegisterPresentation {
            presentation: registered,
            creation_id,
        } => {
            assert_eq!(*registered, presentation(1));
            assert_eq!(*creation_id, host(1).creation_id());
        }
        _ => panic!("expected a register effect"),
    }
    let routed = leaf.apply_event_observed(event(carrier, 1, host(1), hint_json(30, 30)), t0);
    assert_eq!(class_of(&routed), (NativeClass::Routed, Some(1)));
    match &routed.effects[0] {
        NativeObservationEffect::RouteHint {
            host: routed_host,
            presentation: routed_presentation,
            raw,
        } => {
            assert_eq!(*routed_host, host(1));
            assert_eq!(*routed_presentation, presentation(1));
            assert_eq!(&**raw, hint_json(30, 30));
        }
        _ => panic!("expected a route effect"),
    }

    // An unchanged snapshot emits nothing — no re-register, no replay.
    let noop = leaf
        .reconcile_complete_bindings(&[binding(1, carrier)], t0)
        .unwrap();
    assert!(noop.is_empty());
    let [committed] = leaf.bindings() else {
        panic!("unchanged snapshot must retain its sole committed binding");
    };
    assert_eq!(committed.presentation, presentation(1));
    assert_eq!(committed.carrier, carrier);
    assert_eq!(committed.host, host(1));

    // Removal retires the presentation; events for the old identity
    // are held again rather than routed.
    let removed = leaf.reconcile_complete_bindings(&[], t0).unwrap();
    assert_eq!(effect_kinds(&removed), vec!["retire"]);
    assert!(leaf.bindings().is_empty());
    let held_again = leaf.apply_event_observed(event(carrier, 2, host(1), hint_json(31, 31)), t0);
    assert_eq!(held_again.class, NativeClass::Held);
    assert_eq!(leaf.held_count(), 1);

    // Rebind the same presentation to a NEW host identity: the new
    // identity's held record replays onto the rebound presentation in
    // admission order; the never-rebound old identity's record stays
    // held (a shared presentation string never purges another
    // identity's records).
    let new_held = leaf.apply_event_observed(event(carrier, 3, host(7), hint_json(33, 33)), t0);
    assert_eq!(new_held.class, NativeClass::Held);
    let rebound = leaf
        .reconcile_complete_bindings(
            &[Binding {
                presentation: presentation(1),
                carrier,
                host: host(7),
            }],
            t0,
        )
        .unwrap();
    assert_eq!(effect_kinds(&rebound), vec!["register", "route"]);
    match &rebound[1] {
        NativeObservationEffect::RouteHint {
            host: routed_host,
            presentation: routed_presentation,
            ..
        } => {
            assert_eq!(*routed_host, host(7));
            assert_eq!(*routed_presentation, presentation(1));
        }
        _ => panic!("expected the replayed held record's route effect"),
    }
    assert_eq!(leaf.held_count(), 1);

    // A snapshot of only foreign carriers binds nothing but still
    // retires the live binding — no fact can revive a carrier.
    let foreign = [binding(9, carrier_id(9, 3))];
    let retired = leaf.reconcile_complete_bindings(&foreign, t0).unwrap();
    assert_eq!(effect_kinds(&retired), vec!["retire"]);
    let unrouted = leaf.apply_event_observed(event(carrier, 4, host(7), hint_json(34, 34)), t0);
    assert_eq!(class_of(&unrouted), (NativeClass::Held, Some(4)));

    // Oversized snapshot with unique presentations AND identities:
    // rejected whole on the bound check alone.
    let oversized: Vec<Binding> = (1..=1025u16)
        .map(|index| {
            let mut bytes = [0u8; 16];
            bytes[..2].copy_from_slice(&index.to_be_bytes());
            Binding {
                presentation: TerminalPresentationId::parse(&format!("{index:032x}")).unwrap(),
                carrier,
                host: HostTerminalIdentity::new(
                    HostCreationId::from_bytes(bytes).unwrap(),
                    HostEpoch::from_bytes(bytes).unwrap(),
                    HostTerminalId::from_bytes(bytes).unwrap(),
                ),
            }
        })
        .collect();
    assert!(leaf.reconcile_complete_bindings(&oversized, t0).is_err());
}

/// The translator's new registration seam — the reason it exists:
/// `environment_overlay_for` returns early when no socket path is
/// bound, so a native binding on a socket-less terminal must register
/// the presentation generation through `register_presentation_binding`
/// instead. Driven directly on the runtime-free translator with the
/// one runtime passed per call, exactly like the service fold does.
#[test]
fn register_presentation_binding_registers_without_a_socket() {
    use crate::agent::{AgentHookObservationIngress, AgentRuntime, JoinOptions};

    let mut runtime = AgentRuntime::new();
    // `None` socket path — the overlay seam would return early here.
    let mut ingress = AgentHookObservationIngress::new(None, JoinOptions::default());
    assert!(ingress.admit_envelope(&mut runtime, &envelope_body(9, 9), 0.0));

    let presentation = "desktop:presentation:c9";
    let generation = format!("{:032x}", 9u8);
    ingress.register_presentation_binding(
        &mut runtime,
        presentation.to_owned(),
        generation.clone(),
    );
    ingress.observe_terminal_application_signal(
        &mut runtime,
        &hint_json(9, 9),
        generation,
        presentation.to_owned(),
        0.0,
    );
    ingress.advance_settlement(&mut runtime, 1_000.0, 1_000.0);
    assert_eq!(runtime.snapshot().sessions.len(), 1);
}

#[test]
fn current_carrier_mirrors_the_canonical_tuple() {
    let mut leaf = leaf();
    let t0 = Instant::now();
    let carrier = carrier_id(7, 3);

    assert_eq!(leaf.current_carrier(), None);

    leaf.admit_carrier(carrier, t0).unwrap();
    assert_eq!(leaf.current_carrier(), Some((carrier, 0)));

    leaf.reconcile_complete_bindings(&[binding(1, carrier)], t0)
        .unwrap();
    let routed = leaf.apply_event_observed(event(carrier, 1, host(1), hint_json(1, 1)), t0);
    assert_eq!(routed.class, NativeClass::Routed);
    assert_eq!(leaf.current_carrier(), Some((carrier, 1)));

    leaf.retire_carrier(&carrier, t0);
    assert_eq!(leaf.current_carrier(), None);
}

fn envelope_body(correlation: u64, event: u64) -> Vec<u8> {
    serde_json::json!({
        "adapterId": "codex",
        "correlations": [{
            "correlationId": format!("correlation-{correlation:016}"),
            "visibility": "hidden",
        }],
        "eventId": format!("event-{event:016}"),
        "observation": {
            "activity": null,
            "agentKey": null,
            "event": "session-started",
            "operationKey": null,
            "sessionKey": "session-1",
            "turnKey": null,
            "version": 1,
        },
        "version": 3,
    })
    .to_string()
    .into_bytes()
}

/// Tests observe committed counters and watermark, rather than a second production result payload.
pub(crate) struct ObservedTransition {
    pub(crate) class: NativeClass,
    pub(crate) ack_w: Option<u64>,
    pub(crate) effects: Vec<NativeObservationEffect>,
}

impl NativeObservation {
    pub(crate) fn apply_event_observed(
        &mut self,
        event: NativeEvent,
        now: Instant,
    ) -> ObservedTransition {
        let before = self.counts.buckets;
        let transition = self.apply_event(event, now);
        self.observed_transition(before, transition)
    }
    pub(crate) fn apply_gap_observed(
        &mut self,
        carrier: &CarrierIdentity,
        lo: u64,
        hi: u64,
        now: Instant,
    ) -> ObservedTransition {
        let before = self.counts.buckets;
        let transition = self.apply_gap(carrier, lo, hi, now);
        self.observed_transition(before, transition)
    }
    fn observed_transition(
        &self,
        before: [u64; 12],
        transition: super::NativeTransition,
    ) -> ObservedTransition {
        let class = [
            NativeClass::Held,
            NativeClass::Routed,
            NativeClass::Full,
            NativeClass::Duplicate,
            NativeClass::Conflict,
            NativeClass::Invalid,
            NativeClass::Dedup,
            NativeClass::Hole,
            NativeClass::Gap,
            NativeClass::Discard,
        ]
        .into_iter()
        .find(|class| self.counts.buckets[*class as usize] > before[*class as usize])
        .expect("one event or gap increments one classification counter");
        ObservedTransition {
            class,
            ack_w: if class == NativeClass::Discard {
                None
            } else {
                self.current_carrier().map(|(_, watermark)| watermark)
            },
            effects: transition.effects,
        }
    }
}

impl NativeObservation {
    /// Test-only observability of the held count — the sweep/cap arms
    /// assert through it instead of introspecting private state.
    pub(crate) fn held_count(&self) -> usize {
        self.records.len()
    }
}
