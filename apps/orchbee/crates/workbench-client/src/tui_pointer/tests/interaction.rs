use super::*;

const AREA: Rect = Rect::new(10, 5, 8, 2);

fn close_hit() -> HitTarget {
    HitTarget::Help {
        page: 1,
        action: HelpControl::Close,
    }
}

fn close() -> InteractionTarget {
    InteractionTarget::Fixture(close_hit())
}

fn previous_hit() -> HitTarget {
    HitTarget::Help {
        page: 1,
        action: HelpControl::Previous,
    }
}

fn previous() -> InteractionTarget {
    InteractionTarget::Fixture(previous_hit())
}

fn interaction_map(
    projection: PointerProjection,
    target: InteractionTarget,
    hit_rect: Rect,
    paint_rect: Rect,
    spec: InteractionSpec,
) -> FrameHitMap {
    interaction_map_on(
        projection,
        HitLayer::Foreground,
        target,
        hit_rect,
        paint_rect,
        spec,
    )
}

fn interaction_map_on(
    projection: PointerProjection,
    layer: HitLayer,
    target: InteractionTarget,
    hit_rect: Rect,
    paint_rect: Rect,
    spec: InteractionSpec,
) -> FrameHitMap {
    let mut hits = HitMapBuilder::with_pointer(projection);
    hits.record_interaction(layer, hit_rect, paint_rect, target, spec);
    hits.finish()
}

fn pointer_with(map: FrameHitMap) -> PointerState {
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.publish(map);
    assert!(!pointer.take_projection_redraw());
    pointer
}

fn drive(pointer: &mut PointerState, input: MouseEvent) -> PointerOutcome {
    pointer.event(
        &mut ClientView::new(80),
        &mut RecordingPort::default(),
        Instant::now(),
        input,
    )
}

#[test]
fn settings_backdrop_uses_only_its_painted_close_and_revokes_on_owner_or_geometry_change() {
    let mut editor = crate::native_settings::Editor::default();
    let token = editor.open().unwrap();
    let target = InteractionTarget::SettingsControl {
        token,
        control: crate::native_settings::Control::Close,
    };
    let map = |registered: bool| {
        let mut hits = HitMapBuilder::new();
        hits.start_settings();
        hits.record(
            HitLayer::OverlaySurface,
            Rect::new(10, 5, 30, 10),
            HitTarget::OverlaySurface,
        );
        if registered {
            hits.record_overlay_dismissal(&target);
        }
        hits.record_interaction(
            HitLayer::Overlay,
            AREA,
            AREA,
            target.clone(),
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
        );
        hits.finish()
    };
    for revoke in 0..5 {
        let mut pointer = pointer_with(map(true));
        assert_eq!(
            pointer.settings_event(event(MouseAction::Press, MouseButton::Left, 2, 2)),
            PointerOutcome::Handled
        );
        assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::Active);
        assert_eq!(
            HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
                HitLayer::Overlay,
                AREA,
                AREA,
                &target,
                InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
            ),
            ProjectedPointer::Rest
        );
        match revoke {
            0 => pointer.publish(map(true)),
            1 => pointer.publish(map(false)),
            2 => pointer.set_host_focus(false),
            3 => pointer.invalidate_map(),
            4 => {
                pointer.settings_event(event(MouseAction::Motion, MouseButton::Left, 20, 8));
            }
            _ => unreachable!(),
        }
        let result = pointer.settings_event(event(MouseAction::Release, MouseButton::Left, 2, 2));
        assert_eq!(
            result,
            if revoke == 0 {
                PointerOutcome::Interaction {
                    target: target.clone(),
                    contract: PrimaryContract::ActivateOnRelease,
                }
            } else {
                PointerOutcome::InteractionEnded
            }
        );
        assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::None);
    }
}

