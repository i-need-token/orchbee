use std::ffi::OsString;

use super::{Invocation, UsageFailure, parse_invocation};

#[test]
fn admits_only_the_three_accepted_argument_vectors() {
    let cases = [
        (Vec::<OsString>::new(), Ok(Invocation::Tui)),
        (vec![OsString::from("--help")], Ok(Invocation::Help)),
        (vec![OsString::from("--version")], Ok(Invocation::Version)),
        (vec![OsString::from("-h")], Err(UsageFailure)),
        (vec![OsString::from("help")], Err(UsageFailure)),
        (vec![OsString::from("--unknown")], Err(UsageFailure)),
        (
            vec![OsString::from("--help"), OsString::from("--version")],
            Err(UsageFailure),
        ),
    ];

    for (arguments, expected) in cases {
        assert_eq!(parse_invocation(arguments.into_iter()), expected);
    }
}

#[cfg(unix)]
#[test]
fn rejects_non_unicode_arguments() {
    use std::os::unix::ffi::OsStringExt;

    let argument = OsString::from_vec(vec![0xff]);
    assert_eq!(
        parse_invocation(vec![argument].into_iter()),
        Err(UsageFailure)
    );
}
