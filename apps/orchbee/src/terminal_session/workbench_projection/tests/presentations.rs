use super::*;

fn children(value: &mut Snapshot) -> &mut Vec<Presentation> {
    let PanelNode::Panel {
        child_presentations,
        ..
    } = &mut value.terminal_contexts[0].tabs[0].root
    else {
        unreachable!()
    };
    child_presentations
}
fn child(seed: u128) -> Presentation {
    Presentation {
        presentation_id: presentation_id(seed),
        label: format!("child {seed}"),
        custom_label: None,
        binding: Some(PresentationBinding {
            worker_id: worker_id(seed + 1),
            terminal_id: terminal_id(seed + 2),
        }),
    }
}

#[test]
fn children_preserve_one_layout_leaf_and_complete_ordered_payload() {
    let mut source = snapshot(2, 1);
    children(&mut source).extend([child(9000), child(9010)]);
    let projection = project_native_standalone_tabs(&source).unwrap();
    let tab = &projection.tabs()[0];
    assert_eq!(tab.panel_count(), 1);
    assert_eq!(tab.leaf_panel_ids(), vec![tab.panel_id().clone()]);
    let panel = tab.panel(0).unwrap();
    assert_eq!(panel.presentation_count(), 3);
    assert_eq!(
        panel.presentation(1).unwrap().presentation_id(),
        &presentation_id(9000)
    );
    assert_eq!(
        panel.presentation(2).unwrap().binding().terminal_id(),
        &terminal_id(9012)
    );
    let mut reordered = source.clone();
    reordered.revision = WorkbenchRevision::new(2);
    children(&mut reordered).reverse();
    assert!(
        validate_canonical_refresh(
            &projection,
            project_native_standalone_tabs(&reordered).unwrap()
        )
        .is_ok()
    );
    children(&mut reordered).pop();
    assert!(
        validate_canonical_refresh(
            &projection,
            project_native_standalone_tabs(&reordered).unwrap()
        )
        .is_ok()
    );
}

#[test]
fn child_binding_identity_cannot_be_duplicated_rebound_or_silently_filtered() {
    let mut source = snapshot(2, 1);
    children(&mut source).push(child(9000));
    let projection = project_native_standalone_tabs(&source).unwrap();
    for case in 0..6 {
        let mut next = source.clone();
        next.revision = WorkbenchRevision::new(2);
        match case {
            0 => {
                let duplicate = children(&mut next)[0].clone();
                children(&mut next).push(duplicate);
            }
            1 => children(&mut next)[0].binding = None,
            2 => children(&mut next)[0].binding.as_mut().unwrap().terminal_id = terminal_id(9999),
            3 => children(&mut next)[0].label = "changed identity label".into(),
            4 => {
                children(&mut next).clear();
                children(&mut next).extend((0..17).map(|n| child(9000 + n * 10)));
            }
            _ => {
                children(&mut next)[0].presentation_id =
                    projection.tabs()[1].presentation_id().clone()
            }
        }
        assert!(
            project_native_standalone_tabs(&next)
                .map(|candidate| validate_canonical_refresh(&projection, candidate).is_err())
                .unwrap_or(true),
            "case {case}"
        );
    }
}
