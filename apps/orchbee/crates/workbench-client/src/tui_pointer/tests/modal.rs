use super::*;
use crate::workbench_client::WorkspaceCloseToken;

#[derive(Default)]
struct SetupPort {
    decisions: Vec<ModalControl>,
}
impl SetupPort {
    fn choose_setup_project(&mut self, _: crate::workbench_client::SetupToken, index: usize) {
        self.decisions.push(ModalControl::Choice(index));
    }

    fn submit_setup_draft(
        &mut self,
        _: crate::workbench_client::SetupToken,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        self.decisions.push(ModalControl::Submit);
        None
    }

    fn cancel_setup_draft(&mut self, _: crate::workbench_client::SetupToken) {
        self.decisions.push(ModalControl::Cancel);
    }
}

impl TerminalInputPort for SetupPort {
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{SetupAction, SurfaceAction};
        match action {
            SurfaceAction::Setup(SetupAction::Choose { token, index }) => {
                self.choose_setup_project(token, index);
                None
            }
            SurfaceAction::Setup(SetupAction::Submit(token)) => self.submit_setup_draft(token),
            SurfaceAction::Setup(SetupAction::Cancel(token)) => {
                self.cancel_setup_draft(token);
                None
            }
            _ => None,
        }
    }

    fn forward(&mut self, _: &[u8]) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        panic!("modal controls cannot forward terminal input");
    }
}

#[test]
fn setup_choice_and_form_actions_resolve_the_painted_cell_on_matching_release() {
    let area = Rect::new(10, 5, 8, 1);
    for control in [
        ModalControl::Choice(17),
        ModalControl::Submit,
        ModalControl::Cancel,
    ] {
        for (x, y, admitted) in [
            (10, 5, true),
            (17, 5, true),
            (9, 5, false),
            (18, 5, false),
            (10, 4, false),
            (10, 6, false),
        ] {
            let mut pointer = PointerState::default();
            let mut port = SetupPort::default();
            let mut hits = HitMapBuilder::new();
            hits.start_modal();
            hits.record_modal_control(
                HitLayer::Overlay,
                area,
                HitTarget::Setup {
                    token: crate::workbench_client::SetupToken::for_test(7),
                    control,
                },
                InteractionAvailability::Enabled,
            );
            pointer.set_capture(true);
            pointer.publish(hits.finish());
            assert!(!pointer.modal_event(
                &mut port,
                event(MouseAction::Press, MouseButton::Left, x, y)
            ));
            assert!(port.decisions.is_empty());
            // An unrelated button release cannot commit or steal the left press.
            assert!(!pointer.modal_event(
                &mut port,
                event(MouseAction::Release, MouseButton::Right, x, y)
            ));
            assert!(port.decisions.is_empty());
            assert_eq!(
                pointer.modal_event(
                    &mut port,
                    event(MouseAction::Release, MouseButton::Left, x, y)
                ),
                admitted
            );
            assert_eq!(
                port.decisions,
                if admitted { vec![control] } else { vec![] }
            );
            assert!(!pointer.modal_event(
                &mut port,
                event(MouseAction::Release, MouseButton::Left, x, y)
            ));
        }
    }
}

#[derive(Default)]
struct ClosePort {
    decisions: Vec<(WorkspaceCloseToken, bool)>,
}
impl ClosePort {
    fn click_workspace_close(&mut self, token: WorkspaceCloseToken, confirm: bool) {
        self.decisions.push((token, confirm));
    }
}

impl TerminalInputPort for ClosePort {
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{CloseDecision, SurfaceAction};
        match action {
            SurfaceAction::Close(CloseDecision::Workspace { token, confirm }) => {
                self.click_workspace_close(token, confirm);
                None
            }
            _ => None,
        }
    }

    fn forward(&mut self, _: &[u8]) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        panic!("close gestures cannot forward terminal input");
    }
}

fn map(token: WorkspaceCloseToken, confirm: bool, area: Rect) -> FrameHitMap {
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    hits.record_modal_control(
        HitLayer::Overlay,
        area,
        HitTarget::WorkspaceClose { token, confirm },
        InteractionAvailability::Enabled,
    );
    hits.finish()
}

