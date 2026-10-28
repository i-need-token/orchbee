//! Lifecycle fences observed through the real fixed-viewport backend and its gated writer.

use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, atomic::Ordering},
};

use crate::terminal_lifecycle::{
    self, SynchronizedUpdateLease, TerminalModeGuard,
    test_support::{
        FailingOrderedWireWriter, FakeTerminalModes, OrderedWire, OrderedWireModes,
        OrderedWireWriter, lock_panic_hook_tests,
    },
};

fn failed_frame_backend(
    writer: FailingOrderedWireWriter,
    synchronized_update: &Arc<SynchronizedUpdateLease>,
) -> crate::tui_backend::FixedViewportBackend<
    io::BufWriter<crate::tui_backend::RestorationGatedWriter<FailingOrderedWireWriter>>,
> {
    failed_frame_backend_with_capacity(writer, synchronized_update, 64)
}

fn failed_frame_backend_with_capacity(
    writer: FailingOrderedWireWriter,
    synchronized_update: &Arc<SynchronizedUpdateLease>,
    capacity: usize,
) -> crate::tui_backend::FixedViewportBackend<
    io::BufWriter<crate::tui_backend::RestorationGatedWriter<FailingOrderedWireWriter>>,
> {
    crate::tui_backend::FixedViewportBackend::new(
        io::BufWriter::with_capacity(
            capacity,
            crate::tui_backend::RestorationGatedWriter::new(
                writer,
                Arc::clone(synchronized_update),
            ),
        ),
        crate::terminal_device::WindowDimensions {
            columns: 4,
            pixel_height: 32,
            pixel_width: 16,
            rows: 2,
        },
        Arc::clone(synchronized_update),
    )
}

#[test]
fn owner_panic_blocks_buffered_frame_bytes_from_reaching_the_wire() -> io::Result<()> {
    let _panic_hook_test = lock_panic_hook_tests()?;
    let wire = OrderedWire::default();
    let guard = TerminalModeGuard::enter(OrderedWireModes { wire: wire.clone() })?;
    let synchronized_update = guard.synchronized_update_lease();

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        use ratatui::backend::Backend;
        let mut backend = crate::tui_backend::FixedViewportBackend::new(
            io::BufWriter::with_capacity(
                64 * 1024,
                crate::tui_backend::RestorationGatedWriter::new(
                    OrderedWireWriter { wire: wire.clone() },
                    Arc::clone(&synchronized_update),
                ),
            ),
            crate::terminal_device::WindowDimensions {
                columns: 4,
                pixel_height: 32,
                pixel_width: 16,
                rows: 2,
            },
            Arc::clone(&synchronized_update),
        );
        let mut cell = ratatui::buffer::Cell::default();
        cell.set_symbol("a");
        backend
            .draw(std::iter::once((0, 0, &cell)))
            .expect("draw buffers begin + cells");
        panic!("injected panic while a synchronized frame is buffered");
    }));
    assert!(outcome.is_err(), "injected panic was caught");

    let events = wire.snapshot();
    assert_eq!(
        events,
        [
            "enable-raw-mode",
            "enter-alternate-screen",
            "hide-cursor",
            "end-synchronized-update",
            "show-cursor",
            "leave-alternate-screen",
            "disable-raw-mode",
        ],
        "recovery ends the open pair before the other modes"
    );
    assert!(
        !events.iter().any(|event| event.starts_with("frame-bytes:")),
        "no buffered frame bytes may reach the wire after restoration: {events:?}"
    );
    Ok(())
}

