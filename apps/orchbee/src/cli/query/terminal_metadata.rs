//! One read-only attachment samples frame metadata and the existing Windows Job facts.
use super::{
    BorrowedFd, Failure, FailureKind, Instant, LocalClient, Snapshot, Target, TerminalViewerAction,
    TerminalViewerRequest, attach_viewer_cancellable, obcs_request_id,
    poll_terminal_frame_cancellable, process_info,
};
use crate::cli::results::PanelProcessInfoDoc;
use orchbee_terminal_protocol::TerminalFrameRoute;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug)]
pub(in crate::cli) struct Metadata {
    pub(in crate::cli) route: TerminalFrameRoute,
    pub(in crate::cli) columns: u16,
    pub(in crate::cli) rows: u16,
    pub(in crate::cli) alternate_screen: bool,
    pub(in crate::cli) scrollback_rows: u64,
    pub(in crate::cli) title: Option<Arc<str>>,
    pub(in crate::cli) current_directory: Option<Arc<str>>,
}

#[derive(Debug)]
pub(in crate::cli) struct Read {
    pub(in crate::cli) frame: Metadata,
    pub(in crate::cli) processes: Option<(Value, PanelProcessInfoDoc)>,
}

pub(in crate::cli) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &Target,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<Read, Failure> {
    let (row, viewer, attachment, epoch, projection) =
        attach_viewer_cancellable(client, deadline, snapshot, target, wake)?;
    let result = (|| {
        let frame = poll_terminal_frame_cancellable(client, deadline, &viewer, projection, wake)?;
        let metadata = Metadata {
            route: frame.route,
            columns: frame.geometry.columns,
            rows: frame.geometry.rows,
            alternate_screen: frame.alternate_screen,
            scrollback_rows: frame.scrollback_rows,
            title: frame.title.clone(),
            current_directory: frame.current_directory.clone(),
        };
        // Do not retain a complete terminal screen for every Panel in a list.
        drop(frame);
        let processes = match process_info::read_attached(
            client,
            deadline,
            snapshot,
            row,
            (&viewer, attachment, epoch),
            wake,
        ) {
            Ok(observed) => Some(observed),
            Err(error) if error.kind == FailureKind::ServiceUnavailable => None,
            Err(error) => return Err(error),
        };
        Ok(Read {
            frame: metadata,
            processes,
        })
    })();
    detach_after(result, || {
        if let Ok(request_id) = obcs_request_id() {
            let _ = client.submit_terminal_viewer_cancellable(
                TerminalViewerRequest::new(
                    request_id,
                    TerminalViewerAction::Detach {
                        attachment_id: viewer,
                    },
                ),
                Some(deadline),
                wake,
            );
        }
    })
}

fn detach_after<T>(result: Result<T, Failure>, detach: impl FnOnce()) -> Result<T, Failure> {
    detach();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detach_is_attempted_after_both_successful_and_failed_observations() {
        for result in [Ok(()), Err(Failure::new(FailureKind::Denied))] {
            let detached = std::cell::Cell::new(false);
            let _ = detach_after(result, || detached.set(true));
            assert!(detached.get());
        }
    }
}
