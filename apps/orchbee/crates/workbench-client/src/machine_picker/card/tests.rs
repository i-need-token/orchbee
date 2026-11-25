//! The card through the real input path: Enter (`CR`) connects again, Esc puts it away, and no
//! other key reaches the terminals beneath it.

use super::*;
use crate::client_view::{ClientActionDisposition, ClientView};
use crate::machine_overview::Observation;
use crate::machine_spaces::{Checkpoint, ProjectSummary};
use crate::machines::{Draft, Profile};
use crate::terminal_input::{
    InputDispatcher, PaneAdmission, PaneInput, SurfaceAction, SurfaceInput, TerminalInputPort,
    TerminalInputUnavailable,
};
use crate::workbench_client::input::InputAction;
use crate::workbench_projection::{ClientContextLocation, ClientContextTarget};
use orchbee_machine_protocol::hub::MachineStateName;
use orchbee_workbench_protocol::{
    MetadataTokens, SessionId, WorkbenchRevision, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
};
use std::{sync::Arc, time::Instant};

const ENTER: &[u8] = b"\r";
const ESC: &[u8] = b"\x1b[27u";

/// A port no key may reach while the card shows.
#[derive(Default)]
struct Port {
    reached: usize,
}

impl TerminalInputPort for Port {
    fn forward(&mut self, _input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.reached += 1;
        Ok(())
    }

    fn pane_input(&mut self, _input: PaneInput<'_>) -> PaneAdmission {
        self.reached += 1;
        PaneAdmission::Rejected
    }

    fn surface_input(&mut self, _input: SurfaceInput<'_>) -> bool {
        self.reached += 1;
        false
    }

    fn surface_action(&mut self, _action: SurfaceAction<'_>) -> Option<ClientActionDisposition> {
        None
    }

    fn apply_client_action(&mut self, _action: InputAction) -> Option<ClientActionDisposition> {
        self.reached += 1;
        None
    }

    fn open_action_menu(
        &mut self,
        _target: crate::action_menu::MenuTarget,
        _anchor: Option<crate::action_menu::MenuAnchor>,
        _pointer_released: bool,
    ) -> bool {
        self.reached += 1;
        false
    }
}

fn hex(n: u64) -> String {
    format!("{n:032x}")
}

struct Bench {
    view: ClientView,
    dispatcher: InputDispatcher,
    port: Port,
    devbox: Profile,
}

impl Bench {
    /// `devbox` is saved, reads `state`, and lists one Workspace from when it was last seen.
    fn new(state: MachineStateName) -> Self {
        let devbox = Profile::new(
            ProfileId::parse(&hex(7)).unwrap(),
            Draft::parse("devbox", "example.invalid", "default").unwrap(),
        );
        let mut view = ClientView::new(100);
        view.machines.available = true;
        view.machines.observe_agent_sidebar(true);
        view.machines.replace(std::slice::from_ref(&devbox));
        view.machines
            .observe(&[(devbox.id.clone(), Observation::State(state))]);
        let session_id = SessionId::parse(&hex(2)).unwrap();
        let row = crate::machine_spaces::Row {
            target: ClientContextTarget {
                session_id: session_id.clone(),
                location: ClientContextLocation::Workspace {
                    workspace_id: WorkbenchWorkspaceId::parse(&hex(3)).unwrap(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&hex(4)).unwrap(),
                },
            },
            project_id: None,
            label: Some("api".into()),
            tokens: MetadataTokens::default(),
            directory: None,
            terminal_directory: None,
            git: None,
            git_stale: false,
            held: None,
        };
        let checkpoint = Checkpoint {
            session_id,
            revision: WorkbenchRevision::new(0),
            metadata_revision: WorkbenchRevision::new(0),
            git_requested: false,
            rows: vec![row].into(),
            projects: Vec::<ProjectSummary>::new().into(),
            known_git: Vec::new().into(),
        };
        assert!(
            view.machines
                .seed_last_seen(vec![(devbox.id.clone(), Arc::new(checkpoint))])
        );
        Self {
            view,
            dispatcher: InputDispatcher::default(),
            port: Port::default(),
            devbox,
        }
    }