#[test]
fn a_discarded_failed_frame_drops_without_new_wire_bytes() -> io::Result<()> {
    let wire = OrderedWire::default();
    let mut guard =
        TerminalModeGuard::enter_without_panic_hook(OrderedWireModes { wire: wire.clone() })?;
    let synchronized_update = guard.synchronized_update_lease();
    let (writer, failure) = FailingOrderedWireWriter::partial_then_fail(wire.clone());
    let mut backend = failed_frame_backend_with_capacity(writer, &synchronized_update, 16);
    let mut cell = ratatui::buffer::Cell::default();
    cell.set_symbol("a");
    {
        use ratatui::backend::Backend;
        assert_eq!(
            backend
                .draw((0..8u16).map(|index| (index % 4, index / 4, &cell)))
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe,
            "the partial frame write is followed by the injected failure"
        );
    }
    assert!(synchronized_update.frame_open());

    // run_prepared_surface's error order: discard before the writer drops.
    let settled = wire.snapshot().len();
    assert!(
        wire.snapshot()
            .iter()
            .any(|event| event.starts_with("frame-bytes:")),
        "the failed flush reached the wire before leaving buffered residue"
    );
    failure.store(false, std::sync::atomic::Ordering::Relaxed);
    guard.discard_frame_output();
    drop(backend);
    assert_eq!(
        wire.snapshot().len(),
        settled,
        "the discarded writer's drop-flush emits nothing new"
    );

    guard.restore().map_err(io::Error::other)?;
    let events = wire.snapshot();
    assert_eq!(
        &events[settled..],
        [
            "end-synchronized-update",
            "show-cursor",
            "leave-alternate-screen",
            "disable-raw-mode"
        ],
        "the control channel repays the pending end, then restores modes"
    );
    Ok(())
}

#[test]
fn an_unfenced_writer_drop_retries_the_failed_frames_residue() {
    // Witness for why discard_frame_output must run before the writer drops:
    // BufWriter's drop always retries buffered bytes, so a recovered fd
    // accepts the failed frame's residue.
    let wire = OrderedWire::default();
    let synchronized_update = Arc::new(SynchronizedUpdateLease::new());
    let (writer, failure) = FailingOrderedWireWriter::partial_then_fail(wire.clone());
    let mut backend = failed_frame_backend_with_capacity(writer, &synchronized_update, 16);
    let mut cell = ratatui::buffer::Cell::default();
    cell.set_symbol("a");
    {
        use ratatui::backend::Backend;
        assert_eq!(
            backend
                .draw((0..8u16).map(|index| (index % 4, index / 4, &cell)))
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
    let settled = wire.snapshot().len();
    assert!(
        wire.snapshot()
            .iter()
            .any(|event| event.starts_with("frame-bytes:")),
        "the failed flush must write a prefix before retaining its residue"
    );
    failure.store(false, std::sync::atomic::Ordering::Relaxed);
    drop(backend);
    assert!(
        wire.snapshot().len() > settled,
        "without the output fence the drop-flush retries the failed frame's residue"
    );
}

#[test]
fn a_failed_end_is_repaid_through_the_control_channel() -> io::Result<()> {
    let wire = OrderedWire::default();
    let mut guard =
        TerminalModeGuard::enter_without_panic_hook(OrderedWireModes { wire: wire.clone() })?;
    let synchronized_update = guard.synchronized_update_lease();
    let (writer, failure) = FailingOrderedWireWriter::armed(wire.clone());
    let mut backend = failed_frame_backend(writer, &synchronized_update);
    let mut cell = ratatui::buffer::Cell::default();
    cell.set_symbol("a");
    {
        use ratatui::backend::Backend;
        backend
            .draw(std::iter::once((0, 0, &cell)))
            .expect("draw buffers the frame");
        failure.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(
            backend.flush().is_err(),
            "the injected end-marker failure propagates"
        );
    }
    let settled = wire.snapshot().len();
    guard.discard_frame_output();
    drop(backend);
    guard.restore().map_err(io::Error::other)?;
    let events = wire.snapshot();
    assert!(
        events[settled..]
            .iter()
            .filter(|event| *event == "end-synchronized-update")
            .count()
            == 1,
        "the failed end is repaid once through the control channel"
    );
    assert!(
        !events[settled..]
            .iter()
            .any(|event| event.starts_with("frame-bytes:")),
        "no frame bytes follow the discard"
    );
    Ok(())
}

#[test]
fn panic_cleans_partial_graphics_before_sync_and_discards_buffered_output() -> io::Result<()> {
    let _panic_hook_test = lock_panic_hook_tests()?;
    let wire = OrderedWire::default();
    let guard = TerminalModeGuard::enter(OrderedWireModes { wire: wire.clone() })?;
    let lease = guard.synchronized_update_lease();
    assert!(lease.graphics().register(11));
    lease.graphics().control_started();
    lease.open_frame();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        use io::Write;
        let mut buffered = io::BufWriter::new(crate::tui_backend::RestorationGatedWriter::new(
            OrderedWireWriter { wire: wire.clone() },
            Arc::clone(&lease),
        ));
        buffered
            .write_all(b"\x1b_Ga=t,i=11,m=1;unfinished")
            .unwrap();
        panic!("partial graphics frame before flush");
    }));
    assert!(outcome.is_err());
    let events = wire.snapshot();
    let cleanup = events
        .iter()
        .position(|event| event.starts_with("graphics-cleanup:"))
        .unwrap();
    assert!(events[cleanup].contains(&terminal_lifecycle::graphics::delete(11)));
    assert!(
        cleanup
            < events
                .iter()
                .position(|event| event == "end-synchronized-update")
                .unwrap()
    );
    assert!(
        cleanup
            < events
                .iter()
                .position(|event| event == "leave-alternate-screen")
                .unwrap()
    );
    assert!(!events.iter().any(|event| event.starts_with("frame-bytes:")));
    assert_eq!(lease.graphics().capacity(), 128);
    drop(guard);
    assert_eq!(wire.snapshot(), events);
    Ok(())
}