#[test]
fn settings_targets_activate_only_through_the_settings_owner() {
    let mut editor = crate::native_settings::Editor::default();
    let token = editor.open().expect("the first Settings open has a token");
    let target = InteractionTarget::SettingsControl {
        token,
        control: crate::native_settings::Control::Close,
    };
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);

    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Ignored
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Ignored
    );

    assert_eq!(
        pointer.settings_event(event(MouseAction::Press, MouseButton::Left, 12, 5)),
        PointerOutcome::Handled
    );
    assert_eq!(
        pointer.settings_event(event(MouseAction::Release, MouseButton::Left, 12, 5)),
        PointerOutcome::Interaction {
            target,
            contract: PrimaryContract::ActivateOnRelease,
        }
    );
}

#[test]
fn a_generic_target_cannot_take_over_an_existing_specialized_gesture() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);
    pointer.gesture = Gesture::Pane(PaneGesture {
        button: MouseButton::Left,
        expected: pane_ref(1, 1, 1, 0),
        viewport: AREA,
        pressed: true,
        last_cell: None,
    });

    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Ignored,
        "the press belongs to the gesture that was active at admission time",
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Rest,
    );

    // The dead pane gesture retired while processing the first report. A
    // later physical press may now establish a fresh generic owner.
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Handled,
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Interaction {
            target,
            contract: PrimaryContract::ActivateOnRelease,
        },
    );
}

#[test]
fn any_motion_projects_hover_without_changing_keyboard_or_domain_state() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);
    pointer.set_motion_observability(MotionObservability::AnyMotion);

    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Motion, MouseButton::None, 12, 5),
        ),
        PointerOutcome::IdleMotion { changed: true }
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Hovered
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Motion, MouseButton::None, 17, 6),
        ),
        PointerOutcome::IdleMotion { changed: false }
    );
    let mut shifted = event(MouseAction::Motion, MouseButton::None, 18, 6);
    shifted.mods = Modifiers::NONE.with_shift();
    assert_eq!(
        drive(&mut pointer, shifted),
        PointerOutcome::IdleMotion { changed: false }
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Hovered
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Motion, MouseButton::None, 18, 6),
        ),
        PointerOutcome::IdleMotion { changed: true }
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Rest
    );
}

#[test]
fn release_activation_survives_visual_repaint_of_the_same_fingerprint() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);

    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Handled
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Pressed
    );
    pointer.publish(interaction_map(
        pointer.projection(),
        target.clone(),
        AREA,
        AREA,
        spec,
    ));
    assert!(!pointer.take_projection_redraw());
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 13, 6),
        ),
        PointerOutcome::Interaction {
            target: target.clone(),
            contract: PrimaryContract::ActivateOnRelease,
        }
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Rest
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 13, 6),
        ),
        PointerOutcome::Ignored
    );
}

#[test]
fn release_returns_to_observed_hover_without_synthesizing_position_after_loss() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    drive(
        &mut pointer,
        event(MouseAction::Motion, MouseButton::None, 12, 5),
    );
    drive(
        &mut pointer,
        event(MouseAction::Press, MouseButton::Left, 12, 5),
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Interaction {
            target: target.clone(),
            contract: PrimaryContract::ActivateOnRelease,
        }
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Hovered
    );

    pointer.set_host_focus(false);
    pointer.set_host_focus(true);
    drive(
        &mut pointer,
        event(MouseAction::Press, MouseButton::Left, 12, 5),
    );
    drive(
        &mut pointer,
        event(MouseAction::Release, MouseButton::Left, 12, 5),
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Rest
    );
}

#[test]
fn leaving_never_rearms_until_a_new_press() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);
    drive(
        &mut pointer,
        event(MouseAction::Press, MouseButton::Left, 12, 5),
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Motion, MouseButton::Left, 18, 5),
        ),
        PointerOutcome::Handled
    );
    pointer.publish(interaction_map(
        pointer.projection(),
        target.clone(),
        AREA,
        AREA,
        spec,
    ));
    drive(
        &mut pointer,
        event(MouseAction::Motion, MouseButton::Left, 12, 5),
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::InteractionEnded
    );

    drive(
        &mut pointer,
        event(MouseAction::Press, MouseButton::Left, 12, 5),
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Interaction {
            target,
            contract: PrimaryContract::ActivateOnRelease,
        }
    );
}