fn backdrop_map(token: WorkspaceCloseToken) -> FrameHitMap {
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    hits.record(
        HitLayer::OverlaySurface,
        Rect::new(10, 5, 30, 10),
        HitTarget::OverlaySurface,
    );
    let target = HitTarget::WorkspaceClose {
        token,
        confirm: false,
    };
    hits.record_modal_dismissal(target.clone());
    hits.record_modal_control(
        HitLayer::Overlay,
        Rect::new(36, 5, 3, 1),
        target,
        InteractionAvailability::Enabled,
    );
    hits.finish()
}

#[test]
fn backdrop_cancel_retains_exact_owner_and_matching_release_without_click_through() {
    let token = WorkspaceCloseToken::for_test(31);
    for revoke in 0..5 {
        let mut pointer = PointerState::default();
        let mut port = ClosePort::default();
        pointer.set_capture(true);
        pointer.publish(backdrop_map(token));
        assert_eq!(
            pointer.confirmation_event(
                &mut port,
                event(MouseAction::Press, MouseButton::Left, 2, 2)
            ),
            Some(false)
        );
        assert!(port.decisions.is_empty());
        assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::Active);
        assert_eq!(
            projected_modal_pointer(
                &pointer,
                HitLayer::Overlay,
                Rect::new(36, 5, 3, 1),
                &HitTarget::WorkspaceClose {
                    token,
                    confirm: false
                },
                InteractionAvailability::Enabled
            ),
            ProjectedPointer::Rest,
            "outside presses never press the X visually"
        );
        match revoke {
            0 => pointer.publish(backdrop_map(token)),
            1 => pointer.publish(backdrop_map(WorkspaceCloseToken::for_test(32))),
            2 => pointer.set_host_focus(false),
            3 => pointer.invalidate_map(),
            4 => {
                pointer.confirmation_event(
                    &mut port,
                    event(MouseAction::Motion, MouseButton::Left, 15, 8),
                );
            }
            _ => unreachable!(),
        }
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Release, MouseButton::Left, 2, 2),
        );
        assert_eq!(
            port.decisions,
            if revoke == 0 {
                vec![(token, false)]
            } else {
                vec![]
            }
        );
        assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::None);
        assert_eq!(
            pointer.confirmation_event(
                &mut port,
                event(MouseAction::Release, MouseButton::Left, 2, 2)
            ),
            None
        );
    }
}

fn projected_map(
    projection: PointerProjection,
    layer: HitLayer,
    area: Rect,
    target: HitTarget,
    availability: InteractionAvailability,
) -> FrameHitMap {
    let mut hits = HitMapBuilder::with_pointer(projection);
    hits.start_modal();
    hits.record_modal_control(layer, area, target, availability);
    hits.finish()
}

fn projected_modal_pointer(
    pointer: &PointerState,
    layer: HitLayer,
    area: Rect,
    target: &HitTarget,
    availability: InteractionAvailability,
) -> ProjectedPointer {
    HitMapBuilder::with_pointer(pointer.projection()).projected_modal_pointer(
        layer,
        area,
        target,
        availability,
    )
}

#[test]
fn close_controls_require_matching_release_within_every_painted_edge() {
    let token = WorkspaceCloseToken::for_test(1);
    let area = Rect::new(10, 5, 8, 2);
    for confirm in [false, true] {
        for (x, y, admitted) in [
            (10, 5, true),
            (17, 5, true),
            (10, 6, true),
            (17, 6, true),
            (9, 5, false),
            (18, 5, false),
            (10, 4, false),
            (10, 7, false),
        ] {
            let mut pointer = PointerState::default();
            let mut port = ClosePort::default();
            pointer.set_capture(true);
            pointer.publish(map(token, confirm, area));
            let press = event(MouseAction::Press, MouseButton::Left, x, y);
            let release = event(MouseAction::Release, MouseButton::Left, x, y);
            assert_eq!(
                pointer.confirmation_event(&mut port, press),
                admitted.then_some(false)
            );
            assert_eq!(
                pointer.modal_release_ownership(),
                if admitted {
                    ReleaseOwnership::Active
                } else {
                    ReleaseOwnership::None
                },
            );
            assert!(port.decisions.is_empty());
            assert_eq!(
                pointer.confirmation_event(&mut port, release),
                admitted.then_some(true)
            );
            assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::None);
            assert_eq!(
                port.decisions,
                if admitted {
                    vec![(token, confirm)]
                } else {
                    vec![]
                }
            );
            assert_eq!(pointer.confirmation_event(&mut port, release), None);
        }
    }
}