#[test]
fn actual_backend_graphics_write_failure_keeps_owned_cleanup_until_restoration() {
    use crate::{
        terminal_graphics::{Probe, Reply},
        tui_backend::ResizableBackend,
        workbench_client::PaneGraphicsCanvas,
    };
    use orchbee_workbench_protocol::{
        PaneGraphicsLayer, PaneGraphicsPlacement, PaneGraphicsTarget, SessionId, WorkbenchPanelId,
        WorkbenchRevision,
    };
    use ratatui::{backend::Backend, layout::Rect};
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone()).unwrap();
    let lease = guard.synchronized_update_lease();
    let (writer, failure) = FailingOrderedWireWriter::armed(OrderedWire::default());
    let mut backend = failed_frame_backend(writer, &lease);
    backend.enable_graphics_probe(99);
    let graphics = backend.graphics_surface().unwrap();
    let now = std::time::Instant::now();
    let mut probe = Probe::default();
    probe.activate(99);
    probe.query((4, 2), now).unwrap();
    probe.reply(Reply::Cell(Some((1, 2))), now);
    probe.reply(
        Reply::Kitty {
            image_id: 99,
            placement_id: None,
            success: true,
        },
        now,
    );
    graphics.update(probe.facts(), now, Vec::new(), false);
    let canvas = PaneGraphicsCanvas::fixture(
        PaneGraphicsTarget {
            panel_id: WorkbenchPanelId::parse("22222222222222222222222222222222").unwrap(),
            presentation_id: None,
        },
        WorkbenchRevision::new(1),
        vec![(
            PaneGraphicsLayer {
                layer_id: "primary".into(),
                z_index: 0,
                width: 1,
                height: 1,
                placement: PaneGraphicsPlacement::default(),
                resource_revision: WorkbenchRevision::new(1),
                stream_id: None,
            },
            vec![255; 4],
        )],
    );
    graphics.begin_frame(false);
    graphics.select(
        &SessionId::parse("11111111111111111111111111111111").unwrap(),
        &canvas,
        Rect::new(0, 0, 1, 1),
    );
    graphics.finish_frame(&[canvas.draw_for_test()], false);
    backend.draw(std::iter::empty()).unwrap();
    failure.store(true, Ordering::Relaxed);
    assert_eq!(
        backend.flush().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        lease.graphics().capacity(),
        127,
        "failed output cannot discharge image ownership"
    );
    guard.discard_frame_output();
    drop(backend);
    guard.restore().unwrap();
    assert_eq!(lease.graphics().capacity(), 128);
    let sequences = control.state.lock().unwrap().graphics_cleanup.clone();
    assert_eq!(sequences.len(), 1);
    assert!(sequences[0].starts_with("\x18\x1b\\"));
    assert!(sequences[0].contains(&terminal_lifecycle::graphics::delete(1)));
}