#[test]
fn changed_geometry_availability_contract_or_occlusion_cancels_without_retargeting() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    for change in 0..4 {
        let map = interaction_map(
            PointerProjection::default(),
            target.clone(),
            AREA,
            AREA,
            spec,
        );
        let mut pointer = pointer_with(map);
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        );
        let replacement = match change {
            0 => interaction_map(
                pointer.projection(),
                target.clone(),
                Rect::new(11, 5, 8, 2),
                Rect::new(11, 5, 8, 2),
                spec,
            ),
            1 => interaction_map(
                pointer.projection(),
                target.clone(),
                AREA,
                AREA,
                InteractionSpec::disabled(PrimaryContract::ActivateOnRelease),
            ),
            2 => interaction_map(
                pointer.projection(),
                target.clone(),
                AREA,
                AREA,
                InteractionSpec::enabled(PrimaryContract::Specialized),
            ),
            3 => {
                let mut hits = HitMapBuilder::with_pointer(pointer.projection());
                hits.record(HitLayer::Foreground, AREA, previous_hit());
                hits.record_interaction(HitLayer::Foreground, AREA, AREA, target.clone(), spec);
                hits.finish()
            }
            _ => unreachable!(),
        };
        pointer.publish(replacement);
        pointer.publish(interaction_map(
            pointer.projection(),
            target.clone(),
            AREA,
            AREA,
            spec,
        ));
        assert_eq!(
            drive(
                &mut pointer,
                event(MouseAction::Release, MouseButton::Left, 12, 5),
            ),
            PointerOutcome::InteractionEnded,
            "change {change}"
        );
    }
}

#[test]
fn hit_extent_layer_and_effective_child_exposure_are_part_of_the_arm() {
    let row = previous();
    let child = close();
    let row_rect = Rect::new(10, 5, 12, 1);
    let child_rect = Rect::new(19, 5, 3, 1);
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);

    for replacement in 0..4 {
        let mut pointer = pointer_with(interaction_map(
            PointerProjection::default(),
            row.clone(),
            row_rect,
            row_rect,
            spec,
        ));
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 11, 5),
        );

        let map = match replacement {
            // The press cell remains inside, but the target's admitted hit extent changed.
            0 => interaction_map(
                pointer.projection(),
                row.clone(),
                Rect::new(10, 5, 10, 1),
                row_rect,
                spec,
            ),
            1 => interaction_map_on(
                pointer.projection(),
                HitLayer::Status,
                row.clone(),
                row_rect,
                row_rect,
                spec,
            ),
            // A new frontmost child changes the parent's exposed shape away from the press cell.
            2 => {
                let mut hits = HitMapBuilder::with_pointer(pointer.projection());
                hits.record_interaction(
                    HitLayer::Foreground,
                    child_rect,
                    child_rect,
                    child.clone(),
                    spec,
                );
                hits.record_interaction(
                    HitLayer::Foreground,
                    row_rect,
                    row_rect,
                    row.clone(),
                    spec,
                );
                hits.finish()
            }
            // The same covered cells now belong to a different semantic child.
            3 => {
                let initial_child = InteractionTarget::Fixture(HitTarget::Help {
                    page: 2,
                    action: HelpControl::Close,
                });
                let mut initial = HitMapBuilder::with_pointer(PointerProjection::default());
                initial.record_interaction(
                    HitLayer::Foreground,
                    child_rect,
                    child_rect,
                    initial_child,
                    spec,
                );
                initial.record_interaction(
                    HitLayer::Foreground,
                    row_rect,
                    row_rect,
                    row.clone(),
                    spec,
                );
                let mut pointer_with_child = pointer_with(initial.finish());
                drive(
                    &mut pointer_with_child,
                    event(MouseAction::Press, MouseButton::Left, 11, 5),
                );
                let mut changed = HitMapBuilder::with_pointer(pointer_with_child.projection());
                changed.record_interaction(
                    HitLayer::Foreground,
                    child_rect,
                    child_rect,
                    child.clone(),
                    spec,
                );
                changed.record_interaction(
                    HitLayer::Foreground,
                    row_rect,
                    row_rect,
                    row.clone(),
                    spec,
                );
                pointer_with_child.publish(changed.finish());
                assert_eq!(
                    drive(
                        &mut pointer_with_child,
                        event(MouseAction::Release, MouseButton::Left, 11, 5),
                    ),
                    PointerOutcome::InteractionEnded,
                );
                continue;
            }
            _ => unreachable!(),
        };
        pointer.publish(map);
        assert_eq!(
            drive(
                &mut pointer,
                event(MouseAction::Release, MouseButton::Left, 11, 5),
            ),
            PointerOutcome::InteractionEnded,
            "replacement {replacement}",
        );
    }

    // A target outside the row's painted extent is unrelated and preserves the arm.
    let mut pointer = pointer_with(interaction_map(
        PointerProjection::default(),
        row.clone(),
        row_rect,
        row_rect,
        spec,
    ));
    drive(
        &mut pointer,
        event(MouseAction::Press, MouseButton::Left, 11, 5),
    );
    let mut unrelated = HitMapBuilder::with_pointer(pointer.projection());
    unrelated.record_interaction(
        HitLayer::Foreground,
        Rect::new(30, 5, 3, 1),
        Rect::new(30, 5, 3, 1),
        child,
        spec,
    );
    unrelated.record_interaction(HitLayer::Foreground, row_rect, row_rect, row.clone(), spec);
    pointer.publish(unrelated.finish());
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 11, 5),
        ),
        PointerOutcome::Interaction {
            target: row,
            contract: PrimaryContract::ActivateOnRelease,
        },
    );
}