#[test]
fn specialized_modal_projection_tracks_exact_hover_press_and_transport_reset() {
    let token = WorkspaceCloseToken::for_test(21);
    let area = Rect::new(10, 5, 8, 2);
    let target = HitTarget::WorkspaceClose {
        token,
        confirm: true,
    };
    let mut pointer = PointerState::default();
    let mut port = ClosePort::default();
    pointer.set_capture(true);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    pointer.publish(projected_map(
        pointer.projection(),
        HitLayer::Overlay,
        area,
        target.clone(),
        InteractionAvailability::Enabled,
    ));
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );

    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Motion, MouseButton::None, 12, 5),
        ),
        None,
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Hovered,
    );
    pointer.publish(projected_map(
        pointer.projection(),
        HitLayer::Overlay,
        area,
        target.clone(),
        InteractionAvailability::Enabled,
    ));
    assert!(!pointer.take_projection_redraw());

    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        Some(false),
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Pressed,
    );
    pointer.publish(projected_map(
        pointer.projection(),
        HitLayer::Overlay,
        area,
        target.clone(),
        InteractionAvailability::Enabled,
    ));
    assert!(!pointer.take_projection_redraw());
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Motion, MouseButton::Left, 17, 6),
        ),
        Some(false),
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Pressed,
    );

    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Motion, MouseButton::Left, area.right(), 6),
        ),
        Some(false),
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );
    pointer.confirmation_event(
        &mut port,
        event(MouseAction::Motion, MouseButton::Left, 12, 5),
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
        "returning inside cannot re-arm a cancelled press",
    );
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        Some(true),
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );
    assert!(port.decisions.is_empty());

    pointer.confirmation_event(
        &mut port,
        event(MouseAction::Press, MouseButton::Left, 12, 5),
    );
    pointer.set_host_focus(false);
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );
    pointer.set_host_focus(true);
    assert_eq!(
        pointer.cancelled_modal_event(event(MouseAction::Release, MouseButton::None, 12, 5,)),
        CancelledModalEvent::Ended,
    );

    pointer.confirmation_event(
        &mut port,
        event(MouseAction::Press, MouseButton::Left, 12, 5),
    );
    pointer.set_capture(false);
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );
    pointer.set_capture(true);
    assert_eq!(
        pointer.cancelled_modal_event(event(MouseAction::Release, MouseButton::Left, 12, 5,)),
        CancelledModalEvent::Ended,
    );
    assert!(port.decisions.is_empty());
}

#[test]
fn button_only_modal_press_projects_pressed_without_inventing_hover() {
    let area = Rect::new(10, 5, 8, 1);
    let target = HitTarget::WorkspaceClose {
        token: WorkspaceCloseToken::for_test(22),
        confirm: true,
    };
    let mut pointer = PointerState::default();
    let mut port = ClosePort::default();
    pointer.set_capture(true);
    pointer.publish(projected_map(
        pointer.projection(),
        HitLayer::Overlay,
        area,
        target.clone(),
        InteractionAvailability::Enabled,
    ));
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Motion, MouseButton::None, 12, 5),
        ),
        None,
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        Some(false),
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Pressed,
    );
    pointer.cancel_modal_press();
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );
    assert_eq!(
        pointer.cancelled_modal_event(event(MouseAction::Release, MouseButton::Left, 12, 5,)),
        CancelledModalEvent::Ended,
    );
    assert!(port.decisions.is_empty());
}

