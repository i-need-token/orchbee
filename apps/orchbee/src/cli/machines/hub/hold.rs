//! `orchbee --machine-hub-hold-v1`: attaches with `presence` and `watch` through the client owner,
//! starting the hub when none runs, and prints one line per attachment and event until its standard
//! input ends, holding the hub meanwhile. A diagnostic entry for tests and evidence: its lines use
//! the contract's names, `attached <process id> <hubs started> <state>`, `reattached` likewise,
//! `event hub <state>`, `event catalog <revision>`, `event machine <id>`, and `ended`.

use super::printer::StateFailure;
use crate::{cli::Output, cli_text::CliText, localization::Locale};
use orchbee_machine_protocol::hub::{Event, HubState};
use orchbee_workbench_client::application::machine_hub::{
    Attached, Attachment, HubClient, Notice, data_root, running,
};
use std::{
    io::{Read, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// The bound on reaching the hub, a start included.
const ATTACH: Duration = Duration::from_secs(10);
/// The bound on the `query` for the process id.
const QUERY: Duration = Duration::from_secs(5);
/// How often the entry looks at its standard input between notices.
const POLL: Duration = Duration::from_millis(100);

pub(super) fn run(locale: Locale) -> Output {
    let text = CliText::new(locale);
    let attachment = Attachment {
        presence: true,
        asks: false,
        watch: true,
    };
    let failed = |failure: StateFailure| Output {
        status: failure.status(),
        stdout: Vec::new(),
        stderr: format!("{}\n", text.cli_machine_hub_failure(&failure)),
        finalization: None,
    };
    let (client, attached) = match HubClient::open(attachment, Instant::now() + ATTACH) {
        Ok(opened) => opened,
        Err(failure) => return failed(failure.into()),
    };
    line(&attachment_line("attached", attached));
    let (ended, input_ended) = mpsc::channel::<()>();
    let reader = thread::Builder::new()
        .name("orchbee-hub-hold-input".into())
        .spawn(move || {
            let _read = std::io::stdin().lock().read_to_end(&mut Vec::new());
            let _sent = ended.send(());
        });
    if reader.is_err() {
        client.close();
        return failed(StateFailure::Unavailable);
    }
    loop {
        if !matches!(input_ended.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            break;
        }
        match client.next(Instant::now() + POLL) {
            Some(Notice::Event(Event::Hub(state))) => line(&format!("event hub {}", name(state))),
            Some(Notice::Event(Event::Catalog(catalog))) => {
                line(&format!("event catalog {}", catalog.revision().get()));
            }
            Some(Notice::Event(Event::Machine(machine))) => {
                line(&format!("event machine {}", machine.machine.as_str()));
            }
            Some(Notice::Event(_)) | None => {}
            Some(Notice::Reattached(attached)) => line(&attachment_line("reattached", attached)),
            Some(Notice::Ended(failure)) => {
                line("ended");
                client.close();
                return failed(failure.into());
            }
        }
    }
    client.close();
    Output {
        status: 0,
        stdout: Vec::new(),
        stderr: String::new(),
        finalization: None,
    }
}

fn attachment_line(word: &str, attached: Attached) -> String {
    let process_id = data_root()
        .ok()
        .and_then(|root| running(&root, Instant::now() + QUERY).ok().flatten())
        .unwrap_or(0);
    format!(
        "{word} {process_id} {} {}",
        attached.starts,
        name(attached.hub)
    )
}

const fn name(state: HubState) -> &'static str {
    match state {
        HubState::Held => "held",
        HubState::Expiring { .. } => "expiring",
        HubState::Closing => "closing",
    }
}

/// Writes one line at once, so a reader sees it while the entry holds the hub.
fn line(text: &str) {
    let mut output = std::io::stdout().lock();
    let _written = writeln!(output, "{text}").and_then(|()| output.flush());
}
