use super::*;

fn standalone() -> NativeContextLocation {
    NativeContextLocation::Standalone(
        orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
            "00000000000000000000000000000001",
        )
        .unwrap(),
    )
}

fn catalog(revision: u64) -> Catalog {
    Catalog {
        source: Source {
            site: SiteId::parse("00000000000000000000000000000001").unwrap(),
            revision: WorkbenchRevision::new(revision),
            location: standalone(),
        },
        locations: vec![standalone()],
        tabs: (1..=20)
            .map(|n| WorkbenchTabId::parse(&format!("{n:032x}")).unwrap())
            .collect(),
        current_tab: Some(WorkbenchTabId::parse("00000000000000000000000000000001").unwrap()),
    }
}

#[test]
fn browsing_preserves_exact_focus_across_reorder_and_cancels_its_loss() {
    let mut nav = Navigator::default();
    assert!(nav.open());
    let mut rows = catalog(1);
    assert_eq!(nav.reconcile(Some(rows.clone())), Reconcile::Changed);
    nav.move_focus(Command::Last, 6);
    let focus = nav.focus.clone();
    rows.tabs.reverse();
    rows.source.revision = WorkbenchRevision::new(2);
    assert_eq!(nav.reconcile(Some(rows.clone())), Reconcile::Changed);
    assert_eq!(nav.focus, focus);
    rows.tabs.remove(0);
    assert_eq!(nav.reconcile(Some(rows)), Reconcile::Cancel);
}

#[test]
fn row_proof_requires_the_same_episode_source_and_keyboard_focus() {
    let mut nav = Navigator::default();
    nav.open();
    nav.reconcile(Some(catalog(1)));
    let first = nav.row(nav.focus.clone().unwrap()).unwrap();
    nav.move_focus(Command::Next, 5);
    assert!(!nav.accepts(&first, true));
    assert!(
        nav.accepts(&first, false),
        "another visible row remains clickable"
    );
    nav.reconcile(Some(catalog(2)));
    assert!(!nav.accepts(&first, false));
    nav.close();
    nav.open();
    nav.reconcile(Some(catalog(1)));
    assert!(!nav.accepts(&first, false));
}

#[test]
fn pages_and_column_changes_are_local_and_clamped() {
    let mut nav = Navigator::default();
    nav.open();
    nav.reconcile(Some(catalog(1)));
    nav.move_focus(Command::PageNext, 6);
    assert_eq!(nav.focus, Some(Target::Tab(catalog(1).tabs[5].clone())));
    nav.move_focus(Command::Locations, 6);
    assert_eq!(nav.focus, Some(Target::Location(standalone())));
    nav.move_focus(Command::Previous, 6);
    assert_eq!(nav.focus, Some(Target::Location(standalone())));
    nav.move_focus(Command::Tabs, 6);
    assert_eq!(nav.focus, Some(Target::Tab(catalog(1).tabs[0].clone())));
}

#[test]
fn an_initial_empty_catalog_may_arrive_but_a_lost_source_cannot_be_replaced() {
    let mut nav = Navigator::default();
    nav.open();
    assert_eq!(nav.reconcile(None), Reconcile::Unchanged);
    assert_eq!(nav.reconcile(Some(catalog(1))), Reconcile::Changed);
    assert_eq!(nav.reconcile(None), Reconcile::Cancel);

    nav.close();
    nav.open();
    let mut rows = catalog(1);
    rows.current_tab = None;
    nav.reconcile(Some(rows));
    assert_eq!(
        nav.focus,
        Some(Target::Location(standalone())),
        "initial entry cannot invent an active Tab"
    );
}

#[test]
fn site_or_location_changes_cancel_but_ordinary_output_preserves_paint_authority() {
    for site_changed in [false, true] {
        let mut nav = Navigator::default();
        nav.open();
        nav.reconcile(Some(catalog(1)));
        let proof = nav.row(nav.focus.clone().unwrap()).unwrap();
        assert_eq!(nav.reconcile(Some(catalog(1))), Reconcile::Unchanged);
        assert!(nav.accepts(&proof, true));
        let mut changed = catalog(1);
        if site_changed {
            changed.source.site = SiteId::parse(&format!("{:032x}", 2)).unwrap();
        } else {
            changed.source.location = NativeContextLocation::Standalone(
                orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&format!(
                    "{:032x}",
                    2
                ))
                .unwrap(),
            );
        }
        assert_eq!(nav.reconcile(Some(changed)), Reconcile::Cancel);
    }
}