#[test]
fn specialized_modal_projection_rejects_changed_source_geometry_and_layer() {
    let area = Rect::new(10, 5, 8, 1);
    let original = HitTarget::WorkspaceClose {
        token: WorkspaceCloseToken::for_test(31),
        confirm: true,
    };
    let replacement = HitTarget::WorkspaceClose {
        token: WorkspaceCloseToken::for_test(32),
        confirm: true,
    };
    let mut pointer = PointerState::default();
    let mut port = ClosePort::default();
    pointer.set_capture(true);
    pointer.publish(projected_map(
        pointer.projection(),
        HitLayer::Overlay,
        area,
        original.clone(),
        InteractionAvailability::Enabled,
    ));
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        Some(false),
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::OverlaySurface,
            area,
            &original,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            Rect::new(11, 5, 8, 1),
            &original,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &replacement,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );

    pointer.publish(projected_map(
        pointer.projection(),
        HitLayer::Overlay,
        area,
        replacement,
        InteractionAvailability::Enabled,
    ));
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::Cleanup);
    assert!(pointer.take_projection_redraw());
    pointer.publish(projected_map(
        pointer.projection(),
        HitLayer::Overlay,
        area,
        original.clone(),
        InteractionAvailability::Enabled,
    ));
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &original,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
        "restoring the old paint cannot revive its cancelled press",
    );
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        Some(true),
    );
    assert!(port.decisions.is_empty());
}

#[test]
fn disabled_specialized_modal_target_never_hovers_or_arms() {
    let token = WorkspaceCloseToken::for_test(41);
    let area = Rect::new(10, 5, 8, 1);
    let target = HitTarget::WorkspaceClose {
        token,
        confirm: true,
    };
    let mut pointer = PointerState::default();
    let mut port = ClosePort::default();
    pointer.set_capture(true);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    pointer.publish(projected_map(
        pointer.projection(),
        HitLayer::Overlay,
        area,
        target.clone(),
        InteractionAvailability::Disabled,
    ));
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Motion, MouseButton::None, 12, 5),
        ),
        None,
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Disabled,
        ),
        ProjectedPointer::Rest,
    );
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        None,
    );
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::None);
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        None,
    );
    assert!(port.decisions.is_empty());
}

#[test]
fn raw_modal_hit_without_explicit_interaction_metadata_never_hovers_or_arms() {
    let area = Rect::new(10, 5, 8, 1);
    let target = HitTarget::WorkspaceClose {
        token: WorkspaceCloseToken::for_test(45),
        confirm: true,
    };
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    hits.record(HitLayer::Overlay, area, target.clone());

    let mut pointer = PointerState::default();
    let mut port = ClosePort::default();
    pointer.set_capture(true);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    pointer.publish(hits.finish());
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Motion, MouseButton::None, 12, 5),
        ),
        None,
    );
    assert_eq!(
        projected_modal_pointer(
            &pointer,
            HitLayer::Overlay,
            area,
            &target,
            InteractionAvailability::Enabled,
        ),
        ProjectedPointer::Rest,
    );
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        None,
    );
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::None);
    assert_eq!(
        pointer.confirmation_event(
            &mut port,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        None,
    );
    assert!(port.decisions.is_empty());
}

#[test]
fn generic_and_specialized_modal_presses_never_steal_each_others_owner() {
    let generic_area = Rect::new(2, 2, 4, 1);
    let modal_area = Rect::new(10, 5, 8, 1);
    let generic_hit = HitTarget::Help {
        page: 1,
        action: HelpControl::Close,
    };
    let generic = InteractionTarget::Fixture(generic_hit.clone());
    let modal = HitTarget::WorkspaceClose {
        token: WorkspaceCloseToken::for_test(51),
        confirm: true,
    };
    let paint = |projection| {
        let mut hits = HitMapBuilder::with_pointer(projection);
        hits.start_modal();
        hits.record_interaction(
            HitLayer::Overlay,
            generic_area,
            generic_area,
            generic.clone(),
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
        );
        hits.record_modal_control(
            HitLayer::Overlay,
            modal_area,
            modal.clone(),
            InteractionAvailability::Enabled,
        );
        let map = hits.finish();
        assert_eq!(map.hit_target_at(3, 2), Some(&generic_hit));
        assert_eq!(map.hit_target_at(12, 5), Some(&modal));
        map
    };

    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(paint(pointer.projection()));
    assert_eq!(
        pointer.event(
            &mut ClientView::new(80),
            &mut RecordingPort::default(),
            std::time::Instant::now(),
            event(MouseAction::Press, MouseButton::Left, 3, 2),
        ),
        PointerOutcome::Handled,
    );
    let mut close_port = ClosePort::default();
    assert_eq!(
        pointer.confirmation_event(
            &mut close_port,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        None,
    );
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::Active);
    assert!(close_port.decisions.is_empty());

    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(paint(pointer.projection()));
    assert_eq!(
        pointer.confirmation_event(
            &mut close_port,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        Some(false),
    );
    assert_eq!(
        pointer.event(
            &mut ClientView::new(80),
            &mut RecordingPort::default(),
            std::time::Instant::now(),
            event(MouseAction::Press, MouseButton::Left, 3, 2),
        ),
        PointerOutcome::Ignored,
    );
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::Active);
    assert!(close_port.decisions.is_empty());
}

