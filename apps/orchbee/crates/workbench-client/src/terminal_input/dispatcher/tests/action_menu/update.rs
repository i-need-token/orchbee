//! A click on Update from the action menu, through the dispatcher to the process's departure.

use super::*;
use crate::localization::Locale;
use crate::release_update::{Launcher, Updater, depart};
use orchbee_connection_handshake::{Generation, GenerationRange};

fn listing_generation_one() -> crate::release_update::ClientGenerations {
    Box::new(|_| {
        let one = Generation::new(1).unwrap();
        Some(GenerationRange::new(one, one).unwrap())
    })
}

/// Chooses the menu's only row, Update, with Enter, as a person does after the prefix and Space.
fn click_update(marker: bool) -> (DispatchOutcome, ClientView, Port) {
    let mut port = Port::new(MenuAction::Update);
    port.update = Some((Updater::ready("9.0.0", listing_generation_one()), marker));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02 ");
    port.arm();
    let outcome = dispatcher
        .dispatch(
            &mut view,
            b"\r",
            &mut port,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    (outcome, view, port)
}

/// A Session before slice 1 keeps this TUI and says when the update applies.
#[test]
fn update_waits_for_a_session_without_the_marker() {
    let (outcome, view, port) = click_update(false);
    assert_eq!(outcome, DispatchOutcome::Continue);
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::UpdateWaiting)
    );
    assert_eq!(port.update.as_ref().unwrap().0.departure().take(), None);
    assert!(forwarded_bytes(&port.calls).is_empty());
}

/// A Session the new version joins ends the TUI for it: status 75 under an announcing launcher,
/// and without one a normal exit with one line naming the command.
#[test]
fn update_leaves_with_status_75_or_the_hint() {
    let (outcome, view, port) = click_update(true);
    assert_eq!(outcome, DispatchOutcome::Exit);
    assert_eq!(view.notice(), None);
    let version = port.update.as_ref().unwrap().0.departure().take().unwrap();
    assert_eq!(version, "9.0.0");

    let announced = depart(&version, Launcher::Announcing, Locale::EnUs);
    assert_eq!((announced.status, announced.line), (75, None));

    let alone = depart(&version, Launcher::Absent, Locale::EnUs);
    assert_eq!(alone.status, 0);
    assert_eq!(
        alone.line.as_deref(),
        Some("Run orchbee to open version 9.0.0.\n")
    );
}