/// A press read together with the motion onto a row precedes the frame that reveals the row's
/// pointer-only child. That frame projects the press onto the exposure the press recorded before
/// painting the child, so the child is the press's own reveal and the click survives it.
#[test]
fn a_child_revealed_for_the_pressed_row_keeps_the_arm() {
    let row = previous();
    let child = close();
    let row_rect = Rect::new(10, 5, 12, 1);
    let child_rect = Rect::new(19, 5, 3, 1);
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let mut pointer = pointer_with(interaction_map(
        PointerProjection::default(),
        row.clone(),
        row_rect,
        row_rect,
        spec,
    ));
    drive(
        &mut pointer,
        event(MouseAction::Press, MouseButton::Left, 11, 5),
    );

    let mut revealed = HitMapBuilder::with_pointer(pointer.projection());
    assert_eq!(
        revealed.projected_pointer_beneath(
            HitLayer::Foreground,
            &[],
            row_rect,
            row_rect,
            &row,
            spec
        ),
        ProjectedPointer::Pressed
    );
    revealed.record_interaction(HitLayer::Foreground, child_rect, child_rect, child, spec);
    revealed.record_interaction(HitLayer::Foreground, row_rect, row_rect, row.clone(), spec);
    pointer.publish(revealed.finish());
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 11, 5),
        ),
        PointerOutcome::Interaction {
            target: row,
            contract: PrimaryContract::ActivateOnRelease,
        },
    );
}

#[test]
fn disabled_and_specialized_targets_preserve_discoverability_without_generic_activation() {
    let target = close();
    let disabled = InteractionSpec::disabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        disabled,
    );
    let mut pointer = pointer_with(map);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Motion, MouseButton::None, 12, 5),
        ),
        PointerOutcome::IdleMotion { changed: false }
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            disabled,
        ),
        ProjectedPointer::Rest
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Handled
    );
    assert_eq!(
        pointer.modal_release_ownership(),
        ReleaseOwnership::Cleanup,
        "a disabled target owns only the accepted press's release cleanup",
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::InteractionEnded
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Ignored
    );

    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.set_motion_observability(MotionObservability::ButtonOnly);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    let specialized = InteractionSpec::enabled(PrimaryContract::Specialized);
    pointer.publish(interaction_map(
        pointer.projection(),
        target.clone(),
        AREA,
        AREA,
        specialized,
    ));
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Motion, MouseButton::None, 12, 5),
        ),
        PointerOutcome::IdleMotion { changed: true }
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            specialized,
        ),
        ProjectedPointer::Hovered
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Ignored
    );
}

