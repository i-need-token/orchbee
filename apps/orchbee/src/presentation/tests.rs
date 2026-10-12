use crate::{
    invocation::{Invocation, UsageFailure},
    localization::Locale,
};

use super::{ProcessResult, present};

#[test]
fn every_result_uses_exactly_one_stream_or_the_other() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for invocation in [
            Ok(Invocation::Tui),
            Ok(Invocation::Version),
            Err(UsageFailure),
        ] {
            let result = present(invocation, locale);
            assert_ne!(result.stdout.is_empty(), result.stderr.is_empty());
        }
    }
}

#[test]
fn maps_the_finite_decision_table_to_development_statuses() {
    let cases = [
        (Ok(Invocation::Tui), 1),
        (Ok(Invocation::Version), 0),
        (Err(UsageFailure), 2),
    ];

    for (invocation, status) in cases {
        let result = present(invocation, Locale::EnUs);
        assert_eq!(result.status, status);
    }
}

#[test]
fn version_output_is_locale_neutral() {
    let expected = ProcessResult {
        status: 0,
        stderr: "",
        stdout: concat!("orchbee ", env!("CARGO_PKG_VERSION"), "\n"),
    };
    assert_eq!(present(Ok(Invocation::Version), Locale::EnUs), expected);
    assert_eq!(present(Ok(Invocation::Version), Locale::ZhCn), expected);
}
