//! The Tab naming rule both Clients share between live frame titles and the Server's published
//! titles.
use super::super::presentation_titles::{PresentationTitles, ShownTitle};
use crate::localization::Locale;
use crate::workbench_projection::ClientTerminalBinding;
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, PresentationLaunchProgram, PresentationTitle,
    PresentationTitleRow, SessionId, Snapshot, TerminalId, TerminalPresentationId, WorkerId,
};

fn primary(source: &Snapshot, tab: usize) -> (TerminalPresentationId, PresentationBinding) {
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &source.terminal_contexts[0].tabs[tab].root
    else {
        unreachable!()
    };
    (
        primary_presentation.presentation_id.clone(),
        primary_presentation.binding.clone().unwrap(),
    )
}

fn binding(index: u64) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&format!("{:032x}", 9000 + index)).unwrap(),
        worker_id: WorkerId::parse(&format!("{:032x}", 9100 + index)).unwrap(),
    }
}

fn observed(title: Option<&str>) -> PresentationTitle {
    PresentationTitle::Observed {
        title: title.map(str::to_owned),
    }
}

/// Step by step: the foreground Tab has a live frame, the background Tab has none.
#[test]
fn tab_names_take_the_newest_title_of_their_terminal_and_keep_it_while_none_is_available() {
    let mut source = super::super::tests::membership::snapshot(2, 1);
    for tab in &mut source.terminal_contexts[0].tabs {
        tab.automatic_label = true;
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &mut tab.root
        else {
            unreachable!()
        };
        primary_presentation.launch_program =
            Some(PresentationLaunchProgram::parse("pwsh.exe").unwrap());
    }
    let tabs = [primary(&source, 0), primary(&source, 1)];
    let (mut ui, _, _, _) = super::super::tests::move_membership::ready_ui_with_snapshot(source);
    super::super::test_support::install_projection(&mut ui, Some((0, false, 24)), 0);

    enum Step<'a> {
        /// The foreground terminal's live frame retitles, and a draw records it.
        Frame(Option<&'a str>),
        /// The foreground viewer goes away, as when its Tab is left.
        Leave,
        /// One stream record: the Tab, the binding it was read for, and the title.
        Row(usize, Option<PresentationBinding>, PresentationTitle),
    }
    let steps = [
        (Step::Frame(None), ["PowerShell", "PowerShell"]),
        (Step::Frame(Some("⠋ claude")), ["claude", "PowerShell"]),
        // A background terminal's published title names its Tab.
        (
            Step::Row(1, Some(tabs[1].1.clone()), observed(Some("build"))),
            ["claude", "build"],
        ),
        // A published title newer than the live frame's wins until the frame changes.
        (
            Step::Row(0, Some(tabs[0].1.clone()), observed(Some("server"))),
            ["server", "build"],
        ),
        (Step::Frame(Some("vim")), ["vim", "build"]),
        // Republishing an unchanged value is no arrival, so it never overrides a newer frame.
        (
            Step::Row(0, Some(tabs[0].1.clone()), observed(Some("server"))),
            ["vim", "build"],
        ),
        // A row read for another terminal is ignored.
        (
            Step::Row(1, Some(binding(1)), observed(Some("other"))),
            ["vim", "build"],
        ),
        (
            Step::Row(1, None, PresentationTitle::Unavailable {}),
            ["vim", "build"],
        ),
        // Unavailable keeps the shown title; a cleared title names the program.
        (
            Step::Row(
                1,
                Some(tabs[1].1.clone()),
                PresentationTitle::Unavailable {},
            ),
            ["vim", "build"],
        ),
        (
            Step::Row(1, Some(tabs[1].1.clone()), observed(None)),
            ["vim", "PowerShell"],
        ),
        (
            Step::Row(
                1,
                Some(tabs[1].1.clone()),
                PresentationTitle::Unavailable {},
            ),
            ["vim", "PowerShell"],
        ),
        // A Tab keeps its last frame title after its viewer goes, then follows the Server.
        (Step::Leave, ["vim", "PowerShell"]),
        (
            Step::Row(
                0,
                Some(tabs[0].1.clone()),
                PresentationTitle::Unavailable {},
            ),
            ["vim", "PowerShell"],
        ),
        (
            Step::Row(0, Some(tabs[0].1.clone()), observed(Some("ready"))),
            ["ready", "PowerShell"],
        ),
    ];
    for (index, (step, expected)) in steps.into_iter().enumerate() {
        match step {
            Step::Frame(title) => {
                let projection = &mut ui.projections[0].value;
                let mut frame = super::super::test_support::frame(projection, 0, false, 24);
                frame.title = title.map(Into::into);
                projection.install_frame_for_test(frame);
                // The name changes before a draw records the frame, and stays after it does.
                assert_eq!(
                    ui.view().tab(0).unwrap().display_label(Locale::EnUs),
                    expected[0]
                );
                ui.record_live_titles();
            }
            Step::Leave => ui.projections.clear(),
            Step::Row(tab, binding, title) => {
                ui.receive_presentation_titles(vec![PresentationTitleRow {
                    presentation_id: tabs[tab].0.clone(),
                    binding,
                    title,
                }]);
            }
        }
        let names = [0, 1].map(|tab| {
            ui.view()
                .tab(tab)
                .unwrap()
                .display_label(Locale::EnUs)
                .into_owned()
        });
        assert_eq!(names, expected, "step {index}");
    }
}