#[test]
fn select_on_press_emits_once_and_retains_release_cleanup() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::SelectOnPress);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::None);
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Interaction {
            target,
            contract: PrimaryContract::SelectOnPress,
        }
    );
    assert_eq!(
        pointer.modal_release_ownership(),
        ReleaseOwnership::Active,
        "provisional press ownership still ends on the matching release",
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::InteractionEnded
    );
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::None);
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Ignored
    );
}

#[test]
fn generic_release_ownership_distinguishes_active_intent_from_cleanup_debt() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(PointerProjection::default(), target, AREA, AREA, spec);
    let mut pointer = pointer_with(map);
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::None);

    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Handled,
    );
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::Active,);

    pointer.invalidate_map();
    assert_eq!(
        pointer.modal_release_ownership(),
        ReleaseOwnership::Cleanup,
        "structural invalidation revokes the effect but retains one release cleanup",
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::None, 12, 5),
        ),
        PointerOutcome::InteractionEnded,
    );
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::None,);
}

#[test]
fn compositor_cover_retirement_preserves_generic_interaction_ownership() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Handled,
    );

    assert!(!pointer.retire_covered_chrome_gesture());
    assert_eq!(pointer.modal_release_ownership(), ReleaseOwnership::Active,);
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Interaction {
            target,
            contract: PrimaryContract::ActivateOnRelease,
        },
    );
}

#[test]
fn wheel_cancels_an_active_interaction_without_retiring_its_release_debt() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);

    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Handled,
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Press, MouseButton::WheelDown, 12, 5),
        ),
        PointerOutcome::Handled,
        "wheel input belongs to the accepted primary gesture until cleanup",
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Rest,
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::InteractionEnded,
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Ignored,
        "the accepted release settles cleanup exactly once",
    );
}

#[test]
fn modal_list_scroll_waits_for_generic_release_cleanup() {
    let mut editor = crate::native_settings::Editor::default();
    let token = editor.open().expect("the first Settings open has a token");
    let target = InteractionTarget::SettingsControl {
        token,
        control: crate::native_settings::Control::Close,
    };
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let mut hits = HitMapBuilder::with_pointer(PointerProjection::default());
    hits.record_interaction(HitLayer::Foreground, AREA, AREA, target, spec);
    hits.record(
        HitLayer::Foreground,
        AREA,
        HitTarget::ListScroll(ListScrollTarget::Settings(token)),
    );
    let mut pointer = pointer_with(hits.finish());

    assert_eq!(
        pointer.settings_event(event(MouseAction::Press, MouseButton::Left, 12, 5)),
        PointerOutcome::Handled,
    );
    assert_eq!(
        pointer.settings_event(event(MouseAction::Press, MouseButton::WheelDown, 12, 5)),
        PointerOutcome::Handled,
    );
    assert!(
        pointer.list_scroll.is_none(),
        "an accepted generic press owns intervening wheel reports",
    );
    assert_eq!(
        pointer.settings_event(event(MouseAction::Release, MouseButton::Left, 12, 5)),
        PointerOutcome::InteractionEnded,
    );

    assert_eq!(
        pointer.settings_event(event(MouseAction::Press, MouseButton::WheelDown, 12, 5)),
        PointerOutcome::Handled,
    );
    assert!(
        pointer.list_scroll.is_some(),
        "an idle generic owner still admits its painted list wheel",
    );
}

#[test]
fn focus_capture_and_motion_downgrade_clear_without_synthetic_restoration() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    drive(
        &mut pointer,
        event(MouseAction::Motion, MouseButton::None, 12, 5),
    );
    pointer.set_host_focus(false);
    pointer.set_host_focus(true);
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Rest
    );
    pointer.set_motion_observability(MotionObservability::ButtonOnly);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            AREA,
            AREA,
            &target,
            spec,
        ),
        ProjectedPointer::Rest
    );

    drive(
        &mut pointer,
        event(MouseAction::Press, MouseButton::Left, 12, 5),
    );
    pointer.set_capture(false);
    pointer.set_capture(true);
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::InteractionEnded
    );
}

