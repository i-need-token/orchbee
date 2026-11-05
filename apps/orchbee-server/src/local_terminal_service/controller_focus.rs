//! Server-owned focus order for exact physical terminals. Polling the selected surviving viewer
//! completes promotion through the ordinary fenced controller and resize paths; no lock spans IPC.
use super::{
    LocalTerminalService,
    controller_connection::{self, ControlTarget, ReservationPhase},
    viewer_connection::ViewerAttachment,
};
use orchbee_client_server_protocol::{
    Geometry, RequestId, TerminalControlAction as Action, TerminalControlOutcome as Outcome,
    TerminalControlRequest, TerminalControlState, ViewerId,
};
use std::collections::BTreeMap;

#[derive(Clone)]
struct Focus {
    viewer: ViewerId,
    geometry: Geometry,
}

#[derive(Default)]
pub(super) struct FocusRegistry {
    terminals: BTreeMap<ControlTarget, Vec<Focus>>,
}
impl FocusRegistry {
    pub(super) fn prefers(&self, target: ControlTarget, viewer: &ViewerId) -> bool {
        self.terminals
            .get(&target)
            .and_then(|rows| rows.first())
            .is_some_and(|row| &row.viewer == viewer)
    }
    pub(super) fn remove(&mut self, target: ControlTarget, viewer: &ViewerId) {
        if let Some(rows) = self.terminals.get_mut(&target) {
            rows.retain(|row| &row.viewer != viewer);
            if rows.is_empty() {
                self.terminals.remove(&target);
            }
        }
    }
    fn record(
        &mut self,
        target: ControlTarget,
        viewer: ViewerId,
        geometry: Geometry,
        activate: bool,
    ) -> bool {
        if !self.terminals.contains_key(&target) && self.terminals.len() >= 256 {
            return false;
        }
        let rows = self.terminals.entry(target).or_default();
        if let Some(index) = rows.iter().position(|row| row.viewer == viewer) {
            if !activate {
                rows[index].geometry = geometry;
                return true;
            }
            rows.remove(index);
        } else if rows.len() >= 8 {
            return false;
        }
        rows.insert(0, Focus { viewer, geometry });
        true
    }
}

impl LocalTerminalService {
    pub(super) fn control_observation(
        &self,
        viewer: &ViewerAttachment<'_>,
        geometry: Option<Geometry>,
    ) -> Outcome {
        let target = ControlTarget::from(viewer.target);
        let Ok(owners) = self.controllers.lock() else {
            return Outcome::OutcomeUnknown {};
        };
        let Ok(mut focus) = self.controller_focus.lock() else {
            return Outcome::OutcomeUnknown {};
        };
        if let Some(geometry) = geometry
            && !focus.record(target, viewer.attachment_id.clone(), geometry, true)
        {
            return Outcome::Unavailable {
                reason: orchbee_client_server_protocol::ControlUnavailableReason::ServerBusy,
            };
        }
        let owner = owners
            .get(&target)
            .filter(|owner| owner.phase == ReservationPhase::Active);
        let id = |bytes| format!("{:032x}", u128::from_be_bytes(bytes));
        Outcome::Observed {
            state: Box::new(TerminalControlState {
                geometry: owner
                    .filter(|owner| id(owner.viewer_id.as_bytes()) == viewer.attachment_id.as_str())
                    .and(viewer.controller.as_ref())
                    .and_then(controller_connection::ControllerLease::geometry),
                sequence: owner
                    .filter(|owner| id(owner.viewer_id.as_bytes()) == viewer.attachment_id.as_str())
                    .and(viewer.controller.as_ref())
                    .map_or(0, controller_connection::ControllerLease::sequence),
                controller: owner
                    .and_then(|owner| ViewerId::parse(&id(owner.viewer_id.as_bytes())).ok()),
                controller_id: owner
                    .filter(|owner| id(owner.viewer_id.as_bytes()) == viewer.attachment_id.as_str())
                    .and_then(|owner| {
                        orchbee_client_server_protocol::ControllerId::parse(&id(owner
                            .controller_id
                            .as_bytes()))
                        .ok()
                    }),
                recent_focus: focus.terminals.get(&target).map_or_else(Vec::new, |rows| {
                    rows.iter().map(|row| row.viewer.clone()).collect()
                }),
            }),
        }
    }

