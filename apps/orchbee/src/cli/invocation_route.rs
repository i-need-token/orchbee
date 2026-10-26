//! Process-entry routing preserves offline discovery and checks platform admission before
//! selecting a runtime Session. It owns no grammar, environment lookup or runtime activation.
use super::{Failure, FailureKind, Output, stage_reason_details};
use crate::invocation::{QueryCommand, Request, SelectedRequest};
use crate::localization::Locale;

#[cfg(windows)]
#[path = "invocation_route/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "invocation_route/unix.rs"]
mod platform;

struct Unsupported;

/// Keep the supported canonical template ahead of even locale environment reads.
pub(crate) fn offline_default_config(request: &Request) -> Option<Output> {
    (matches!(request, Request::DefaultConfig)
        && crate::tui_capabilities::Capability::ConfigurationFile.available())
    .then(|| super::default_config(Locale::EnUs))
}

/// Offline discovery ignores Session selection; other requests require native admission.
pub(crate) fn admit(selected: &SelectedRequest) -> Result<(), Failure> {
    if matches!(
        selected.request,
        Request::RootHelp
            | Request::Help(_)
            | Request::Version
            | Request::Skill
            | Request::Completion(_)
            | Request::Query {
                command: QueryCommand::ApiSchema { .. },
                ..
            }
    ) {
        return Ok(());
    }
    platform::admit(selected.availability, selected.session.is_some()).map_err(|Unsupported| {
        Failure::new(FailureKind::ServiceUnavailable)
            .details(stage_reason_details("platform", "unsupported-platform"))
    })
}

/// Transfer the parsed Session choice to its native owner after admission.
pub(crate) fn prepare(selected: SelectedRequest) -> Request {
    platform::select_session(selected.session);
    selected.request
}
