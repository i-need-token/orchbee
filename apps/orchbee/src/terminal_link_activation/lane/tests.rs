use std::{
    num::NonZeroU64,
    sync::{Arc, atomic::AtomicU64},
};

use super::{
    Admission, BrowserError, BrowserPort, Driver, EffectHandoff, Notice, browser_url,
    settle_browser, settle_effect,
};
use crate::{
    terminal_link_activation::{Activation, PaintedLinkSource, PaintedRows, SnapshotMember},
    workbench_client::state::{AttachmentKey, PaneKey, PaneRef},
};

#[derive(Default)]
struct RecordingBrowser {
    opened: Vec<String>,
    failure: Option<BrowserError>,
}

impl BrowserPort for RecordingBrowser {
    fn open(&mut self, url: &str) -> Result<(), BrowserError> {
        self.opened.push(url.to_owned());
        if let Some(error) = self.failure {
            Err(error)
        } else {
            Ok(())
        }
    }
}

#[test]
fn browser_fallback_accepts_only_lowercase_http_schemes() {
    assert!(browser_url("http://example.test"));
    assert!(browser_url("https://example.test/path"));
    assert!(!browser_url("HTTP://example.test"));
    assert!(!browser_url("file:///C:/secret"));
}

#[test]
fn accepted_plugin_ambiguity_never_falls_through_to_the_browser() {
    let mut browser = RecordingBrowser::default();
    let notice = settle_effect(
        EffectHandoff::Plugin(Ok(Activation::Unknown {
            operation_id: "op".into(),
            plugin_id: "plugin".into(),
            handler_id: "handler".into(),
            action_id: "action".into(),
            worker: None,
        })),
        "https://example.test",
        &mut browser,
    );
    assert_eq!(notice, Notice::Unknown);
    assert!(browser.opened.is_empty());

    let notice = settle_effect(
        EffectHandoff::Plugin(Err(
            crate::terminal_link_activation::Error::DefinitelyNotSubmitted,
        )),
        "https://example.test",
        &mut browser,
    );
    assert_eq!(notice, Notice::Failed);
    assert!(browser.opened.is_empty());
}

#[test]
fn definite_no_handler_launches_the_browser_exactly_once() {
    let mut browser = RecordingBrowser::default();
    assert_eq!(
        settle_effect(
            EffectHandoff::NoHandler,
            "https://example.test",
            &mut browser,
        ),
        Notice::Opened
    );
    assert_eq!(browser.opened, vec!["https://example.test".to_owned()]);

    let mut failing = RecordingBrowser {
        failure: Some(BrowserError::Failed),
        ..RecordingBrowser::default()
    };
    assert_eq!(
        settle_effect(
            EffectHandoff::NoHandler,
            "https://example.test",
            &mut failing,
        ),
        Notice::Failed
    );
    assert_eq!(failing.opened, vec!["https://example.test".to_owned()]);

    let mut unavailable = RecordingBrowser {
        failure: Some(BrowserError::Unsupported),
        ..RecordingBrowser::default()
    };
    assert_eq!(
        settle_effect(
            EffectHandoff::NoHandler,
            "https://example.test",
            &mut unavailable
        ),
        Notice::Unsupported
    );
    assert_eq!(
        unavailable.opened.len(),
        1,
        "unsupported capability settles without replay"
    );

    let mut unsupported = RecordingBrowser::default();
    assert_eq!(
        settle_effect(
            EffectHandoff::NoHandler,
            "mailto:user@example.test",
            &mut unsupported,
        ),
        Notice::Failed
    );
    assert!(unsupported.opened.is_empty());
}

#[test]
fn nonmatching_catalog_drift_refuses_browser_without_an_effect() {
    let initial = orchbee_workbench_protocol::PluginLinkCatalogGuard {
        revision: 7,
        fingerprint: "a".repeat(64),
    };
    let current = crate::terminal_link_activation::plugin::SelectionSnapshot {
        selection: None,
        catalog: orchbee_workbench_protocol::PluginLinkCatalogGuard {
            revision: 8,
            fingerprint: "b".repeat(64),
        },
    };
    let mut browser = RecordingBrowser::default();
    assert_eq!(
        settle_browser(&initial, current, "https://example.test", &mut browser,),
        Notice::Failed
    );
    assert!(browser.opened.is_empty());
}

#[test]
fn busy_driver_refuses_without_spawning_or_queueing_another_lane_job() {
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = crate::local_client::tests::server::adjustable_operation_clock(elapsed);
    let mut driver = Driver::new(clock);
    driver.inflight = true;
    assert_eq!(driver.admit(source(), 0, 0), Admission::Busy);
    assert!(driver.lane.is_none());
    assert!(driver.inflight);
}

fn source() -> PaintedLinkSource {
    fn id(value: u128) -> String {
        format!("{value:032x}")
    }
    let tab = orchbee_workbench_protocol::WorkbenchTabId::parse(&id(1)).unwrap();
    let panel = orchbee_workbench_protocol::WorkbenchPanelId::parse(&id(2)).unwrap();
    PaintedLinkSource {
        pane: PaneRef::new(
            PaneKey::new(AttachmentKey::new(1, tab.clone()), panel.clone()),
            0,
        ),
        member: SnapshotMember {
            session: orchbee_workbench_protocol::SessionId::parse(&id(3)).unwrap(),
            context: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&id(4)).unwrap(),
            tab,
            panel,
            presentation: orchbee_workbench_protocol::TerminalPresentationId::parse(&id(5))
                .unwrap(),
            worker: orchbee_workbench_protocol::WorkerId::parse(&id(6)).unwrap(),
            terminal: orchbee_workbench_protocol::TerminalId::parse(&id(7)).unwrap(),
        },
        host_terminal: orchbee_client_server_protocol::HostTerminalId::parse(&id(10)).unwrap(),
        screen: crate::terminal_content::TerminalScreen::Primary,
        rows: PaintedRows {
            attachment: orchbee_terminal_protocol::AttachmentId::from_bytes([8; 16]).unwrap(),
            host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([9; 16]).unwrap(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 1,
            columns: 80,
            start_column: 0,
            total_rows: 24,
            start_row: 0,
            row_count: 24,
            history: false,
        },
    }
}
