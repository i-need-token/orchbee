//! The diagnostic bundle reads the Windows data root and Session catalog; other platforms answer
//! the typed unsupported-platform failure until ORC-328.
use super::super::{Failure, FailureKind, stage_reason_details};
use super::Collected;
use crate::localization::Locale;

pub(super) fn collect(_locale: Locale) -> Result<Collected, Failure> {
    Err(Failure::new(FailureKind::ServiceUnavailable)
        .details(stage_reason_details("platform", "unsupported-platform")))
}
