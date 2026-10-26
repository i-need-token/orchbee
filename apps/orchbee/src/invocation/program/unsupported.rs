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
    vec![name.to_owned()]
}
fn executable_file(path: &Path) -> bool {
    path.is_file() && false
}