    pub(super) fn remember_control_geometry(
        &self,
        viewer: &ViewerId,
        target: ControlTarget,
        geometry: Geometry,
    ) {
        if let Ok(mut focus) = self.controller_focus.lock() {
            focus.record(target, viewer.clone(), geometry, false);
        }
    }

    pub(super) fn promote_focused_viewer<'s>(&'s self, viewer: &mut ViewerAttachment<'s>) {
        let target = ControlTarget::from(viewer.target);
        let geometry = {
            let Ok(owners) = self.controllers.lock() else {
                return;
            };
            if owners.contains_key(&target) {
                return;
            }
            let Ok(focus) = self.controller_focus.lock() else {
                return;
            };
            let Some(candidate) = focus.terminals.get(&target).and_then(|rows| rows.first()) else {
                return;
            };
            if candidate.viewer != viewer.attachment_id {
                return;
            }
            candidate.geometry
        };
        let Outcome::Acquired { controller_id } =
            controller_connection::acquire_promoted(self, viewer)
        else {
            return;
        };
        let Some(operation) = self.next_viewer_operation() else {
            return;
        };
        let Ok(request_id) = RequestId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(operation.as_bytes())
        )) else {
            return;
        };
        let attachment_id = viewer.attachment_id.clone();
        let _ = controller_connection::execute(
            self,
            Some(viewer),
            &TerminalControlRequest::new(
                request_id,
                Action::Resize {
                    attachment_id,
                    controller_id,
                    sequence: std::num::NonZeroU64::MIN,
                    geometry,
                },
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Timestamp ownership is an independent oracle for the ordered production registry.
    #[test]
    fn bounded_focus_resize_and_departure_traces_preserve_recent_live_viewers_and_physical_scope() {
        for encoded in 0..9_usize.pow(4) {
            let mut trace = encoded;
            let mut actual = FocusRegistry::default();
            let mut oracle = BTreeMap::<(ControlTarget, ViewerId), (usize, Geometry)>::new();
            for tick in 1..=4 {
                let event = trace % 9;
                trace /= 9;
                let viewer = ViewerId::parse(&format!("{:032x}", event % 3 + 1)).unwrap();
                let target = ControlTarget {
                    worker: [1; 16],
                    generation: tick as u64 % 2 + 1,
                    host_epoch: [2; 16],
                    terminal: [3; 16],
                };
                let geometry = Geometry::new(80 + tick as u16, 24, 8, 16).unwrap();
                match event / 3 {
                    0 => {
                        assert!(actual.record(target, viewer.clone(), geometry, true));
                        oracle.insert((target, viewer), (tick, geometry));
                    }
                    1 => {
                        assert!(actual.record(target, viewer.clone(), geometry, false));
                        let order = oracle
                            .get(&(target, viewer.clone()))
                            .map_or(tick, |(order, _)| *order);
                        oracle.insert((target, viewer), (order, geometry));
                    }
                    _ => {
                        actual.remove(target, &viewer);
                        oracle.remove(&(target, viewer));
                    }
                }
                for generation in [1, 2] {
                    let scope = ControlTarget {
                        generation,
                        ..target
                    };
                    let mut expected: Vec<_> = oracle
                        .iter()
                        .filter(|((target, _), _)| *target == scope)
                        .collect();
                    expected.sort_by_key(|(_, (at, _))| std::cmp::Reverse(*at));
                    let rows = actual.terminals.get(&scope).map_or(&[][..], Vec::as_slice);
                    let observed: Vec<_> =
                        rows.iter().map(|row| (&row.viewer, row.geometry)).collect();
                    let expected: Vec<_> = expected
                        .iter()
                        .map(|((_, viewer), (_, geometry))| (viewer, *geometry))
                        .collect();
                    assert_eq!(
                        observed, expected,
                        "trace {encoded}, tick {tick}, generation {generation}"
                    );
                    for n in 1..=3 {
                        let viewer = ViewerId::parse(&format!("{n:032x}")).unwrap();
                        assert_eq!(
                            actual.prefers(scope, &viewer),
                            expected.first().is_some_and(|(first, _)| **first == viewer)
                        );
                    }
                }
            }
        }
    }
}
