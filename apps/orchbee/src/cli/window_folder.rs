//! `--create-window-folder`: the hidden helper Desktop Main runs through its bundled Client
//! before its window opens, so the data root and Desktop's folder follow the rule every Orchbee
//! folder follows. It creates `internal/desktop` and each missing folder above it, the data root
//! among them, as private folders and records the root's layout. A folder another account can
//! reach is refused, never repaired, and the failure document names it as `server start` does.

use super::{Failure, FailureKind, Output, failure_output, stage_details};
use crate::invocation::OutputFormat;
use crate::localization::Locale;

pub(crate) fn create(locale: Locale) -> Output {
    let created = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::NotFound))
        .and_then(|root| orchbee_local_runtime::create_private_directory(&root.desktop()));
    let error = match created {
        Ok(_pins) => {
            return Output {
                status: 0,
                stdout: Vec::new(),
                stderr: String::new(),
                finalization: None,
            };
        }
        Err(error) => error,
    };
    let failed = Failure::new(FailureKind::CheckFailed).details(stage_details("data-root"));
    let failed = match orchbee_local_runtime::AdmissionRefusal::of(&error) {
        Some(refusal) => failed.admission_refusal(Box::new(refusal.clone())),
        None => failed,
    };
    failure_output(failed, Some(OutputFormat::Json), locale)
}