#[test]
fn stationary_resolution_requests_exactly_one_followup_projection() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let mut pointer = pointer_with(FrameHitMap::default());
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Motion, MouseButton::None, 12, 5),
        ),
        PointerOutcome::IdleMotion { changed: false }
    );

    let mut first = HitMapBuilder::with_pointer(pointer.projection());
    assert_eq!(
        first.projected_pointer(HitLayer::Foreground, AREA, AREA, &target, spec),
        ProjectedPointer::Rest
    );
    first.record_interaction(HitLayer::Foreground, AREA, AREA, target.clone(), spec);
    pointer.publish(first.finish());
    assert!(pointer.take_projection_redraw());
    assert!(!pointer.take_projection_redraw());

    let mut followup = HitMapBuilder::with_pointer(pointer.projection());
    assert_eq!(
        followup.projected_pointer(HitLayer::Foreground, AREA, AREA, &target, spec),
        ProjectedPointer::Hovered
    );
    followup.record_interaction(HitLayer::Foreground, AREA, AREA, target, spec);
    pointer.publish(followup.finish());
    assert!(!pointer.take_projection_redraw());
}

#[test]
fn cancelled_arm_reconciles_stationary_hover_and_still_consumes_its_late_release_once() {
    let target = close();
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let map = interaction_map(
        PointerProjection::default(),
        target.clone(),
        AREA,
        AREA,
        spec,
    );
    let mut pointer = pointer_with(map);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    drive(
        &mut pointer,
        event(MouseAction::Motion, MouseButton::None, 12, 5),
    );
    drive(
        &mut pointer,
        event(MouseAction::Press, MouseButton::Left, 12, 5),
    );
    pointer.invalidate_map();

    let mut candidate = HitMapBuilder::with_pointer(pointer.projection());
    assert_eq!(
        candidate.projected_pointer(HitLayer::Foreground, AREA, AREA, &target, spec),
        ProjectedPointer::Rest,
    );
    candidate.record_interaction(HitLayer::Foreground, AREA, AREA, target.clone(), spec);
    pointer.publish(candidate.finish());
    assert!(pointer.take_projection_redraw());
    assert!(!pointer.take_projection_redraw());

    let mut followup = HitMapBuilder::with_pointer(pointer.projection());
    assert_eq!(
        followup.projected_pointer(HitLayer::Foreground, AREA, AREA, &target, spec),
        ProjectedPointer::Hovered,
    );
    followup.record_interaction(HitLayer::Foreground, AREA, AREA, target, spec);
    pointer.publish(followup.finish());
    assert!(!pointer.take_projection_redraw());
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Motion, MouseButton::Left, 11, 5),
        ),
        PointerOutcome::Handled,
        "cleanup debt cannot re-hit the freshly painted target",
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::InteractionEnded,
    );
    assert_eq!(
        drive(
            &mut pointer,
            event(MouseAction::Release, MouseButton::Left, 12, 5),
        ),
        PointerOutcome::Ignored,
    );
}

#[test]
fn frontmost_child_target_wins_inside_a_full_row_target() {
    let child = close();
    let row = previous();
    let child_rect = Rect::new(16, 5, 2, 1);
    let row_rect = Rect::new(10, 5, 8, 1);
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let mut hits = HitMapBuilder::with_pointer(PointerProjection::default());
    hits.record_interaction(
        HitLayer::Foreground,
        child_rect,
        child_rect,
        child.clone(),
        spec,
    );
    hits.record_interaction(HitLayer::Foreground, row_rect, row_rect, row.clone(), spec);
    let mut pointer = pointer_with(hits.finish());
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    drive(
        &mut pointer,
        event(MouseAction::Motion, MouseButton::None, 16, 5),
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            child_rect,
            child_rect,
            &child,
            spec,
        ),
        ProjectedPointer::Hovered
    );
    assert_eq!(
        HitMapBuilder::with_pointer(pointer.projection()).projected_pointer(
            HitLayer::Foreground,
            row_rect,
            row_rect,
            &row,
            spec,
        ),
        ProjectedPointer::Rest
    );
}
