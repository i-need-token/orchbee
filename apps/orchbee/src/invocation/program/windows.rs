//! Native executable lookup policy, selected only at the execution boundary.
use super::{FailureEnvelope, ParseFailure, ParsedLaunch, UsageContext};
use orchbee_workbench_protocol::LaunchSelection;
use std::path::Path;
pub(crate) fn resolve(
    selection: Option<ParsedLaunch>,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<Option<LaunchSelection>, ParseFailure> {
    super::execution::resolve(selection, context, envelope, names, executable_file)
}
pub(super) fn names(name: &str) -> Vec<String> {
    if Path::new(name).extension().is_some() {
        vec![name.to_owned()]
    } else {
        vec![format!("{name}.exe"), format!("{name}.com")]
    }
}
fn executable_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("com"))
}
