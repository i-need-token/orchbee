//! The automatic helper `--automatic-update-v1`, which the Session Server runs on its schedule. It
//! is `update` of the selected channel with three differences: it never edits the user `Path`, and
//! refreshes `bin\<stem>.exe` only where that file already exists; it writes nothing on standard
//! error; and it prints only the `orchbee.update` document. A build that does not run automatic
//! updates does not know it. Its grammar never changes; help, completion and the Agent guide leave
//! it out.
use super::{
    Output, Report, Settled, answer, document, engine, output, settle, start, update_release,
};
use crate::{invocation::OutputFormat, localization::Locale};

/// Runs one automatic update. Platform admission ran before dispatch.
pub(crate) fn run(locale: Locale) -> Output {
    let mut context = match start(
        super::super::execution::Capability::ReleaseUpdate,
        OutputFormat::Json,
        locale,
    ) {
        Ok(context) => context,
        Err(output) => return *output,
    };
    let mut report = Report::new();
    let result = update_release(
        None,
        engine::PathPolicy::LauncherIfPresent,
        &|_, _, _| {},
        &mut context,
        &mut report,
    );
    let settled = settle(context, &report, result);
    answer_document(&report, settled, locale)
}

/// The helper's whole answer for a settled run, whatever its outcome: one `orchbee.update` document
/// on standard output and nothing on standard error.
pub(super) fn answer_document(report: &Report, settled: Settled, locale: Locale) -> Output {
    let encoded =
        output::encode_document(&document(None, report, settled.error, settled.interrupted));
    answer(encoded, String::new(), settled, OutputFormat::Json, locale)
}
