//! Plugin popups through a real local Session: the CLI, Server, Worker and Zig Host under test, a
//! real TUI Client that runs in a terminal of a second Session, and a terminal that prints in
//! another Workspace. Each popup request submits ClientView controls that wake the TUI Client,
//! and the popup source's own terminal reads commit on the Server; whatever refresh that causes
//! must not refuse the popup. A Client held by copy mode or by a window too small for the popup
//! must name that cause.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use session::{Control, Fixture, Run};
use std::fs;

/// Opens per run, so that a pass does not depend on one quiet moment.
const OPENS: usize = 3;

#[test]
fn popups_open_on_a_live_client_and_busy_refusals_name_their_cause() {
    let fixture = Fixture::new("orchbee-plugin-popup-", &["plugin"]);
    link_plugin(&fixture);
    fixture.workspace("work");
    let tui = fixture.start_tui(160, 45);
    fixture.background_output();
    fixture.settle_ready(&tui.client);

    for _ in 0..OPENS {
        let opened = popup(&fixture, &tui.client);
        assert!(
            opened.code == Some(0)
                && opened.document["result"]["observation"]["reply"]["outcome"]["operation"]["status"]
                    ["kind"]
                    == "popup-opened",
            "{opened}"
        );
        let closed = fixture.run(&["popup", "close", "--client", &tui.client]);
        assert_eq!(closed.code, Some(0), "{closed}");
    }

    // Copy mode captures the Client's input.
    fixture.keys(&tui.terminal, &["ctrl+b", "U"]);
    fixture.settle_busy(&tui.client);
    assert_refused(&popup(&fixture, &tui.client), "modal-open");
    fixture.keys(&tui.terminal, &["escape"]);
    fixture.settle_ready(&tui.client);

    // The action menu captures the Client's input while its view stays ready.
    fixture.keys(&tui.terminal, &["ctrl+b", "f2"]);
    fixture.settle_ready(&tui.client);
    assert_refused(&popup(&fixture, &tui.client), "modal-open");
    fixture.keys(&tui.terminal, &["escape"]);
    fixture.settle_ready(&tui.client);

    // A window of 8 by 5 cells has no room for the popup.
    let small = Control::hold(&fixture, &tui.terminal, 8, 5);
    fixture.settle_busy(&tui.client);
    assert_refused(&popup(&fixture, &tui.client), "insufficient-space");
    small.release();
}

fn assert_refused(run: &Run, reason: &str) {
    let details = &run.document["failure"]["details"];
    assert!(
        run.code == Some(3)
            && details["reason"] == "client-refused"
            && details["clientReason"] == reason,
        "{run}"
    );
}

/// Links a popup plugin whose pane runs `cmd.exe` at half width and ten rows.
fn link_plugin(fixture: &Fixture) {
    let manifest = fixture.path("plugin/orchbee-plugin.toml");
    fs::write(
        &manifest,
        "id = 'audit'\nname = 'Audit tools'\nversion = '1'\n\n[[panes]]\nid = 'tools'\n\
         title = 'Tools'\nplacement = 'popup'\nwidth = '50%'\nheight = 10\n\
         command = ['cmd.exe']\n",
    )
    .unwrap();
    let linked = fixture.run(&["plugin", "link", &manifest]);
    assert_eq!(linked.code, Some(0), "{linked}");
}

fn popup(fixture: &Fixture, client: &str) -> Run {
    fixture.run(&[
        "plugin",
        "pane",
        "open",
        "--plugin",
        "audit",
        "--entrypoint",
        "tools",
        "--client",
        client,
    ])
}