/// A row waits until the snapshot binds its Presentation to the terminal it was read for, and a
/// Presentation bound to another terminal shows its program until that terminal has a title.
#[test]
fn a_title_applies_only_to_the_terminal_the_snapshot_binds() {
    let session = SessionId::parse(&format!("{:032x}", 1)).unwrap();
    let presentation = TerminalPresentationId::parse(&format!("{:032x}", 2)).unwrap();
    let [first, second, third] = [1, 2, 3].map(binding);
    let terminals = [&first, &second, &third].map(ClientTerminalBinding::from);
    let mut titles = PresentationTitles::default();
    let shown = |titles: &PresentationTitles, terminal: usize| match titles.shown(
        &presentation,
        Some(&terminals[terminal]),
        None,
    ) {
        ShownTitle::Program => None,
        ShownTitle::Projected(text) => Some(text.to_owned()),
        ShownTitle::Live(_) => unreachable!(),
    };

    titles.observe_surface(&session, &presentation, Some(&terminals[0]), Some("vim"));
    assert_eq!(shown(&titles, 0).as_deref(), Some("vim"));
    // The row for the second terminal arrives before the snapshot that binds it.
    titles.receive(
        &session,
        vec![PresentationTitleRow {
            presentation_id: presentation.clone(),
            binding: Some(second.clone()),
            title: observed(Some("build")),
        }],
        |_| Some(&terminals[0]),
    );
    assert_eq!(shown(&titles, 0).as_deref(), Some("vim"));
    assert_eq!(shown(&titles, 1).as_deref(), Some("build"));
    titles.follow(&session, |_| Some(Some(&terminals[1])));
    assert_eq!(shown(&titles, 1).as_deref(), Some("build"));
    // Another terminal shows its program until a title arrives for it; the old row never applies.
    titles.follow(&session, |_| Some(Some(&terminals[2])));
    assert_eq!(shown(&titles, 2), None);
    assert_eq!(
        titles.shown(
            &presentation,
            Some(&terminals[2]),
            Some(Some("Administrator: top"))
        ),
        ShownTitle::Live("Administrator: top")
    );
    titles.observe_surface(
        &session,
        &presentation,
        Some(&terminals[2]),
        Some("Administrator: top"),
    );
    assert_eq!(shown(&titles, 2).as_deref(), Some("top"));
    // A Presentation that leaves the snapshot takes its title with it.
    titles.follow(&session, |_| None);
    assert_eq!(shown(&titles, 2), None);
}
