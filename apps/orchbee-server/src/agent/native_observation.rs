//! `NativeObservation` — the private native observation reducer leaf.
//!
//! Production-active: the managed drain/registry path owns every call.
//! It owns held hints, the one current carrier's scalar-W sequence, and
//! the current committed bindings — a pure fold that classifies typed
//! facts and returns owned effect data. It never calls `AgentRuntime`,
//! the translator, I/O, the registry, or the coordinator: the
//! `with_state` closure applies the returned effects to the runtime-free
//! translator and the one runtime inside the same cell guard.
//!
//! Semantics are the selected model-32 contract: `seq<=W` dedup |
//! `seq==W+1` classify+commit | `seq>W+1` hole | gap clips the prefix
//! then must cover contiguously | a dead or absent carrier mints no ACK.
//! The production Worker connection generation IS the drain epoch — no
//! same-generation rotate and no drain token exists here; page-fence/
//! token/ackEcho prevalidation stays the carrier consumer's
//! obligation.

#[cfg(test)]
mod tests;

use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use orchbee_capability_node_protocol::{
    HostCreationId, HostTerminalIdentity, WorkerGeneration, WorkerId,
};
use orchbee_workbench_protocol::TerminalPresentationId;
use serde_json::Value;

use super::AgentModelError;
use super::hook_contract::parse_hint;

/// Logical bounds — held records and their retained raw payloads.
const MAXIMUM_HELD_RECORDS: usize = 1024;
const MAXIMUM_HELD_BYTES: usize = 4_194_304;
/// The wire's own application-signal payload bound; the typed
/// constructor enforces it before any parse or storage so an oversized
/// fact is unrepresentable inside the leaf.
const MAXIMUM_EVENT_PAYLOAD_BYTES: usize = 4096;
/// Held records expire ten seconds after first classification.
const HELD_TTL: Duration = Duration::from_secs(10);

/// The one current carrier: CNP `WorkerId` + `WorkerGeneration` — the
/// same typed pair `TerminalOperationAuthorization` already carries.
/// Fields are copied byte-exact from the authoritative caller at the
/// fact boundary; the workbench string `WorkerId` is never the carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CarrierIdentity {
    pub worker_id: WorkerId,
    pub generation: WorkerGeneration,
}

/// The typed domain fact the (future) wire/decode boundary produces.
/// `sequence` is carrier-owned and positive by construction; `raw` is
/// validated UTF-8 within the application payload bound at construction
/// — oversized input is rejected by the type, never stored or parsed.
#[derive(Debug)]
pub(crate) struct NativeEvent {
    carrier: CarrierIdentity,
    sequence: NonZeroU64,
    host: HostTerminalIdentity,
    raw: Box<str>,
}

impl NativeEvent {
    pub(crate) fn new(
        carrier: CarrierIdentity,
        sequence: NonZeroU64,
        host: HostTerminalIdentity,
        raw: impl Into<Box<str>>,
    ) -> Result<Self, AgentModelError> {
        let raw = raw.into();
        if raw.len() > MAXIMUM_EVENT_PAYLOAD_BYTES {
            return Err(AgentModelError(
                "native event payload exceeds the signal bound",
            ));
        }
        Ok(Self {
            carrier,
            sequence,
            host,
            raw,
        })
    }
}

/// Semantic equality unit for duplicate/conflict: the validated hint
/// fields only — `version=3` and `kind="correlation"` are fixed shape.
/// Byte, member-order, and whitespace differences classify Duplicate,
/// never Conflict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypedHint {
    pub correlation_id: String,
    pub event_id: String,
}

/// Held key: the six-field identity+correlation tuple
/// (worker, generation, creation, host_epoch, host_terminal,
/// correlation).
#[derive(Debug, Eq, PartialEq)]
struct HeldKey {
    carrier: CarrierIdentity,
    host: HostTerminalIdentity,
    correlation_id: String,
}

struct HeldRecord {
    hint: TypedHint,
    raw: Box<str>,
    received_at: Instant,
}

