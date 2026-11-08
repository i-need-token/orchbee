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

/// Offline discovery ignores Session selection; other requests require native admission. A
/// surface this release withholds is refused first, on every platform.
pub(crate) fn admit(selected: &SelectedRequest) -> Result<(), Failure> {
    if let Request::Withheld(feature) = selected.request {
        return Err(withheld(feature));
    }
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

/// The refusal of a surface this release withholds: this build cannot serve it.
pub(crate) fn withheld(feature: crate::invocation::grammar::Withheld) -> Failure {
    let mut details = stage_reason_details("release", "withheld-in-release");
    details.insert("feature".to_owned(), serde_json::json!(feature.as_str()));
    Failure::new(FailureKind::ServiceUnavailable).details(details)
}

/// Transfer the parsed Session choice to its native owner after admission.
pub(crate) fn prepare(selected: SelectedRequest) -> Request {
    platform::select_session(selected.session);
    selected.request
}
