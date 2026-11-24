//! `orchbee --machine-session <machine>`: one admitted Client connection to a saved machine's
//! Session, relayed over standard input and output for the desktop window, as `server connect`
//! relays the local one. It attaches to the running hub and never starts one, admits this build's own
//! offer through the machine's carrier as the TUI does, says how that went on its first line, and
//! then copies bytes it never reads. Ending it never stops the remote Session.
//! `apps/orchbee/docs/machines.md#desktop-session-relay` owns the contract.
use crate::{cli::Output, machine_carrier::relay_session};
use serde_json::json;
use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

/// How long joining the hub and the admission may take; Main waits 30 seconds for the first line.
const ADMISSION: Duration = Duration::from_secs(25);

pub(crate) fn serve(machine: &str) -> Output {
    let admitted = relay_session(machine, Instant::now() + ADMISSION);
    let first = match &admitted {
        Ok(session) => json!({
            "version": 1,
            "outcome": "admitted",
            "capabilities": session.capabilities,
        }),
        Err(reason) => json!({ "version": 1, "outcome": "refused", "reason": reason }),
    };
    let mut line = first.to_string().into_bytes();
    line.push(b'\n');
    let mut out = io::stdout().lock();
    if out.write_all(&line).and_then(|()| out.flush()).is_err() {
        return finished(1);
    }
    drop(out);
    let Ok(session) = admitted else {
        return finished(1);
    };
    let (Ok(upstream), Ok(sender)) = (session.stream.try_clone(), session.stream.try_clone())
    else {
        return finished(1);
    };
    if std::thread::Builder::new()
        .name("machine-session-stdin".to_owned())
        .spawn(move || {
            let mut writer = upstream;
            let _ended = crate::cli::server_connect::pump(&mut io::stdin().lock(), &mut writer);
            // A closed parent ends the connection, and with it the output relay.
            let _closed = sender.shutdown(std::net::Shutdown::Both);
        })
        .is_err()
    {
        return finished(1);
    }
    let mut reader = &session.stream;
    // Either side ending is the normal end of the relay; the parent observes the closed pipe.
    let _ended = crate::cli::server_connect::pump(&mut reader, &mut io::stdout().lock());
    finished(0)
}

fn finished(status: u8) -> Output {
    Output {
        status,
        stdout: Vec::new(),
        stderr: String::new(),
        finalization: None,
    }
}