/// One committed binding: presentation -> (carrier, host). The leaf's
/// `bindings` Vec is the single mirror — there is no second
/// registrations map.
#[derive(Clone, Debug)]
pub(crate) struct Binding {
    pub presentation: TerminalPresentationId,
    pub carrier: CarrierIdentity,
    pub host: HostTerminalIdentity,
}

/// Owned effect data returned by the pure fold. Raw payloads MOVE into
/// the batch — nothing is cloned.
pub(crate) enum NativeObservationEffect {
    /// Replay one hint through the translator's signal path with the
    /// committed binding's presentation and the record's own creation.
    RouteHint {
        raw: Box<str>,
        host: HostTerminalIdentity,
        presentation: TerminalPresentationId,
    },
    /// Update the join fence: presentation -> creation generation.
    RegisterPresentation {
        presentation: TerminalPresentationId,
        creation_id: HostCreationId,
    },
    /// Drop the join fence for a retired or rebound presentation.
    RetirePresentation {
        presentation: TerminalPresentationId,
    },
}

/// Internal classes — one name per counter bucket. `Purged`/`Expired`
/// are counter-only: apply methods never return them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeClass {
    Held,
    Routed,
    Full,
    Expired,
    Duplicate,
    Conflict,
    Invalid,
    Purged,
    Dedup,
    Hole,
    Gap,
    Discard,
}

/// The fold's internal result: class + scalar-W ACK data value + the
/// owned effect batch. The service consumes the effects inside
/// `with_state` and surfaces only `NativeOutcome` — an already-applied
/// batch is never returned for duplicate replay.
pub(crate) struct NativeTransition {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "fold classification; only test seams read it — the drain fold consumes effects"
        )
    )]
    pub(crate) class: NativeClass,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "scalar-W ACK data value; ACK writer is the future caller outside the cell"
        )
    )]
    pub(crate) ack_w: Option<u64>,
    pub(crate) effects: Vec<NativeObservationEffect>,
}

struct NativeCounts {
    buckets: [u64; 12],
}

impl NativeCounts {
    fn bump(&mut self, class: NativeClass) {
        self.buckets[class as usize] = self.buckets[class as usize].saturating_add(1);
    }

    fn bump_by(&mut self, class: NativeClass, count: u64) {
        self.buckets[class as usize] = self.buckets[class as usize].saturating_add(count);
    }
}

/// The leaf state. `Vec` everywhere by deliberate choice: the held cap
/// is 1024 so linear scans stay trivial, insertion order IS the
/// original admission order reconcile replays, and `HostTerminalIdentity`
/// carries no Hash/Ord so no std map key is expressible anyway.
pub(crate) struct NativeObservation {
    carrier: Option<(CarrierIdentity, u64)>,
    records: Vec<(HeldKey, HeldRecord)>,
    held_bytes: usize,
    bindings: Vec<Binding>,
    counts: NativeCounts,
}

impl NativeObservation {
    pub(crate) fn new() -> Self {
        Self {
            carrier: None,
            records: Vec::new(),
            held_bytes: 0,
            bindings: Vec::new(),
            counts: NativeCounts { buckets: [0; 12] },
        }
    }

    /// Entry-head expiry: removes records whose first-classification
    /// stamp aged past the TTL. Every leaf entry runs it, including
    /// reconcile — an expired record can never replay on a late binding.
    /// Emits no translator effects; idle retained storage stays bounded
    /// by the caps until the next entry or a future poll.
    pub(crate) fn sweep_expired(&mut self, now: Instant) {
        let mut expired = 0u64;
        let mut held_bytes = self.held_bytes;
        self.records.retain(|(_key, record)| {
            if now.saturating_duration_since(record.received_at) >= HELD_TTL {
                held_bytes -= record.raw.len();
                expired += 1;
                false
            } else {
                true
            }
        });
        self.held_bytes = held_bytes;
        self.counts.bump_by(NativeClass::Expired, expired);
    }