    fn facts(&self, state: State) -> Machine {
        Machine {
            id: self.devbox.id.as_str().to_owned(),
            label: "devbox".into(),
            address: None,
            state,
            capacity: None,
            workspaces: None,
            terminals: None,
            agents: None,
            last_attempt: None,
            explanation: crate::machine_status::Explanation::default(),
            stopping: false,
            color: crate::machine_color::MachineColor::DEFAULT,
        }
    }

    /// Chooses devbox's Workspace in the sidebar, as Enter or a click on its row does.
    fn choose(&mut self) -> Selection {
        let selected = self
            .view
            .machines
            .workspace_rows
            .iter()
            .find(|row| row.choice == Choice::Remote(self.devbox.id.clone()))
            .expect("devbox's last-seen Workspace is listed")
            .selection();
        assert!(self.view.machines.activate_sidebar_workspace(&selected));
        selected
    }

    fn send(&mut self, bytes: &[u8]) {
        let outcome = crate::terminal_input::dispatch_input(
            &mut self.dispatcher,
            &mut self.view,
            bytes,
            &mut self.port,
            Instant::now(),
            |_view, _port| Ok::<(), std::convert::Infallible>(()),
        );
        assert!(matches!(
            outcome,
            Ok(crate::terminal_input::DispatchOutcome::Continue)
        ));
    }
}

#[test]
fn a_chosen_offline_machine_shows_its_card_until_esc_and_enter_connects_again() {
    let mut bench = Bench::new(MachineStateName::Offline);
    let selected = bench.choose();
    assert_eq!(
        bench.view.machines.card_machine(),
        Some(bench.devbox.id.clone())
    );
    let facts = bench.facts(State::Offline);
    assert!(bench.view.machines.observe_card(Some(facts)));
    assert!(bench.view.machines.card_shown() && bench.view.machines.captures());

    // Typing goes nowhere while the card covers another machine's terminals.
    bench.view.machines.request = None;
    bench.send(b"ls");
    assert_eq!(bench.port.reached, 0);
    assert_eq!(bench.view.machines.request, None);

    // Enter makes the same choice again, so the hub tries at once; the card stays.
    bench.send(ENTER);
    assert_eq!(bench.view.machines.request, Some(selected));
    assert!(bench.view.machines.card_shown());

    // Esc gives the choice up.
    bench.send(ESC);
    assert!(!bench.view.machines.card_shown());
    assert_eq!(bench.view.machines.request, None);
    assert_eq!(bench.port.reached, 0);
}

#[test]
fn the_machine_on_screen_shows_its_card_while_lost_and_esc_keeps_it_away_until_its_state_changes() {
    let mut bench = Bench::new(MachineStateName::Reconnecting);
    let devbox = Choice::Remote(bench.devbox.id.clone());
    bench
        .view
        .machines
        .observe_focus(devbox.clone(), None, None);
    assert_eq!(
        bench.view.machines.card_machine(),
        Some(bench.devbox.id.clone())
    );
    let lost = bench.facts(State::Reconnecting);
    assert!(bench.view.machines.observe_card(Some(lost.clone())));
    assert!(bench.view.machines.card_shown());

    // Enter chooses the machine on screen again.
    bench.send(ENTER);
    assert_eq!(
        bench.view.machines.request,
        Some(Selection::from(devbox.clone()))
    );

    bench.send(ESC);
    assert!(!bench.view.machines.card_shown());
    bench.view.machines.observe_card(Some(lost));
    assert!(!bench.view.machines.card_shown());
    let login = bench.facts(State::NeedsLogin);
    bench.view.machines.observe_card(Some(login));
    assert!(bench.view.machines.card_shown());
    let online = bench.facts(State::Online);
    bench.view.machines.observe_card(Some(online));
    assert!(!bench.view.machines.card_shown());
    assert_eq!(bench.port.reached, 0);
}