#[test]
fn close_press_never_survives_changed_token_geometry_modifiers_focus_or_capture() {
    let token = WorkspaceCloseToken::for_test(1);
    let area = Rect::new(10, 5, 8, 1);
    for change in 0..9 {
        let mut pointer = PointerState::default();
        let mut port = ClosePort::default();
        pointer.set_capture(true);
        pointer.publish(map(token, true, area));
        assert_eq!(
            pointer.confirmation_event(
                &mut port,
                event(MouseAction::Press, MouseButton::Left, 12, 5)
            ),
            Some(false)
        );
        let mut release = event(MouseAction::Release, MouseButton::Left, 12, 5);
        match change {
            0 => pointer.publish(map(WorkspaceCloseToken::for_test(2), true, area)),
            1 => pointer.publish(map(token, false, area)),
            2 => {
                pointer.publish(map(token, true, Rect::new(11, 5, 8, 1)));
                pointer.publish(map(token, true, area));
            }
            3 => {
                pointer.invalidate_map();
                pointer.publish(map(token, true, area));
            }
            4 => {
                pointer.set_capture(false);
                pointer.set_capture(true);
            }
            5 => release.column = area.right(),
            6 => release.mods = Modifiers::CTRL,
            7 => pointer.cancel_modal_press(),
            8 => {
                pointer.publish(FrameHitMap::default());
                pointer.publish(map(token, true, area));
            }
            _ => unreachable!(),
        }
        assert_eq!(
            pointer.confirmation_event(&mut port, release),
            Some(true),
            "change {change}"
        );
        assert!(port.decisions.is_empty(), "change {change}");
    }
}

#[test]
fn leaving_a_painted_control_cancels_even_after_return_and_repaint() {
    let token = WorkspaceCloseToken::for_test(1);
    let area = Rect::new(10, 5, 8, 2);
    for button in [MouseButton::Left, MouseButton::None] {
        for (outside_x, outside_y) in [(9, 5), (18, 5), (10, 4), (10, 7)] {
            let mut pointer = PointerState::default();
            let mut port = ClosePort::default();
            pointer.set_capture(true);
            pointer.publish(map(token, true, area));
            assert_eq!(
                pointer.confirmation_event(
                    &mut port,
                    event(MouseAction::Press, MouseButton::Left, 12, 5)
                ),
                Some(false)
            );
            pointer.confirmation_event(
                &mut port,
                event(MouseAction::Motion, button, outside_x, outside_y),
            );
            // A repaint and return expose the same control, but the intent
            // was already cancelled by the ordered boundary crossing.
            pointer.publish(map(token, true, area));
            pointer.confirmation_event(&mut port, event(MouseAction::Motion, button, 12, 5));
            assert_eq!(
                pointer.confirmation_event(
                    &mut port,
                    event(MouseAction::Release, MouseButton::Left, 12, 5)
                ),
                Some(true)
            );
            assert!(port.decisions.is_empty());

            // A new gesture can still activate the exact control once.
            pointer.confirmation_event(
                &mut port,
                event(MouseAction::Press, MouseButton::Left, 12, 5),
            );
            pointer.confirmation_event(
                &mut port,
                event(MouseAction::Release, MouseButton::Left, 12, 5),
            );
            assert_eq!(port.decisions, vec![(token, true)]);
        }
    }
}

