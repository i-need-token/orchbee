//! The admitted link to the Server: one reader that applies each frame the Server sends, one
//! writer that sends the outbound queue, and the end the process takes. Only the Server's drain
//! ends the process cleanly; the link's end without one, a malformed frame or a failed write ends
//! it as a failure, which the Server counts as a crash. Either way every call is cancelled first,
//! and its operation gets a bounded time to end its children.

use std::{
    io::{self, BufReader, Write},
    net::Shutdown,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use orchbee_files_protocol::{DrainReason, LinkMessage, LinkSide, read_link_message};
use orchbee_local_ipc::LocalStream;

use crate::{calls::Calls, exit::Stage, operations::Services, outbound::Outbound};

/// How long one frame may take to reach a Server that stopped reading.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
/// How long cancelled operations get to end their children, which spans the 2-second grace a
/// terminated process tree gets before it is killed.
const SETTLE_BUDGET: Duration = Duration::from_secs(3);
const READ_BUFFER_BYTES: usize = 64 * 1024;

/// Why serving ended.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum End {
    /// The Server drained the process.
    Drained(DrainReason),
    /// A termination signal arrived.
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "only macOS and Linux catch termination signals")
    )]
    Signal(i32),
    /// The link ended without a drain.
    ServerGone,
    /// Serving failed at `Stage` with this I/O error kind.
    Failed(Stage, io::ErrorKind),
}

/// Serves the admitted `stream` until the first end any thread sends through `ends`, which a
/// signal handler may share, and returns it after cancelling every call.
pub(crate) fn serve(stream: LocalStream, ends: Sender<End>, ended: &Receiver<End>) -> End {
    let outbound = Arc::new(Outbound::new());
    let calls = Calls::new(Arc::clone(&outbound));
    let services = Services::new(Arc::clone(&calls));
    if let Err(error) = start(&stream, &outbound, &services, ends) {
        return End::Failed(Stage::Threads, error.kind());
    }
    // This function holds a sender until it returns, so the channel never disconnects first.
    let end = ended.recv().unwrap_or(End::ServerGone);
    services.drain();
    let deadline = Instant::now() + SETTLE_BUDGET;
    calls.settled_before(deadline);
    if matches!(end, End::Drained(_)) {
        // The Server reads its drain's closes; after any other end nothing reads them.
        outbound.sent_before(deadline);
    }
    outbound.close();
    let _ = stream.shutdown(Shutdown::Both);
    end
}

fn start(
    stream: &LocalStream,
    outbound: &Arc<Outbound>,
    services: &Arc<Services>,
    ends: Sender<End>,
) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    writer.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let reader = BufReader::with_capacity(READ_BUFFER_BYTES, stream.try_clone()?);
    let writer_outbound = Arc::clone(outbound);
    let writer_ends = ends.clone();
    thread::Builder::new()
        .name("orchbee-files-writer".into())
        .spawn(move || {
            while let Some(frame) = writer_outbound.next() {
                let written = writer.write_all(&frame);
                writer_outbound.written();
                if let Err(error) = written {
                    let _ = writer_ends.send(End::Failed(Stage::Write, error.kind()));
                    return;
                }
            }
        })?;
    let services = Arc::clone(services);
    thread::Builder::new()
        .name("orchbee-files-reader".into())
        .spawn(move || {
            let _ = ends.send(read(reader, &services));
        })?;
    Ok(())
}

/// Applies the Server's frames until one ends the link.
fn read(mut reader: BufReader<LocalStream>, services: &Arc<Services>) -> End {
    loop {
        let message = match read_link_message(&mut reader, LinkSide::Server) {
            Ok(message) => message,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return End::ServerGone,
            Err(error) => return End::Failed(Stage::Read, error.kind()),
        };
        match message {
            LinkMessage::Request(request) => {
                if services.request(request).is_err() {
                    return End::Failed(Stage::Read, io::ErrorKind::InvalidData);
                }
            }
            LinkMessage::Cancel { call } => services.cancel(call),
            LinkMessage::LaneClosed { connection, lane } => services.lane_closed(connection, lane),
            LinkMessage::ConnectionLost { connection } => services.connection_lost(connection),
            LinkMessage::Scope(update) => services.scope(update),
            // No service of this build refreshes on an Agent's completion.
            LinkMessage::AgentSettled { .. } => {}
            LinkMessage::Drain(reason) => return End::Drained(reason),
            // Admission is over, and the codec refuses this process's own kinds.
            LinkMessage::Admitted
            | LinkMessage::Refused(_)
            | LinkMessage::Hello(_)
            | LinkMessage::Frame(_)
            | LinkMessage::Close { .. }
            | LinkMessage::Activity { .. } => {
                return End::Failed(Stage::Read, io::ErrorKind::InvalidData);
            }
        }
    }
}

#[cfg(test)]
mod tests;