    /// Registry fact: install the sole live carrier at W=0. Refuses when
    /// a carrier is already active — the registry serializes admit/retire
    /// so this arm is defensive only. Sweeps first like every entry
    /// point; the held set is necessarily empty here (records only ever
    /// belong to the carrier that was current at admission).
    pub(crate) fn admit_carrier(
        &mut self,
        carrier: CarrierIdentity,
        now: Instant,
    ) -> Result<(), AgentModelError> {
        self.sweep_expired(now);
        if self.carrier.is_some() {
            return Err(AgentModelError("carrier already active"));
        }
        self.carrier = Some((carrier, 0));
        Ok(())
    }

    /// Registry fact: carrier death. Only an exact (worker, generation)
    /// match retires — a stale-generation fact is a no-op, mirroring the
    /// registry's own generation-checked retire. Removes the drain,
    /// every held record of the carrier, and returns a
    /// `RetirePresentation` effect for EVERY binding of the carrier so
    /// the caller can drop the join fences in the same fold. No
    /// tombstone, no history.
    pub(crate) fn retire_carrier(
        &mut self,
        carrier: &CarrierIdentity,
        now: Instant,
    ) -> Vec<NativeObservationEffect> {
        self.sweep_expired(now);
        let mut effects = Vec::new();
        let Some((current, _w)) = &self.carrier else {
            return effects;
        };
        if current != carrier {
            return effects;
        }
        self.carrier = None;
        let mut purged = 0u64;
        let mut held_bytes = self.held_bytes;
        self.records.retain(|(key, record)| {
            if key.carrier == *carrier {
                held_bytes -= record.raw.len();
                purged += 1;
                false
            } else {
                true
            }
        });
        self.held_bytes = held_bytes;
        self.counts.bump_by(NativeClass::Purged, purged);
        let mut kept = Vec::with_capacity(self.bindings.len());
        for binding in std::mem::take(&mut self.bindings) {
            if binding.carrier == *carrier {
                effects.push(NativeObservationEffect::RetirePresentation {
                    presentation: binding.presentation,
                });
            } else {
                kept.push(binding);
            }
        }
        self.bindings = kept;
        effects
    }

    /// Ordered event on the current carrier. Ordering inside the single
    /// critical section: sweep -> carrier fence -> dedup -> hole ->
    /// classify -> atomic W commit. EVERY classify arm commits W —
    /// Invalid and Full consume the sequence so W never wedges.
    pub(crate) fn apply_event(&mut self, event: NativeEvent, now: Instant) -> NativeTransition {
        self.sweep_expired(now);
        let Some((current, w)) = self.carrier else {
            self.counts.bump(NativeClass::Discard);
            return NativeTransition {
                class: NativeClass::Discard,
                ack_w: None,
                effects: Vec::new(),
            };
        };
        if current != event.carrier {
            self.counts.bump(NativeClass::Discard);
            return NativeTransition {
                class: NativeClass::Discard,
                ack_w: None,
                effects: Vec::new(),
            };
        }
        let seq = event.sequence.get();
        if seq <= w {
            self.counts.bump(NativeClass::Dedup);
            return NativeTransition {
                class: NativeClass::Dedup,
                ack_w: Some(w),
                effects: Vec::new(),
            };
        }
        if seq - w > 1 {
            self.counts.bump(NativeClass::Hole);
            return NativeTransition {
                class: NativeClass::Hole,
                ack_w: Some(w),
                effects: Vec::new(),
            };
        }
        let (class, effects) = self.classify(event, now);
        self.carrier = Some((current, seq));
        self.counts.bump(class);
        NativeTransition {
            class,
            ack_w: Some(seq),
            effects,
        }
    }

