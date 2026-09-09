use std::ffi::OsString;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Invocation {
    Help,
    Tui,
    Version,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UsageFailure;

pub(crate) fn parse_invocation(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<Invocation, UsageFailure> {
    let Some(argument) = arguments.next() else {
        return Ok(Invocation::Tui);
    };
    if arguments.next().is_some() {
        return Err(UsageFailure);
    }
    if argument == "--help" {
        return Ok(Invocation::Help);
    }
    if argument == "--version" {
        return Ok(Invocation::Version);
    }
    if argument == "tui" {
        return Ok(Invocation::Tui);
    }
    Err(UsageFailure)
}

#[cfg(test)]
mod tests;
