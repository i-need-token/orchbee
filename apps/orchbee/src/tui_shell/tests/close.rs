//! Native Tab close confirmation through the real shell loop.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::{client_view::ClientView, workbench_client::TerminalStatus};
use orchbee_workbench_client::test_support::workbench_client::{
    TestResult, dispatch_close_input, fixture_geometry, interactive_close_server, native_client,
    native_workbench_client, spin_with_draw,
};

#[test]
fn real_shell_resize_and_confirm_in_one_iteration_keeps_the_native_prompt_unsubmitted() -> TestResult
{
    let geometry = fixture_geometry()?;
    let commands = Arc::new(AtomicUsize::new(0));
    let input_bytes = Arc::new(AtomicUsize::new(0));
    let server =
        interactive_close_server(geometry, Arc::clone(&commands), Arc::clone(&input_bytes))?;
    let mut workbench_client = native_workbench_client(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    dispatch_close_input(&mut workbench_client, &mut ClientView::new(100), b"\x02x");
    assert_eq!(
        super::run_native_close_resize_confirmation(&mut workbench_client)?,
        // armed at the loop-entry draw, still armed after the invalidated Enter,
        // still armed while the lone Esc waits out its decode deadline, gone
        // once the wait resolves at the next packet boundary.
        [true, true, true, false],
        "same-iteration resize+Enter must preserve the real current prompt until explicit Esc"
    );
    assert_eq!(commands.load(Ordering::SeqCst), 0);
    assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
    server.finish(workbench_client)?;
    Ok(())
}