#[test]
fn motion_inside_the_same_control_preserves_intent_across_visual_repaint() {
    let token = WorkspaceCloseToken::for_test(1);
    let area = Rect::new(10, 5, 8, 2);
    let mut pointer = PointerState::default();
    let mut port = ClosePort::default();
    pointer.set_capture(true);
    pointer.publish(map(token, true, area));
    pointer.confirmation_event(
        &mut port,
        event(MouseAction::Press, MouseButton::Left, 10, 5),
    );
    pointer.publish(map(token, true, area));
    pointer.confirmation_event(
        &mut port,
        event(MouseAction::Motion, MouseButton::Left, 17, 6),
    );
    pointer.confirmation_event(
        &mut port,
        event(MouseAction::Release, MouseButton::Left, 17, 6),
    );
    assert_eq!(port.decisions, vec![(token, true)]);
}

#[test]
fn cancelled_painted_press_cleanup_ends_once_and_a_fresh_press_retires_old_debt() {
    let token = WorkspaceCloseToken::for_test(1);
    let area = Rect::new(10, 5, 8, 1);
    let press = event(MouseAction::Press, MouseButton::Left, 12, 5);
    let release = event(MouseAction::Release, MouseButton::Left, 12, 5);
    let mut pointer = PointerState::default();
    let mut port = ClosePort::default();
    pointer.set_capture(true);
    pointer.publish(map(token, true, area));

    assert_eq!(pointer.confirmation_event(&mut port, press), Some(false));
    assert_eq!(
        pointer.cancelled_modal_event(release),
        CancelledModalEvent::Ignored
    );
    assert_eq!(pointer.confirmation_event(&mut port, release), Some(true));
    assert_eq!(port.decisions, vec![(token, true)]);

    port.decisions.clear();
    assert_eq!(pointer.confirmation_event(&mut port, press), Some(false));
    pointer.cancel_modal_press();
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::Cleanup);
    assert_eq!(
        pointer.cancelled_modal_event(event(MouseAction::Motion, MouseButton::Left, 13, 5)),
        CancelledModalEvent::Captured
    );
    assert_eq!(
        pointer.cancelled_modal_event(release),
        CancelledModalEvent::Ended
    );
    assert_eq!(
        pointer.cancelled_modal_event(release),
        CancelledModalEvent::Ignored
    );
    assert!(port.decisions.is_empty());

    assert_eq!(pointer.confirmation_event(&mut port, press), Some(false));
    pointer.cancel_modal_press();
    assert_eq!(
        pointer.cancelled_modal_event(press),
        CancelledModalEvent::Retired
    );
    // The same new press continues into the current owner after retiring the
    // old cleanup record.
    assert_eq!(pointer.confirmation_event(&mut port, press), Some(false));
    assert_eq!(pointer.confirmation_event(&mut port, release), Some(true));
    assert_eq!(port.decisions, vec![(token, true)]);

    let mut fresh = PointerState::default();
    assert_eq!(
        fresh.cancelled_modal_event(release),
        CancelledModalEvent::Ignored
    );
}

#[test]
fn cancelled_form_control_cleanup_never_replays_its_semantic_target() {
    let area = Rect::new(10, 5, 8, 1);
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    hits.record_modal_control(
        HitLayer::Overlay,
        area,
        HitTarget::Setup {
            token: crate::workbench_client::SetupToken::for_test(7),
            control: ModalControl::Submit,
        },
        InteractionAvailability::Enabled,
    );
    let mut pointer = PointerState::default();
    let mut port = SetupPort::default();
    pointer.set_capture(true);
    pointer.publish(hits.finish());
    assert!(!pointer.modal_event(
        &mut port,
        event(MouseAction::Press, MouseButton::Left, 12, 5)
    ));
    pointer.cancel_modal_press();
    assert_eq!(
        pointer.cancelled_modal_event(event(MouseAction::Release, MouseButton::None, 12, 5)),
        CancelledModalEvent::Ended
    );
    assert!(port.decisions.is_empty());
}