    fn classify(
        &mut self,
        event: NativeEvent,
        now: Instant,
    ) -> (NativeClass, Vec<NativeObservationEffect>) {
        let parsed = serde_json::from_str::<Value>(&event.raw)
            .ok()
            .and_then(|value| parse_hint(&value));
        let Some(parsed_hint) = parsed else {
            return (NativeClass::Invalid, Vec::new());
        };
        let hint = TypedHint {
            correlation_id: parsed_hint.correlation_id,
            event_id: parsed_hint.event_id,
        };
        if let Some(binding) = self
            .bindings
            .iter()
            .find(|binding| binding.carrier == event.carrier && binding.host == event.host)
        {
            return (
                NativeClass::Routed,
                vec![NativeObservationEffect::RouteHint {
                    raw: event.raw,
                    host: event.host,
                    presentation: binding.presentation.clone(),
                }],
            );
        }
        let key = HeldKey {
            carrier: event.carrier,
            host: event.host,
            correlation_id: hint.correlation_id.clone(),
        };
        if let Some((_key, existing)) = self.records.iter().find(|(k, _record)| *k == key) {
            return (
                if existing.hint == hint {
                    NativeClass::Duplicate
                } else {
                    NativeClass::Conflict
                },
                Vec::new(),
            );
        }
        // The byte arm is implied by (count<=1024, raw<=4096) but kept
        // as an explicit guard — it can never fire independently.
        if self.records.len() >= MAXIMUM_HELD_RECORDS
            || self.held_bytes + event.raw.len() > MAXIMUM_HELD_BYTES
        {
            return (NativeClass::Full, Vec::new());
        }
        self.held_bytes += event.raw.len();
        self.records.push((
            key,
            HeldRecord {
                hint,
                raw: event.raw,
                received_at: now,
            },
        ));
        (NativeClass::Held, Vec::new())
    }

    /// Gap cover on the current carrier — wire-side structural validity
    /// (positive lo<=hi) is re-asserted defensively. A fully-covered gap
    /// (`hi<=W`) is a dedup-equivalent noop ACK, never Hole. Covering
    /// advances W only; already-held classified records are not purged
    /// and unclassified covered sequences become accounted-gap.
    pub(crate) fn apply_gap(
        &mut self,
        carrier: &CarrierIdentity,
        lo: u64,
        hi: u64,
        now: Instant,
    ) -> NativeTransition {
        self.sweep_expired(now);
        let Some((current, w)) = self.carrier else {
            self.counts.bump(NativeClass::Discard);
            return NativeTransition {
                class: NativeClass::Discard,
                ack_w: None,
                effects: Vec::new(),
            };
        };
        if current != *carrier || lo == 0 || lo > hi {
            self.counts.bump(NativeClass::Discard);
            return NativeTransition {
                class: NativeClass::Discard,
                ack_w: None,
                effects: Vec::new(),
            };
        }
        if hi <= w {
            self.counts.bump(NativeClass::Dedup);
            return NativeTransition {
                class: NativeClass::Dedup,
                ack_w: Some(w),
                effects: Vec::new(),
            };
        }
        // hi > w, so w < u64::MAX and w+1 cannot overflow.
        if lo.max(w + 1) != w + 1 {
            self.counts.bump(NativeClass::Hole);
            return NativeTransition {
                class: NativeClass::Hole,
                ack_w: Some(w),
                effects: Vec::new(),
            };
        }
        self.carrier = Some((current, hi));
        self.counts.bump(NativeClass::Gap);
        NativeTransition {
            class: NativeClass::Gap,
            ack_w: Some(hi),
            effects: Vec::new(),
        }
    }

    /// Complete committed-binding snapshot (coordinator fact). Sweeps
    /// first, then validates the WHOLE snapshot — bound, unique
    /// presentations, one-to-one identities — before any mutation; an
    /// ambiguous or oversized snapshot is rejected entirely with the
    /// existing internal contract error and leaves bindings plus the
    /// translator untouched. Entries not matching the current carrier
    /// are dropped: no arbitrary fact can activate a carrier.
    ///
    /// Ordering: retire old/rebound presentations (join fence down) and
    /// purge their old-identity held FIRST, then register new/rebound
    /// presentations, then route eligible held in original admission
    /// order. Unchanged facts emit no effects — no re-register, no TTL
    /// refresh, no duplicate replay.
    pub(crate) fn reconcile_complete_bindings(
        &mut self,
        snapshot: &[Binding],
        now: Instant,
    ) -> Result<Vec<NativeObservationEffect>, AgentModelError> {
        self.sweep_expired(now);
        if snapshot.len() > MAXIMUM_HELD_RECORDS {
            return Err(AgentModelError("binding snapshot exceeds the bound"));
        }
        for (index, left) in snapshot.iter().enumerate() {
            for right in &snapshot[index + 1..] {
                if left.presentation == right.presentation
                    || (left.carrier == right.carrier && left.host == right.host)
                {
                    return Err(AgentModelError("binding snapshot is not one-to-one"));
                }
            }
        }
        let current = self.carrier.map(|(carrier, _w)| carrier);
        let mut new_bindings = Vec::with_capacity(snapshot.len());
        for binding in snapshot {
            if Some(binding.carrier) == current {
                new_bindings.push(binding.clone());
            }
        }
        let mut effects = Vec::new();
        let mut purge_identities = Vec::new();
        for old in &self.bindings {
            let unchanged = new_bindings.iter().any(|new| {
                new.presentation == old.presentation
                    && new.carrier == old.carrier
                    && new.host == old.host
            });
            if unchanged {
                continue;
            }
            effects.push(NativeObservationEffect::RetirePresentation {
                presentation: old.presentation.clone(),
            });
            let old_survives = new_bindings
                .iter()
                .any(|new| new.carrier == old.carrier && new.host == old.host);
            if !old_survives {
                purge_identities.push((old.carrier, old.host));
            }
        }
        if !purge_identities.is_empty() {
            let mut purged = 0u64;
            let mut held_bytes = self.held_bytes;
            self.records.retain(|(key, record)| {
                if purge_identities.contains(&(key.carrier, key.host)) {
                    held_bytes -= record.raw.len();
                    purged += 1;
                    false
                } else {
                    true
                }
            });
            self.held_bytes = held_bytes;
            self.counts.bump_by(NativeClass::Purged, purged);
        }
        for new in &new_bindings {
            let unchanged = self.bindings.iter().any(|old| {
                old.presentation == new.presentation
                    && old.carrier == new.carrier
                    && old.host == new.host
            });
            if !unchanged {
                effects.push(NativeObservationEffect::RegisterPresentation {
                    presentation: new.presentation.clone(),
                    creation_id: new.host.creation_id(),
                });
            }
        }
        self.bindings = new_bindings;
        // Route eligible held in original admission order — Vec order
        // IS admission order. Records move out; nothing is cloned.
        let mut routed = 0u64;
        let mut held_bytes = self.held_bytes;
        let mut remaining = Vec::with_capacity(self.records.len());
        for (key, record) in std::mem::take(&mut self.records) {
            if let Some(binding) = self
                .bindings
                .iter()
                .find(|binding| binding.carrier == key.carrier && binding.host == key.host)
            {
                held_bytes -= record.raw.len();
                routed += 1;
                effects.push(NativeObservationEffect::RouteHint {
                    raw: record.raw,
                    host: key.host,
                    presentation: binding.presentation.clone(),
                });
            } else {
                remaining.push((key, record));
            }
        }
        self.records = remaining;
        self.held_bytes = held_bytes;
        self.counts.bump_by(NativeClass::Routed, routed);
        Ok(effects)
    }

    /// Test-only observability of the held count — the sweep/cap arms
    /// assert through it instead of introspecting private state.
    #[cfg(test)]
    pub(crate) fn held_count(&self) -> usize {
        self.records.len()
    }

    /// The one live carrier tuple: the full identity plus its scalar `W`.
    /// Read-only observability for the drain page/final-fence
    /// folds — neither an empty page nor the final fold can learn the
    /// live watermark from transition returns alone. No new state, no
    /// token, no second watermark.
    pub(crate) fn current_carrier(&self) -> Option<(CarrierIdentity, u64)> {
        self.carrier
    }
}
